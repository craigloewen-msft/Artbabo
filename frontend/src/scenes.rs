use bevy::{
    prelude::*,
    render::{
        render_asset::RenderAssetUsages,
        render_resource::{Extent3d, TextureDimension, TextureFormat},
    },
};
use bevy_egui::{
    egui::{self, Align2},
    EguiContexts,
};
use bevy_eventwork::Network;
use bevy_eventwork_mod_websockets::WebSocketProvider;

use crate::resources::*;
mod backend_server_connections;
use backend_server_connections::*;

use server_responses::*;

use reqwest::get;

use bevy_async_task::{AsyncTaskRunner, AsyncTaskStatus};

use ::image::ImageReader;
use std::io::Cursor;

// === Assets ===
#[derive(Resource, Debug, Default)]
pub struct Images {
    current_bid_image: Handle<Image>,
}

// impl FromWorld for Images {
//     fn from_world(world: &mut World) -> Self {
//         let asset_server = world.get_resource_mut::<AssetServer>().unwrap();
//         Self {
//             dog: asset_server.load("dog.png"),
//         }
//     }
// }

// === Helper functions ===

fn draw_round_ui(
    contexts: &mut EguiContexts,
    room_state: &RoomState,
    round_timer: &RoundTimer,
    current_player_data: &CurrentPlayerData,
    net: &Network<WebSocketProvider>,
) {
    egui::Area::new("round_area".into())
        .anchor(Align2::CENTER_TOP, (0., 0.))
        .show(contexts.ctx_mut(), |ui| {
            ui.vertical(|ui| {
                // Show timer information at top
                ui.label("Time left: ");
                ui.label(format!("{:.2}", round_timer.0.remaining_secs()));

                ui.label("Current art: ");
                ui.label(room_state.current_art_bid.image_url.clone());
                ui.label(format!(
                    "Current bid: {}",
                    room_state.current_art_bid.max_bid
                ));
                ui.label(format!(
                    "Bid increase: {}",
                    room_state.current_art_bid.bid_increase_amount
                ));
                ui.label(format!(
                    "Owner: {}",
                    room_state.current_art_bid.owner_player_id
                ));

                let button = ui.add_enabled(true, egui::Button::new("Bid"));

                if button.clicked() {
                    send_bid_action(current_player_data.player_id, room_state.room_id, net);
                }

                // Add button end round
                if ui.button("End Round").clicked() {
                    send_end_round_action(current_player_data.player_id, room_state.room_id, net);
                }
            });
        });

    egui::Area::new("round_1_area_bottom".into())
        .anchor(Align2::CENTER_BOTTOM, (0., 0.))
        .show(contexts.ctx_mut(), |ui| {
            ui.horizontal(|ui| {
                for player in room_state.players.iter() {
                    ui.horizontal(|ui| {
                        ui.label(player.username.clone());
                        ui.label(format!("Money: {}", player.money));
                    });
                }
            });
        });
}

// === Intro scenes ===

pub fn draw_intro_ui(
    mut contexts: EguiContexts,
    mut input_text: Local<String>,
    mut player_settings: ResMut<PlayerSettings>,
    net: Res<Network<WebSocketProvider>>,
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

                            send_random_room_request(player_settings.username.as_str(), net);
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
    // app.init_resource::<Images>()
    app.add_systems(Update, draw_intro_ui.run_if(in_state(GameState::Intro)));
}

// === Waiting room scenes ===

pub fn draw_waiting_room_ui(
    mut contexts: EguiContexts,
    mut query: Query<&mut RoomState>,
    player_settings: ResMut<PlayerSettings>,
    net: Res<Network<WebSocketProvider>>,
) {
    // If an entity with room state exists, update it
    let room_state = query.get_single_mut().unwrap();

    // For each player in the room, display their username and money
    egui::Area::new("waiting_room_area".into())
        .anchor(Align2::CENTER_TOP, (0., 200.))
        .show(contexts.ctx_mut(), |ui| {
            ui.vertical(|ui| {
                ui.label("Waiting room");
                for player in room_state.players.iter() {
                    ui.horizontal(|ui| {
                        ui.label(player.username.clone());
                        ui.label(format!("Money: {}", player.money));
                    });
                }

                // Check if the current player is the host (player in position 0)
                if let Some(host) = room_state.players.get(0) {
                    // TODO: Do an ID based check instead of username check
                    if host.username == player_settings.username {
                        // Replace with actual current player username check
                        let button = ui.add_enabled(
                            room_state.players.len() > 1,
                            egui::Button::new("Start Game"),
                        );
                        if button.clicked() {
                            send_start_game_request(room_state.room_id, net);
                        }
                    }
                }
            });
        });
}

pub fn add_waiting_room_scenes(app: &mut App) {
    app.add_systems(
        Update,
        draw_waiting_room_ui.run_if(in_state(GameState::WaitingRoom)),
    );
}

// === ImageCreation scenes ===

pub fn draw_image_creation_ui(
    mut contexts: EguiContexts,
    mut player_settings: ResMut<PlayerSettings>,
    mut prompt_info_data: ResMut<PromptInfoDataList>,
    round_timer: ResMut<RoundTimer>,
    net: Res<Network<WebSocketProvider>>,
) {
    egui::Area::new("image_creation_area".into())
        .anchor(Align2::CENTER_TOP, (0., 0.))
        .show(contexts.ctx_mut(), |ui| {
            ui.vertical(|ui| {
                // Show timer information at top
                ui.label("Time left: ");
                ui.label(format!("{:.2}", round_timer.0.remaining_secs()));

                ui.label("Fill out these prompts");
                ui.add_space(10.0); // Add some space between the label and the text box

                for index in 0..prompt_info_data.prompt_list.len() {
                    ui.vertical(|ui| {
                        ui.label(prompt_info_data.prompt_list[index].prompt_text.clone());
                        ui.text_edit_multiline(
                            &mut prompt_info_data.prompt_list[index].prompt_answer,
                        );
                        ui.add_space(10.0); // Add some space between the label and the text box
                    });
                }

                ui.add_space(15.0); // Add some space between the label and the text box

                let button = ui.add_enabled(
                    !player_settings.button_submitted,
                    egui::Button::new("Start Game"),
                );

                if button.clicked() {
                    player_settings.button_submitted = true;
                    // Handle the submit button click event
                    send_completed_prompts(prompt_info_data.clone(), net);
                    // Add your submit logic here
                }
            });
        });
}

pub fn on_enter_image_creation(mut commands: Commands, mut round_timer: ResMut<RoundTimer>) {
    // Reset the button submitted state
    commands.insert_resource(PlayerSettings {
        username: String::new(),
        button_submitted: false,
    });

    // Create a new round timer
    *round_timer = RoundTimer(Timer::from_seconds(IMAGE_CREATION_TIME, TimerMode::Once));
}

pub fn add_image_creation_scenes(app: &mut App) {
    app.add_systems(
        Update,
        draw_image_creation_ui.run_if(in_state(GameState::ImageCreation)),
    )
    .add_systems(OnEnter(GameState::ImageCreation), on_enter_image_creation);
}

// === Image generation scenes ===

pub fn draw_image_generation_ui(mut contexts: EguiContexts) {
    // Draw a 'please wait message'
    egui::Area::new("image_generation_area".into())
        .anchor(Align2::CENTER_TOP, (0., 0.))
        .show(contexts.ctx_mut(), |ui| {
            ui.label("Please wait for all images to be generated");
        });
}

pub fn add_image_generation_scenes(app: &mut App) {
    app.add_systems(
        Update,
        draw_image_generation_ui.run_if(in_state(GameState::ImageGeneration)),
    );
}

// === Round 1 scenes ===

pub fn draw_round_1_ui(
    mut contexts: EguiContexts,
    round_timer: ResMut<RoundTimer>,
    mut query: Query<&mut RoomState>,
    current_player_data: Res<CurrentPlayerData>,
    net: Res<Network<WebSocketProvider>>,
    mut task_executor: AsyncTaskRunner<Option<Image>>,
    asset_server: ResMut<AssetServer>,
    mut images: ResMut<Images>,
    mut commands: Commands,
    mut is_image_loaded: Local<bool>,
) {
    let room_state = query.get_single_mut().unwrap();

    match task_executor.poll() {
        AsyncTaskStatus::Idle => {
            if !*is_image_loaded {
                let url = room_state.current_art_bid.image_url.clone();
                // Spawn an async task to download the image
                task_executor.start(async move {
                    info!("Started image loading");
                    let response = get(&url).await.unwrap();
                    let bytes = response.bytes().await.unwrap();

                    // Decode the image
                    let reader = ImageReader::new(Cursor::new(bytes))
                        .with_guessed_format()
                        .unwrap(); // Correct use of Result
                    let image = reader.decode().unwrap(); // Decode the image from the reader
                    let rgba_image = image.to_rgba8();
                    let (width, height) = rgba_image.dimensions();
                    info!("Image dimensions: {}x{}", width, height);

                    // Create a Bevy texture
                    let texture = Image::new_fill(
                        Extent3d {
                            width,
                            height,
                            depth_or_array_layers: 1,
                        },
                        TextureDimension::D2,
                        &rgba_image,
                        TextureFormat::Rgba8UnormSrgb,
                        RenderAssetUsages::RENDER_WORLD,
                    );

                    info!("Finished image loading");

                    Some(texture)
                });
            }
        }
        AsyncTaskStatus::Pending => {}
        AsyncTaskStatus::Finished(returned_image_option) => {
            if let Some(returned_image) = returned_image_option {
                let image_handle = asset_server.add(returned_image.clone());
                images.current_bid_image = image_handle.clone();
                *is_image_loaded = true;
                // Spawn entity with this image
                commands.spawn(SpriteBundle {
                    texture: images.current_bid_image.clone(),
                    sprite: Sprite {
                        custom_size: Some(Vec2::new(85., 85.)),
                        ..default()
                    },
                    ..Default::default()
                });
            }
        }
    }

    egui::Area::new("round_area".into())
        .anchor(Align2::CENTER_TOP, (0., 0.))
        .show(contexts.ctx_mut(), |ui| {
            ui.horizontal(|ui| {
                // Show timer information at top
                ui.vertical(|ui| {
                    ui.label("Time left: ");
                    ui.label(format!("{:.2}", round_timer.0.remaining_secs()));
                });

                ui.vertical(|ui| {
                    ui.label("Current bid:");
                    ui.label(format!("{}", room_state.current_art_bid.max_bid));
                });

                let current_bid_owner = room_state
                    .players
                    .iter()
                    .find(|player| player.id == room_state.current_art_bid.owner_player_id);

                ui.vertical(|ui| {
                    ui.label("Current owner:");
                    if let Some(owner) = current_bid_owner {
                        if room_state.current_art_bid.max_bid > 0 {
                            ui.label(format!("{}", owner.username));
                        } else {
                            ui.label("No owner");
                        }
                    }
                });

                let button = ui.add_enabled(true, egui::Button::new("Bid"));

                if button.clicked() {
                    send_bid_action(current_player_data.player_id, room_state.room_id, &net);
                }

                // Add button end round
                if ui.button("End Round").clicked() {
                    send_end_round_action(current_player_data.player_id, room_state.room_id, &net);
                }
            });
        });

    egui::Area::new("round_1_area_bottom".into())
        .anchor(Align2::CENTER_BOTTOM, (0., 0.))
        .show(contexts.ctx_mut(), |ui| {
            ui.horizontal(|ui| {
                for player in room_state.players.iter() {
                    ui.horizontal(|ui| {
                        ui.label(player.username.clone());
                        ui.label(format!("Money: {}", player.money));
                    });
                }
            });
        });
}

pub fn on_enter_round_1(mut round_timer: ResMut<RoundTimer>) {
    // Create a new round timer
    *round_timer = RoundTimer(Timer::from_seconds(ROUND_1_TIME, TimerMode::Once));
}

pub fn add_round_1_scenes(app: &mut App) {
    app.add_systems(Update, draw_round_1_ui.run_if(in_state(GameState::Round1)))
        .add_systems(OnEnter(GameState::Round1), on_enter_round_1);
}

// === Round 2 scenes ===

pub fn draw_round_2_ui(
    mut contexts: EguiContexts,
    round_timer: ResMut<RoundTimer>,
    mut query: Query<&mut RoomState>,
    current_player_data: Res<CurrentPlayerData>,
    net: Res<Network<WebSocketProvider>>,
) {
    let room_state = query.get_single_mut().unwrap();

    draw_round_ui(
        &mut contexts,
        room_state.as_ref(),
        &round_timer,
        &current_player_data,
        &net,
    );
}

pub fn on_enter_round_2(mut round_timer: ResMut<RoundTimer>) {
    // Create a new round timer
    *round_timer = RoundTimer(Timer::from_seconds(ROUND_2_TIME, TimerMode::Once));
}

pub fn add_round_2_scenes(app: &mut App) {
    app.add_systems(Update, draw_round_2_ui.run_if(in_state(GameState::Round2)))
        .add_systems(OnEnter(GameState::Round2), on_enter_round_2);
}

// === ETC ===

// pub fn draw_image(
//     mut contexts: EguiContexts,
//     mut is_initialized: Local<bool>,
//     images: Res<Images>,
//     mut rendered_texture_id: Local<egui::TextureId>,
// ) {
//     if !*is_initialized {
//         *is_initialized = true;
//         *rendered_texture_id = contexts.add_image(images.dog.clone_weak());
//     }

//     egui::Area::new("example_area2".into())
//         .anchor(Align2::CENTER_TOP, (0., 100.))
//         .show(contexts.ctx_mut(), |ui| {
//             let added_button = ui.add(egui::ImageButton::new(egui::widgets::Image::new(
//                 egui::load::SizedTexture::new(*rendered_texture_id, [256.0, 256.0]),
//             )));
//             if added_button.clicked() {
//                 println!("Image clicked!");
//             }
//         });
// }

// === Main add logic ===

pub fn add_scenes(app: &mut App) {
    app.init_state::<GameState>();
    app.insert_resource(Images::default());
    add_intro_scenes(app);
    add_waiting_room_scenes(app);
    add_image_creation_scenes(app);
    add_image_generation_scenes(app);
    add_backend_server_connections(app);
    add_round_1_scenes(app);
    add_round_2_scenes(app);
}
