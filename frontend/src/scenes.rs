use std::time::Duration;
use bevy_http_client::prelude::*;

use bevy::{
    ecs::{system::SystemState, world::CommandQueue},
    prelude::*,
    tasks::{block_on, futures_lite::future, AsyncComputeTaskPool, Task},
};
use bevy_egui::{
    egui::{self, Align2},
    EguiContexts,
};

use crate::resources::PlayerSettings;
mod backend_server_connections;
use backend_server_connections::*;

// === GameState enum ===
#[derive(States, Clone, Eq, PartialEq, Debug, Hash, Default)]
pub enum GameState {
    #[default]
    Intro,
    RoomCreation,
}

// === Assets ===
#[derive(Resource)]
pub struct Images {
    dog: Handle<Image>,
}

impl FromWorld for Images {
    fn from_world(world: &mut World) -> Self {
        let asset_server = world.get_resource_mut::<AssetServer>().unwrap();
        Self {
            dog: asset_server.load("dog.png"),
        }
    }
}

// === Intro scenes ===

pub fn draw_intro_ui(
    mut contexts: EguiContexts,
    mut input_text: Local<String>,
    mut player_settings: ResMut<PlayerSettings>,
    ev_request: EventWriter<TypedRequest<backend_server_connections::backend_responses::RoomCreationResponse>>,
) {
    if player_settings.username != "" {
        // Room option select screen
        egui::Area::new("welcome_area".into())
            .anchor(Align2::CENTER_TOP, (0., 200.))
            .show(contexts.ctx_mut(), |ui| {
                ui.vertical(|ui| {
                    ui.label("Select a room");
                    ui.horizontal(|ui| {
                        let random_room = ui.button("Join random room");
                        let private_room = ui.button("Join private room");

                        if random_room.clicked() {
                            info!("Starting request to server");

                            send_random_room_creation_request(ev_request, player_settings.username.as_str());
                        }
                        if private_room.clicked() {
                            info!("Joining private room");
                        }
                    });
                });
            });
    } else {
        // Username input screen
        egui::Area::new("input_area".into())
            .anchor(Align2::CENTER_TOP, (0., 200.))
            .show(contexts.ctx_mut(), |ui| {
                ui.vertical(|ui| {
                    ui.label("Enter username");
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut *input_text);
                        if ui.button("Submit").clicked() {
                            warn!("Text input: {}", *input_text);
                            player_settings.username = input_text.clone();
                        }
                    });
                });
            });
    }
}

pub fn add_intro_scenes(app: &mut App) {
    app.init_resource::<Images>()
        .add_systems(Update, draw_intro_ui);
}

// === ETC ===

pub fn draw_image(
    mut contexts: EguiContexts,
    mut is_initialized: Local<bool>,
    images: Res<Images>,
    mut rendered_texture_id: Local<egui::TextureId>,
) {
    if !*is_initialized {
        *is_initialized = true;
        *rendered_texture_id = contexts.add_image(images.dog.clone_weak());
    }

    egui::Area::new("example_area2".into())
        .anchor(Align2::CENTER_TOP, (0., 100.))
        .show(contexts.ctx_mut(), |ui| {
            let added_button = ui.add(egui::ImageButton::new(egui::widgets::Image::new(
                egui::load::SizedTexture::new(*rendered_texture_id, [256.0, 256.0]),
            )));
            if added_button.clicked() {
                println!("Image clicked!");
            }
        });
}

// === Main add logic ===

pub fn add_scenes(app: &mut App) {
    app.init_state::<GameState>();
    add_intro_scenes(app);
    add_backend_server_connections(app);
}
