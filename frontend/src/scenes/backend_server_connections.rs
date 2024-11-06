use bevy::prelude::*;
use bevy_http_client::prelude::*;
use serde::Deserialize;

fn handle_error(mut ev_error: EventReader<HttpResponseError>) {
    for error in ev_error.read() {
        error!("Error doing request{}", error.err);
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct User {
    name: String,
    age: u8,
    alive: bool,
}

pub fn send_ip_request(mut ev_request: EventWriter<TypedRequest<User>>) {
    info!("Sending ip request");
    ev_request.send(
        HttpClient::new()
            .get("http://localhost:8000/api/test")
            .with_type::<User>(),
    );
}

fn handle_ip_response(mut ev_response: EventReader<TypedResponse<User>>) {
    for response in ev_response.read() {
        info!("ip: {}", response.name);
    }
}

pub fn add_backend_server_connections(app: &mut App) {
        app.register_request_type::<User>()
        .add_systems(Update, (handle_ip_response, handle_error));
}
