use bevy::{prelude::*, render::camera::ScalingMode, window::PrimaryWindow};
use bevy_egui::{EguiContexts, EguiPlugin};
use bevy_egui::{EguiSettings, EguiContext};

mod scenes;
use scenes::add_scenes;
mod resources;
use server_responses::*;

const SCREEN_SCALING_SIZE: f32 = 100.0;

fn main() {
    let mut app = App::new();

    app.add_plugins((
        DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                // fill the entire browser window
                fit_canvas_to_parent: true,
                // don't hijack keyboard shortcuts like F5, F6, F12, Ctrl+R etc.
                prevent_default_event_handling: false,
                ime_enabled: true,
                ..default()
            }),
            ..default()
        }),
        EguiPlugin,
    ))
    .insert_resource(resources::PlayerSettings {
        username: String::new(),
    })
    .insert_resource(resources::CurrentPlayerData { player_id: 0 })
    .insert_resource(resources::FrontEndPromptList::default())
    .insert_resource(RoundEndInfo::default())
    .insert_resource(GameEndInfo::default())
    .insert_resource(RoundTimer(Timer::from_seconds(5.0, TimerMode::Once)))
    .add_systems(Startup, setup)
    .add_systems(Update, update_camera_scaling)
    .add_systems(Update, tick_timers)
    .add_systems(Update, remove_finished_notifications);
    // .add_systems(Update, handle_timer_events);

    add_scenes(&mut app);

    app.run();
}

fn setup(mut commands: Commands) {
    let camera = Camera2d::default();
    commands.spawn(camera);
}

fn update_camera_scaling(
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut query: Query<&mut OrthographicProjection>,
    mut contexts: EguiContexts,
) {
    for window in windows.iter_mut() {
        let aspect_ratio = window.width() / window.height();

        // Camera scaling
        for mut projection in query.iter_mut() {
            if aspect_ratio > 1.0 {
                projection.scaling_mode = ScalingMode::FixedVertical {
                    viewport_height: SCREEN_SCALING_SIZE,
                };
            } else {
                projection.scaling_mode = ScalingMode::FixedVertical {
                    viewport_height: SCREEN_SCALING_SIZE,
                };
            }
        }
    }
}

fn tick_timers(
    time: Res<Time>,
    mut round_timer: ResMut<RoundTimer>,
    mut notification_timers: Query<&mut GamePlayerNotification>,
) {
    round_timer.0.tick(time.delta());

    for mut game_notification in notification_timers.iter_mut() {
        game_notification.timer.tick(time.delta());
    }
}

fn remove_finished_notifications(
    mut commands: Commands,
    query: Query<(Entity, &GamePlayerNotification)>,
) {
    for (entity, game_notification) in query.iter() {
        if game_notification.timer.finished() {
            commands.entity(entity).despawn();
        }
    }
}

// fn handle_timer_events(mut query: Query<&mut RoundTimer>) {
//     for timer in query.iter_mut() {
//         if timer.0.finished() {
//             info!("Timer finished!");
//         }
//     }
// }
