use bevy::{prelude::*, render::camera::ScalingMode, time::common_conditions::on_timer};
use bevy_egui::EguiPlugin;
use bevy_http_client::prelude::*;

mod scenes;
use scenes::{add_scenes, GameState, Images};
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
            HttpClientPlugin,
        ))
        .insert_resource(resources::PlayerSettings {
            username: String::new(),
        })
        .add_systems(Startup, setup);

    add_scenes(&mut app);

    app.run();
}

fn setup(mut commands: Commands) {
    let mut camera_bundle = Camera2dBundle::default();
    camera_bundle.projection.scaling_mode = ScalingMode::FixedVertical(10.);
    commands.spawn(camera_bundle);
}