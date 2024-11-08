use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy::tasks::TaskPoolBuilder;
use bevy::{app::ScheduleRunnerPlugin, log::Level};
use bevy_eventwork::{AppNetworkMessage, EventworkRuntime, NetworkData, NetworkEvent};
use serde::{Deserialize, Serialize};

use bevy_eventwork::{
    managers::network_request::{AppNetworkResponseMessage, RequestMessage, Requester, Response},
    tcp::TcpProvider,
    ConnectionId, NetworkMessage,
};

use bevy_eventwork_mod_websockets::*;

mod backend_responses;

use backend_responses::*;

fn main() {
    info!("Building app");
    let mut app = App::new();

    app.add_plugins(MinimalPlugins)
        .add_plugins(LogPlugin {
            level: Level::DEBUG,
            filter: "wgpu=error,bevy_render=info,bevy_ecs=trace".to_string(),
            custom_layer: |_| None,
        })
        .add_plugins(bevy_eventwork::EventworkPlugin::<
            WebSocketProvider,
            bevy::tasks::TaskPool,
        >::default())
        .listen_for_message::<RoomCreationResponse, WebSocketProvider>()
        .insert_resource(EventworkRuntime(
            TaskPoolBuilder::new().num_threads(2).build(),
        ))
        .insert_resource(NetworkSettings::default())
        .add_systems(Update, (handle_incoming_messages, handle_network_events))
        .run();
}

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
