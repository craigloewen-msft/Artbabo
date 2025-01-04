use std::pin::Pin;
use std::{collections::HashMap, sync::Arc};

use std::future::Future;

use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use ws::stream::DuplexStream;
use ws::{result::Result, Message};

use log::{error, info, warn};

use async_channel;

use tokio::sync::Mutex;

pub use bevy_eventwork::{ConnectionId, NetworkMessage};

pub trait EventWorkSendMessages {
    async fn send_message<T>(&self, connection_id: usize, message: T) -> Result<(), String>
    where
        T: NetworkMessage;

    async fn broadcast<T>(&self, message: T) -> Result<(), String>
    where
        T: NetworkMessage;
}

// Taken from bevy_eventwork, made public so the server doesn't have to include bevy as a dependency
pub enum NetworkEvent {
    Connected(ConnectionId),
    Disconnected(ConnectionId),
    // TODO: Implement errors
    // Error(String),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NetworkPacket {
    kind: String,
    data: Vec<u8>,
}

#[derive(Clone)]
pub struct EventWorkPacket {
    id: usize,
    broadcast: bool,
    serialized_packet: Vec<u8>,
}

type BoxedFuture = Pin<Box<dyn Future<Output = Result<(), String>> + Send>>;
type EventHandleFunction = dyn Fn(EventWorkSender) -> BoxedFuture + Send + Sync;
type EventHandleFunctionStore = Arc<EventHandleFunction>;

pub struct EventWorkSender {
    pub packet_input: NetworkPacket,
    pub message_send_channel: async_channel::Sender<EventWorkPacket>,
    pub connection_id: usize,
}

impl EventWorkSender {
    pub fn get_network_data<T>(&self) -> Result<T, String>
    where
        T: for<'de> Deserialize<'de> + NetworkMessage,
    {
        match bincode::deserialize(&self.packet_input.data) {
            Ok(data) => Ok(data),
            Err(e) => Err(e.to_string()),
        }
    }

    fn from_message_to_packet<T>(
        connection_id: usize,
        broadcast: bool,
        message: T,
    ) -> EventWorkPacket
    where
        T: NetworkMessage,
    {
        let packet = NetworkPacket {
            kind: String::from(T::NAME),
            data: bincode::serialize(&message).unwrap(),
        };

        let serialized_packet = bincode::serialize(&packet).unwrap();

        EventWorkPacket {
            id: connection_id,
            broadcast,
            serialized_packet,
        }
    }
}

impl EventWorkSendMessages for EventWorkSender {
    async fn send_message<T>(&self, connection_id: usize, message: T) -> Result<(), String>
    where
        T: NetworkMessage,
    {
        let eventwork_packet = Self::from_message_to_packet(connection_id, false, message);

        match self.message_send_channel.send(eventwork_packet).await {
            Ok(_) => {
                return Ok(());
            }
            Err(e) => {
                return Err(e.to_string());
            }
        }
    }

    async fn broadcast<T>(&self, message: T) -> Result<(), String>
    where
        T: NetworkMessage,
    {
        let eventwork_packet = Self::from_message_to_packet(0, true, message);

        match self.message_send_channel.send(eventwork_packet).await {
            Ok(_) => {
                return Ok(());
            }
            Err(e) => {
                return Err(e.to_string());
            }
        }
    }
}

struct EventWorkConnection {
    id: usize,
    handle_packet_task: Arc<dyn Fn() -> BoxedFuture + Send + Sync>,
    write_reference: Arc<Mutex<SplitSink<DuplexStream, Message>>>,
}

impl EventWorkConnection {
    pub async fn send_message(&self, message: EventWorkPacket) -> Result<(), String> {
        match self
            .write_reference
            .lock()
            .await
            .send(Message::Binary(message.serialized_packet))
            .await
        {
            Ok(_) => {
                return Ok(());
            }
            Err(e) => {
                return Err(e.to_string());
            }
        }
    }
}

pub struct EventWorkServer {
    event_map_reference: Arc<Mutex<HashMap<String, EventHandleFunctionStore>>>,
    connection_counter: usize,
    active_connections_reference: Arc<Mutex<HashMap<usize, EventWorkConnection>>>,
    tx_message_send_channel: async_channel::Sender<EventWorkPacket>,
    tx_message_receive_channel: async_channel::Receiver<EventWorkPacket>,
    network_event_send_channel: async_channel::Sender<NetworkEvent>,
    network_event_receive_channel: async_channel::Receiver<NetworkEvent>,
    network_event_function_option_reference:
        Arc<Mutex<Option<Arc<dyn Fn(NetworkEvent) -> BoxedFuture + Send + Sync>>>>,
}

impl EventWorkServer {
    pub fn default() -> Self {
        let (send, receive) = async_channel::unbounded();
        let (close_send, close_receive) = async_channel::unbounded();

        Self {
            event_map_reference: Arc::new(Mutex::new(HashMap::default())),
            connection_counter: 0,
            active_connections_reference: Arc::new(Mutex::new(HashMap::default())),
            tx_message_send_channel: send,
            tx_message_receive_channel: receive,
            network_event_send_channel: close_send,
            network_event_receive_channel: close_receive,
            network_event_function_option_reference: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn init(&self) {
        // Spawn thread for handling message send requests
        let tx_message_receive_channel = self.tx_message_receive_channel.clone();
        let active_connections_reference = Arc::clone(&self.active_connections_reference);
        tokio::spawn(async move {
            while let Ok(eventwork_packet) = tx_message_receive_channel.recv().await {
                if eventwork_packet.broadcast {
                    match Self::broadcast_message_internal(
                        active_connections_reference.clone(),
                        eventwork_packet,
                    )
                    .await
                    {
                        Ok(_) => {}
                        Err(e) => {
                            error!("Failed to broadcast message: {}", e);
                        }
                    };
                } else {
                    match Self::send_message_internal(
                        active_connections_reference.clone(),
                        eventwork_packet,
                    )
                    .await
                    {
                        Ok(_) => {}
                        Err(e) => {
                            error!("Failed to send message: {}", e);
                        }
                    };
                }
            }
        });

        // Spawn thread for handling network event requests
        let network_event_receive_channel = self.network_event_receive_channel.clone();
        let active_connections_reference_clone = Arc::clone(&self.active_connections_reference);
        let network_event_function_option_reference =
            self.network_event_function_option_reference.clone();
        tokio::spawn(async move {
            while let Ok(network_event) = network_event_receive_channel.recv().await {
                if let NetworkEvent::Disconnected(event_connection_id) = network_event {
                    let connection_id = event_connection_id.id as usize;
                    let mut active_connections = active_connections_reference_clone.lock().await;
                    active_connections.remove(&connection_id);
                    info!("Removed connection with id: {}", connection_id);
                }

                if let Some(network_event_function) = network_event_function_option_reference
                    .lock()
                    .await
                    .as_ref()
                {
                    match network_event_function(network_event).await {
                        Ok(_) => {}
                        Err(e) => {
                            error!(
                                "Failed to handle network in user defined function event: {}",
                                e
                            );
                        }
                    }
                }
            }
        });
    }

    pub async fn on_network_event(
        &mut self,
        function: Arc<dyn Fn(NetworkEvent) -> BoxedFuture + Send + Sync>,
    ) {
        self.network_event_function_option_reference
            .lock()
            .await
            .replace(function);
    }

    async fn send_message_internal(
        active_connections_reference: Arc<Mutex<HashMap<usize, EventWorkConnection>>>,
        eventwork_packet: EventWorkPacket,
    ) -> Result<(), String> {
        let active_connections = active_connections_reference.lock().await;
        match active_connections.get(&eventwork_packet.id) {
            Some(connection) => match connection.send_message(eventwork_packet).await {
                Ok(_) => {}
                Err(e) => {
                    return Err(e.to_string());
                }
            },
            None => {
                return Err(format!(
                    "Failed to find connection with id: {}",
                    eventwork_packet.id
                ));
            }
        }
        Ok(())
    }

    async fn broadcast_message_internal(
        active_connections_reference: Arc<Mutex<HashMap<usize, EventWorkConnection>>>,
        eventwork_packet: EventWorkPacket,
    ) -> Result<(), String> {
        let active_connections = active_connections_reference.lock().await;
        for connection in active_connections.values() {
            match connection.send_message(eventwork_packet.clone()).await {
                Ok(_) => {}
                Err(e) => {
                    return Err(format!("Failed to send message: {}", e));
                }
            }
        }
        Ok(())
    }

    pub async fn handle_new_connection(
        &mut self,
        stream: DuplexStream,
    ) -> Result<Arc<dyn Fn() -> BoxedFuture + Send + Sync>, String> {
        let (write, read) = stream.split();

        let tx_message_send_channel = self.tx_message_send_channel.clone();
        let network_event_send_channel = self.network_event_send_channel.clone();

        let read_reference = Arc::new(Mutex::new(read));
        let write_reference = Arc::new(Mutex::new(write));
        let event_map_reference = Arc::clone(&self.event_map_reference);

        let connection_id = self.connection_counter;

        let new_connection = EventWorkConnection {
            id: connection_id,
            handle_packet_task: Arc::new(move || {
                let read_reference_clone = Arc::clone(&read_reference);
                let event_map_reference_clone = Arc::clone(&event_map_reference);
                let tx_message_send_channel_clone = tx_message_send_channel.clone();
                let connection_id_clone = connection_id;
                Box::pin(async move {
                    while let Some(message) = read_reference_clone.lock().await.next().await {
                        let message_val = match message {
                            Ok(message) => message,
                            Err(e) => {
                                warn!("Hit a non-fatal error: {:?}", e);
                                continue;
                            }
                        };

                        let packet = match message_val.clone() {
                            ws::Message::Binary(binary) => {
                                match bincode::deserialize::<NetworkPacket>(&binary) {
                                    Ok(packet) => packet,
                                    Err(e) => {
                                        error!("Error deserializing packet: {:?}", e);
                                        break;
                                    }
                                }
                            }
                            _ => {
                                error!("Received non-binary message: {}", message_val);
                                break;
                            }
                        };

                        // Handle packet code
                        let function = {
                            let event_map = event_map_reference_clone.lock().await;
                            match event_map.get(&packet.kind) {
                                Some(function) => function.clone(),
                                None => {
                                    error!("Received packet with unknown kind: {}", packet.kind);
                                    break;
                                }
                            }
                        };

                        let eventwork_sender = EventWorkSender {
                            packet_input: packet,
                            message_send_channel: tx_message_send_channel_clone.clone(),
                            connection_id: connection_id_clone,
                        };
                        if let Err(e) = function(eventwork_sender).await {
                            error!("User defined function encountered an error:");
                            error!("{}", e);
                        }
                    }
                    Ok(())
                })
            }),
            write_reference: Arc::clone(&write_reference),
        };

        self.connection_counter += 1;

        let mut active_connections = self.active_connections_reference.lock().await;

        let future_function = new_connection.handle_packet_task.clone();
        info!("Added a new connection with id: {}", new_connection.id);
        match network_event_send_channel
            .clone()
            .send(NetworkEvent::Connected(ConnectionId {
                id: new_connection.id as u32,
            }))
            .await
        {
            Ok(_) => {}
            Err(e) => {
                error!("Failed to send new connection message: {}", e);
            }
        }

        let new_connection_id = new_connection.id;

        active_connections.insert(new_connection.id, new_connection);

        Ok(Arc::new(move || {
            let future_function_clone = future_function.clone();
            let network_event_send_channel_clone = network_event_send_channel.clone();
            let new_connection_id_clone = new_connection_id;
            Box::pin(async move {
                future_function_clone().await?;
                info!("Connection with id: {} has closed, sending disconnect message", new_connection_id_clone);
                match network_event_send_channel_clone
                    .send(NetworkEvent::Disconnected(ConnectionId {
                        id: new_connection_id_clone as u32,
                    }))
                    .await
                {
                    Ok(_) => {}
                    Err(e) => {
                        error!("Failed to send close connection message: {}", e);
                    }
                }
                Ok(())
            })
        }))
    }

    pub async fn register_message<T>(
        &self,
        input_function: EventHandleFunctionStore,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        T: NetworkMessage,
    {
        let mut event_map = self.event_map_reference.lock().await;
        event_map.insert(String::from(T::NAME), input_function);
        Ok(())
    }
}

impl EventWorkSendMessages for EventWorkServer {
    async fn send_message<T>(&self, connection_id: usize, message: T) -> Result<(), String>
    where
        T: NetworkMessage,
    {
        let eventwork_packet =
            EventWorkSender::from_message_to_packet::<T>(connection_id, false, message);

        Self::send_message_internal(
            Arc::clone(&self.active_connections_reference),
            eventwork_packet,
        )
        .await
    }

    async fn broadcast<T>(&self, message: T) -> Result<(), String>
    where
        T: NetworkMessage,
    {
        let eventwork_packet = EventWorkSender::from_message_to_packet::<T>(0, true, message);

        Self::send_message_internal(
            Arc::clone(&self.active_connections_reference),
            eventwork_packet,
        )
        .await
    }
}
