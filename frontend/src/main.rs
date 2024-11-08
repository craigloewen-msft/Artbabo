use bevy::{prelude::*, render::camera::ScalingMode, tasks::TaskPool};
use bevy_egui::EguiPlugin;
use bevy_eventwork::{EventworkRuntime, Network};
use core::net::Ipv4Addr;
use std::net::{IpAddr, SocketAddr};
use url::*;

mod scenes;
use bevy_eventwork_mod_websockets::{NetworkSettings, WebSocketProvider};
use scenes::add_scenes;
mod resources;

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
    ))
    .insert_resource(resources::PlayerSettings {
        username: String::new(),
    })
    .add_systems(Startup, setup);

    add_scenes(&mut app);

    app.run();
}

fn setup(
    mut commands: Commands,
    net: ResMut<Network<WebSocketProvider>>,
    task_pool: Res<EventworkRuntime<TaskPool>>,
    settings: Res<NetworkSettings>
) {
    let mut camera_bundle = Camera2dBundle::default();
    camera_bundle.projection.scaling_mode = ScalingMode::FixedVertical(10.);
    commands.spawn(camera_bundle);

    info!("Setting up networking and wanting to connect");
    net.connect(
        url::Url::parse("ws://127.0.0.1:8081").unwrap(),
        &task_pool.0,
        &settings,
    );
}
