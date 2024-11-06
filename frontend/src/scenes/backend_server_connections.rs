use bevy::prelude::*;
use bevy_http_client::prelude::*;
use serde::Deserialize;

fn handle_error(mut ev_error: EventReader<HttpResponseError>) {
    for error in ev_error.read() {
        error!("Error doing request{}", error.err);
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct IpInfo {
    pub ip: String,
}

pub fn send_ip_request(mut ev_request: EventWriter<TypedRequest<IpInfo>>) {
    info!("Sending ip request");
    ev_request.send(
        HttpClient::new()
            .get("https://api.ipify.org?format=json")
            .with_type::<IpInfo>(),
    );
}

fn handle_ip_response(mut ev_response: EventReader<TypedResponse<IpInfo>>) {
    for response in ev_response.read() {
        info!("ip: {}", response.ip);
    }
}

pub fn add_backend_server_connections(app: &mut App) {
        app.register_request_type::<IpInfo>()
        .add_systems(Update, (handle_ip_response, handle_error));
}
