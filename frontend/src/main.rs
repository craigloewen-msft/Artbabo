use bevy::{prelude::*, render::camera::ScalingMode};
use bevy_egui::EguiPlugin;

mod scenes;
use scenes::{GameState, Images};
mod resources;

fn main() {
    let room_creation_scenes = scenes::get_intro_system_methods();
    let intro_systems = scenes::get_intro_system_methods();

    App::new()
        .add_plugins((
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
        .init_resource::<Images>()
        .insert_resource(resources::PlayerSettings {
            username: String::new(),
        })
        .init_state::<GameState>()
        .add_systems(Startup, setup)
        .add_systems(Update, room_creation_scenes.run_if(in_state(GameState::RoomCreation)))
        .add_systems(Update, intro_systems)
        .run();
}


fn setup(mut commands: Commands) {
    let mut camera_bundle = Camera2dBundle::default();
    camera_bundle.projection.scaling_mode = ScalingMode::FixedVertical(10.);
    commands.spawn(camera_bundle);
}