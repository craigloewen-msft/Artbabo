use bevy::prelude::*;
use bevy_client_server_events::{
    client::{ConnectToServer, ReceiveFromServer, SendToServer},
    client_server_events_plugin,
    server::{ReceiveFromClient, SendToClient, StartServer},
    NetworkConfig,
};

pub mod backend_responses;
use backend_responses::*;

pub fn send_random_room_creation_request(
    mut room_creation_sender: EventWriter<SendToServer<RoomCreationRequest>>,
    username: &str,
) {
    info!("Sending random room creationg request");

    let room_creation_request = backend_responses::RoomCreationRequest {
        username: username.to_string(),
        room_id: "".to_string(),
    };

    room_creation_sender.send(SendToServer {
        content: room_creation_request,
    });
}

fn setup_client(mut connect_to_server: EventWriter<ConnectToServer>) {
    connect_to_server.send(ConnectToServer::default()); // Connects to 127.0.0.1:5000 by default.
}

fn update_client(mut room_creation_response: EventReader<ReceiveFromServer<RoomCreationResponse>>) {
    for response in room_creation_response.read() {
        println!("Server Response: {:?}", response.content);
    }
}

pub fn add_backend_server_connections(app: &mut App) {
    client_server_events_plugin!(
        app,
        RoomCreationRequest => NetworkConfig::default(),
        RoomCreationResponse => NetworkConfig::default()
    );
    app.add_systems(Startup, setup_client)
        .add_systems(Update, update_client);
}
