use bevy::prelude::*;
use bevy_http_client::prelude::*;

pub mod backend_responses;

fn handle_error(mut ev_error: EventReader<HttpResponseError>) {
    for error in ev_error.read() {
        error!("Error doing request{}", error.err);
    }
}

pub fn send_random_room_creation_request(mut ev_request: EventWriter<TypedRequest<backend_responses::RoomCreationResponse>>, username: &str) {
    info!("Sending random room creationg request");

    let room_creation_request = backend_responses::RoomCreationRequest {
        username: username.to_string(),
        room_id: "".to_string(),
    };

    ev_request.send(
        HttpClient::new()
            .post("http://localhost:8000/api/join_random_room")
            .json(&room_creation_request)
            .with_type::<backend_responses::RoomCreationResponse>(),
    );
}

fn handle_room_creation_response(mut ev_response: EventReader<TypedResponse<backend_responses::RoomCreationResponse>>) {
    for response in ev_response.read() {
        info!("Room creation success: {}", response.success);
    }
}

pub fn add_backend_server_connections(app: &mut App) {
        app.register_request_type::<backend_responses::RoomCreationResponse>()
        .add_systems(Update, (handle_room_creation_response, handle_error));
}
