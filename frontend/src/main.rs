use bevy::{prelude::*, render::camera::ScalingMode, window::PrimaryWindow};
use bevy_egui::EguiPlugin;

mod scenes;
use scenes::add_scenes;
mod resources;
use server_responses::*;

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
        button_submitted: false,
    })
    .insert_resource(resources::CurrentPlayerData { player_id: 0 })
    .insert_resource(PromptInfoDataList::default())
    .insert_resource(RoundTimer(Timer::from_seconds(5.0, TimerMode::Once)))
    .add_systems(Startup, setup)
    .add_systems(Update, update_camera_scaling)
    .add_systems(Update, tick_timers);
    // .add_systems(Update, handle_timer_events);

    add_scenes(&mut app);

    app.run();
}

fn setup(mut commands: Commands) {
    let mut camera_bundle = Camera2dBundle::default();
    camera_bundle.projection.scaling_mode = ScalingMode::FixedVertical(10.);
    commands.spawn(camera_bundle);
}

fn update_camera_scaling(
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut query: Query<&mut OrthographicProjection>,
) {
    for mut window in windows.iter_mut() {
        let aspect_ratio = window.width() / window.height();

        for mut projection in query.iter_mut() {
            if aspect_ratio > 1.0 {
                projection.scaling_mode = ScalingMode::FixedVertical(10.0);
            } else {
                projection.scaling_mode = ScalingMode::FixedHorizontal(10.0);
            }
        }
    }
}

fn tick_timers(time: Res<Time>, mut round_timer: ResMut<RoundTimer>) {
    round_timer.0.tick(time.delta());
}

// fn handle_timer_events(mut query: Query<&mut RoundTimer>) {
//     for timer in query.iter_mut() {
//         if timer.0.finished() {
//             info!("Timer finished!");
//         }
//     }
// }
