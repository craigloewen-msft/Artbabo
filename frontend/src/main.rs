use bevy::{prelude::*, render::camera::ScalingMode};
use bevy_egui::{
    egui::{self, Align2, Color32, FontId, RichText},
    EguiContexts, EguiPlugin,
};

mod scenes;
use scenes::{GameState, Images};

fn main() {
    let room_creation_scenes = scenes::get_intro_system_methods();
    let loading_systems = scenes::get_loading_system_methods();

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
        .insert_resource(ClearColor(Color::srgb(0.53, 0.53, 0.53)))
        .init_resource::<Images>()
        .init_state::<GameState>()
        .add_systems(Startup, setup)
        .add_systems(Update, room_creation_scenes.run_if(in_state(GameState::RoomCreation)))
        .add_systems(Update, loading_systems)
        .run();
}


fn setup(mut contexts: EguiContexts, mut commands: Commands) {
    let mut camera_bundle = Camera2dBundle::default();
    camera_bundle.projection.scaling_mode = ScalingMode::FixedVertical(10.);
    commands.spawn(camera_bundle);
}