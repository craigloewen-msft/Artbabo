use bevy::{prelude::*, tasks::TaskPoolBuilder};

use bevy_eventwork::{AppNetworkMessage, EventworkRuntime, NetworkData, NetworkEvent};
use bevy_eventwork_mod_websockets::*;
pub mod backend_responses;
use backend_responses::*;

fn handle_incoming_messages(mut new_messages: EventReader<NetworkData<RoomCreationResponse>>) {
    for new_message in new_messages.read() {
        info!("Received new message: {:?}", new_message);
    }
}

fn handle_network_events(mut new_network_events: EventReader<NetworkEvent>) {
    for event in new_network_events.read() {
        info!("Received event");
        match event {
            NetworkEvent::Connected(_) => {
                info!("Connected to server!");
            }

            NetworkEvent::Disconnected(_) => {
                info!("Disconnected from server!");
            }
            NetworkEvent::Error(err) => {
                info!("Error!");
            }
        }
    }
}


pub fn add_backend_server_connections(app: &mut App) {
    info!("Building backend server connections");
    app.add_plugins(bevy_eventwork::EventworkPlugin::<
            WebSocketProvider,
            bevy::tasks::TaskPool,
        >::default())
        .listen_for_message::<RoomCreationResponse, WebSocketProvider>()
        .insert_resource(EventworkRuntime(
            TaskPoolBuilder::new().num_threads(2).build(),
        ))
        .insert_resource(NetworkSettings::default())
         .add_systems(Update, (handle_incoming_messages, handle_network_events));
    info!("Server connections built");
}
