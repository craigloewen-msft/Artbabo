use bevy::{log::LogPlugin, tasks::TaskPool};
use bevy::prelude::*;
use bevy::tasks::TaskPoolBuilder;
use bevy::log::Level;
use bevy_eventwork::{AppNetworkMessage, EventworkRuntime, Network, NetworkData, NetworkEvent};

use std::net::{IpAddr, SocketAddr};
use core::net::Ipv4Addr;

use bevy_eventwork_mod_websockets::*;

extern crate server_responses;
use server_responses::*;

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
            TaskPool,
        >::default())
        .listen_for_message::<RoomCreationResponse, WebSocketProvider>()
        .insert_resource(EventworkRuntime(
            TaskPoolBuilder::new().num_threads(2).build(),
        ))
        .insert_resource(NetworkSettings::default())
        .add_systems(Startup, setup_networking)
        .add_systems(Update, (handle_connection_events))
        .run();
}

// at any time.
fn setup_networking(
    mut net: ResMut<Network<WebSocketProvider>>,
    settings: Res<NetworkSettings>,
    task_pool: Res<EventworkRuntime<TaskPool>>,
) {
    let ip_address = "127.0.0.1".parse().expect("Could not parse ip address");

    info!("Address of the server: {}", ip_address);

    let _socket_address = SocketAddr::new(ip_address, 9999);

    match net.listen(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
        &task_pool.0,
        &settings,
    ) {
        Ok(_) => (),
        Err(err) => {
            error!("Could not start listening: {}", err);
            panic!();
        }
    }

    info!("Started listening for new connections!");
}

fn handle_connection_events(
    mut commands: Commands,
    net: Res<Network<WebSocketProvider>>,
    mut network_events: EventReader<NetworkEvent>,
) {
    for event in network_events.read() {
        if let NetworkEvent::Connected(conn_id) = event {
            info!("New player connected: {}", conn_id);

            net.broadcast(RoomCreationResponse {
                success: true,
            });
        }
    }
}