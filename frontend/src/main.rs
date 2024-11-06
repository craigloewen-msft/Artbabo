use bevy::{prelude::*, render::camera::ScalingMode, time::common_conditions::on_timer};
use bevy_egui::EguiPlugin;
use bevy_http_client::prelude::*;
use serde::Deserialize;

mod scenes;
use scenes::{add_intro_scene_logic, GameState, Images};
mod resources;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct PostInfo {
    pub userId: u32,
    pub id: u32,
    pub title: String,
    pub body: String,
}

impl PostInfo {
    pub fn user_id(&self) -> u32 {
        self.userId
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct IpInfo {
    pub ip: String,
}

fn main() {
    let mut app = App::new();

    app.add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    // fill the entire browser window
                    fit_canvas_to_parent: true,
                    // don't hijack keyboard shortcuts like F5, F6, F12, Ctrl+R etc.
                    prevent_default_event_handling: false,
                    ..default()
                }),
                ..default()
            }),
            EguiPlugin,
            HttpClientPlugin,
        ))
        .insert_resource(resources::PlayerSettings {
            username: String::new(),
        })
        .init_state::<GameState>()
        .add_systems(Startup, setup)
        .register_request_type::<PostInfo>()
        .register_request_type::<IpInfo>()
        .add_systems(
            Update,
            (
                send_request.run_if(on_timer(std::time::Duration::from_secs(1))),
                send_ip_request.run_if(on_timer(std::time::Duration::from_secs(3))),
            ),
        )
        .add_systems(Update, (handle_response, handle_ip_response, handle_error));

    add_intro_scene_logic(&mut app);

    app.run();
}

fn setup(mut commands: Commands) {
    let mut camera_bundle = Camera2dBundle::default();
    camera_bundle.projection.scaling_mode = ScalingMode::FixedVertical(10.);
    commands.spawn(camera_bundle);
}

fn send_request(mut ev_request: EventWriter<TypedRequest<PostInfo>>) {
    info!("Sending request");
    ev_request.send(
        HttpClient::new()
            .get("https://jsonplaceholder.typicode.com/posts/1")
            .with_type::<PostInfo>(),
    );
}

fn send_ip_request(mut ev_request: EventWriter<TypedRequest<IpInfo>>) {
    info!("Sending ip request");
    ev_request.send(
        HttpClient::new()
            .get("https://api.ipify.org?format=json")
            .with_type::<IpInfo>(),
    );
}

fn handle_response(mut ev_response: EventReader<TypedResponse<PostInfo>>) {
    for response in ev_response.read() {
        info!("user_id: {}", response.user_id());
    }
}

fn handle_ip_response(mut ev_response: EventReader<TypedResponse<IpInfo>>) {
    for response in ev_response.read() {
        info!("ip: {}", response.ip);
    }
}

fn handle_error(mut ev_error: EventReader<HttpResponseError>) {
    for error in ev_error.read() {
        error!("Error doing request{}", error.err);
    }
}
