use bevy::{
    prelude::*,
    render::{
        render_asset::RenderAssetUsages,
        render_resource::{Extent3d, TextureDimension, TextureFormat},
    },
};
use bevy_egui::{
    egui::{self, Align2, RichText},
    EguiContexts,
};
use bevy_eventwork::Network;
use bevy_eventwork_mod_websockets::WebSocketProvider;

use crate::resources::*;
mod backend_server_connections;
use backend_server_connections::*;

use server_responses::*;

use bevy_async_task::AsyncTaskRunner;

use ::image::ImageReader;
use std::{collections::HashMap, io::Cursor, task::Poll};

// === Assets ===
#[derive(Resource, Debug, Default)]
pub struct Images {
    current_bid_image: Option<Handle<Image>>,
}

#[derive(Component)]
pub struct BidImage;

// impl FromWorld for Images {
//     fn from_world(world: &mut World) -> Self {
//         let asset_server = world.get_resource_mut::<AssetServer>().unwrap();
//         Self {
//             dog: asset_server.load("dog.png"),
//         }
//     }
// }

// === Helper functions ===

fn timer_value_to_alpha_function(
    remaining_time_value: f32,
    fade_in_value: f32,
    fade_out_value: f32,
    total_timer_value: f32,
) -> u8 {
    let return_value: f32;
    if remaining_time_value > fade_in_value {
        // Linear fade in
        return_value = (total_timer_value - remaining_time_value)
            / (total_timer_value - fade_in_value)
            * 255.0;
    } else if remaining_time_value > fade_out_value {
        return_value = 255.0;
    } else {
        // Linear fade out
        return_value = remaining_time_value / fade_out_value * 255.0;
    }
    return return_value as u8;
}

// === Intro scenes ===

pub fn draw_intro_ui(
    mut contexts: EguiContexts,
    mut input_text: Local<String>,
    mut room_code_text: Local<String>,
    mut player_settings: ResMut<PlayerSettings>,
    net: Res<Network<WebSocketProvider>>,
) {
    if player_settings.username != "" {
        // Room option select screen
        egui::Window::new("welcome_area".to_string())
            .anchor(Align2::CENTER_TOP, (0., 200.))
            .show(contexts.ctx_mut(), |ui| {
                ui.vertical(|ui| {
                    ui.label("Select a room");
                    ui.vertical(|ui| {
                        let random_room = ui.button("Join random room");
                        ui.add_space(10.0);
                        if random_room.clicked() {
                            info!("Starting request to server");

                            send_random_room_request(player_settings.username.as_str(), &net);
                        }

                        ui.add_space(10.0);

                        ui.vertical(|ui| {
                            ui.label("Enter custom room code");
                            ui.text_edit_singleline(&mut *room_code_text);
                            let private_room = ui.add_enabled(
                                room_code_text.len() > 0,
                                egui::Button::new("Join private room"),
                            );
                            if private_room.clicked() {
                                info!("Joining private room");
                                send_private_room_request(
                                    player_settings.username.as_str(),
                                    &room_code_text,
                                    &net,
                                );
                            }
                        });
                    });
                });
            });
    } else {
        // Username input screen
        egui::Window::new("input_area".to_string())
            .anchor(Align2::CENTER_TOP, (0., 200.))
            .show(contexts.ctx_mut(), |ui| {
                ui.vertical(|ui| {
                    ui.label("Enter username");
                    ui.vertical(|ui| {
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
    egui::Window::new("waiting_room_area".to_string())
        .anchor(Align2::CENTER_TOP, (0., 200.))
        .show(contexts.ctx_mut(), |ui| {
            ui.vertical(|ui| {
                ui.label("Waiting room");
                for player in room_state.players.iter() {
                    ui.horizontal(|ui| {
                        ui.label(player.username.clone());
                    });
                }

                // Check if the current player is the host (player in position 0)
                if let Some(host) = room_state.players.get(0) {
                    // TODO: Do an ID based check instead of username check
                    if host.username == player_settings.username {
                        // Replace with actual current player username check
                        let button = ui.add_enabled(
                            room_state.players.len() >= MIN_PLAYERS,
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
    mut front_end_prompt_list: ResMut<FrontEndPromptList>,
    net: Res<Network<WebSocketProvider>>,
) {
    egui::Window::new("image_creation_area".to_string())
        .anchor(Align2::CENTER_TOP, (0., 0.))
        .show(contexts.ctx_mut(), |ui| {
            ui.vertical(|ui| {
                ui.label("Fill out these prompts");
                ui.add_space(10.0); // Add some space between the label and the text box

                for index in 0..front_end_prompt_list.prompt_data_list.len() {
                    ui.vertical(|ui| {
                        let prompt_data_message =
                            &mut front_end_prompt_list.prompt_data_list[index];
                        let prompt = &mut prompt_data_message.prompt;
                        ui.label(prompt.prompt_text.clone());
                        if prompt_data_message.state == PromptState::Error {
                            // Show label in red if there is an error
                            ui.label(
                                egui::RichText::new(prompt_data_message.error_message.clone())
                                    .color(egui::Color32::from_rgb(255, 100, 100)),
                            );
                        } else if prompt_data_message.state == PromptState::PromptCompleted {
                            // Show label in green if the prompt is completed
                            ui.label(
                                egui::RichText::new("Prompt completed")
                                    .color(egui::Color32::from_rgb(100, 255, 100)),
                            );
                        } else if prompt_data_message.state == PromptState::FullyCompleted {
                            ui.label(
                                egui::RichText::new("Fully completed - image generated")
                                    .color(egui::Color32::from_rgb(100, 255, 100)),
                            );
                        }

                        ui.text_edit_multiline(&mut prompt.prompt_answer);
                        ui.add_space(10.0); // Add some space between the label and the text box

                        let button = ui.add_enabled(
                            prompt_data_message.state == PromptState::Proposed
                                || prompt_data_message.state == PromptState::Error,
                            egui::Button::new("Submit prompt"),
                        );

                        if button.clicked() {
                            send_completed_prompt(prompt_data_message, index, &net);
                        }
                    });
                }
            });
        });
}

pub fn on_enter_image_creation(mut commands: Commands) {
    // Reset the button submitted state
    commands.insert_resource(PlayerSettings {
        username: String::new(),
    });
}

pub fn add_image_creation_scenes(app: &mut App) {
    app.add_systems(
        Update,
        draw_image_creation_ui.run_if(in_state(GameState::ImageCreation)),
    )
    .add_systems(OnEnter(GameState::ImageCreation), on_enter_image_creation);
}

// === Bidding round scenes ===

pub fn draw_bidding_round_ui(
    mut contexts: EguiContexts,
    round_timer: ResMut<RoundTimer>,
    mut query: Query<&mut RoomState>,
    current_player_data: Res<CurrentPlayerData>,
    net: Res<Network<WebSocketProvider>>,
    mut task_executor: AsyncTaskRunner<Option<Image>>,
    asset_server: ResMut<AssetServer>,
    mut images: ResMut<Images>,
    mut commands: Commands,
    game_state: Res<State<GameState>>,
    round_end_info: Res<RoundEndInfo>,
    notifications_query: Query<&GamePlayerNotification>,
) {
    let room_state = query.get_single_mut().unwrap();

    let current_player = room_state
        .players
        .iter()
        .find(|player| player.id == current_player_data.player_id)
        .unwrap();

    // If there is no image start the process to get one
    if images.current_bid_image.is_none() {
        // Check if a task already exists before starting it
        if task_executor.is_idle() {
            let url = room_state.current_art_bid.prompt_info.image_url.clone();
            // Spawn an async task to download the image
            task_executor.start(async move {
                info!("Started image loading for: {}", url.escape_debug());

                let client = reqwest::Client::new();
                let response = client.get(&url).send().await;

                match response {
                    Ok(resp) => {
                        if resp.status().is_success() {
                            let bytes = resp.bytes().await.unwrap();
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
                        } else {
                            info!("HTTP error: {}", resp.status());
                            if let Ok(text) = resp.text().await {
                                info!("Response body: {}", text);
                                panic!();
                            } else {
                                panic!();
                            }
                        }
                    }
                    Err(e) => {
                        info!("Failed to fetch url at all: {:?}", e);
                        panic!();
                    }
                }
            });
        }
    }

    match task_executor.poll() {
        Poll::Pending => {}
        Poll::Ready(Ok(returned_image_option)) => {
            if let Some(returned_image) = returned_image_option {
                let image_handle = asset_server.add(returned_image.clone());
                images.current_bid_image = Some(image_handle.clone());
                // Spawn entity with this image

                let mut image_sprite = Sprite::from_image(image_handle.clone());
                image_sprite.custom_size = Some(Vec2::new(75., 75.));

                commands.spawn((
                    BidImage,
                    Transform::from_translation(Vec3::new(0., -15.0, 0.)),
                    image_sprite,
                ));
            }
        }
        Poll::Ready(Err(e)) => {
            info!("Error in async task: {:?}", e);
        }
    }

    egui::Window::new("player_hints".to_string())
        .anchor(Align2::CENTER_BOTTOM, (0., 10.))
        .show(contexts.ctx_mut(), |ui| {
            ui.vertical(|ui| {
                ui.heading("Your Hints");
                ui.add_space(8.0);

                for hint in current_player.hints.iter() {
                    ui.label(hint);
                }

                if current_player.hints.is_empty() {
                    ui.label("No hints available yet");
                }
            });
        });

    egui::Window::new("round_area".to_string())
        .anchor(Align2::CENTER_TOP, (0., 0.))
        .default_width(300.0)
        .show(contexts.ctx_mut(), |ui| {
            egui::ScrollArea::horizontal().show(ui, |ui| {
                // Show initial top bar of status
                ui.add_space(2.0);
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
                        .find(|player| player.id == room_state.current_art_bid.max_bid_player_id);

                    ui.vertical(|ui| {
                        ui.label("Max bid owner:");
                        if let Some(owner) = current_bid_owner {
                            if room_state.current_art_bid.max_bid > 0 {
                                ui.label(format!("{}", owner.username));
                            } else {
                                ui.label("No owner");
                            }
                        }
                    });

                    ui.vertical(|ui| {
                        ui.add_space(1.0);
                    });
                });

                ui.add_space(5.0);
                if *game_state.get() == GameState::BiddingRound {
                    // Prepare hash map for player notifications
                    let mut player_notifications_map =
                        HashMap::<u32, Vec<&GamePlayerNotification>>::new();
                    for notification in notifications_query.iter() {
                        if let Some(notification_list) =
                            player_notifications_map.get_mut(&notification.target_player_id)
                        {
                            notification_list.push(notification);
                        } else {
                            player_notifications_map
                                .insert(notification.target_player_id, vec![notification]);
                        }
                    }

                    // Sort notifications by time remaining
                    for notification_list in player_notifications_map.values_mut() {
                        notification_list.sort_by(|a, b| {
                            b.timer
                                .remaining_secs()
                                .partial_cmp(&a.timer.remaining_secs())
                                .unwrap()
                        });
                    }

                    // Show players if in bidding round
                    ui.columns(room_state.players.len(),|columns| {
                        for (i, player) in room_state.players.iter().enumerate() {
                            columns[i].vertical(|ui| {
                                if player.id == current_player_data.player_id {
                                    ui.label(
                                        RichText::new(format!("{} (You)", player.username))
                                            .strong(),
                                    );
                                } else {
                                    ui.label(
                                        RichText::new(format!("{}", player.username)).strong(),
                                    );
                                }

                                ui.label(format!("Force bids: {}", player.force_bids_left));

                                if player.id == current_player_data.player_id {
                                    let button = ui.add_enabled(
                                        *game_state.get() == GameState::BiddingRound,
                                        egui::Button::new("Bid")
                                            .fill(egui::Color32::from_rgb(45, 65, 180)),
                                    );

                                    if button.clicked() {
                                        send_bid_action(
                                            current_player_data.player_id,
                                            room_state.room_id,
                                            &net,
                                        );
                                    }
                                } else {
                                    let force_bid_button = ui.add_enabled(
                                        current_player.force_bids_left > 0
                                            && *game_state.get() == GameState::BiddingRound,
                                        egui::Button::new("Force bid"),
                                    );

                                    if force_bid_button.clicked() {
                                        send_force_bid_action(
                                            current_player_data.player_id,
                                            player.id,
                                            room_state.room_id,
                                            &net,
                                        );
                                    }
                                }
                                // Show notifications
                                ui.label("------");
                                if let Some(notification_list) =
                                    player_notifications_map.get(&player.id)
                                {
                                    for notification in notification_list {
                                        if notification.target_player_id == player.id {
                                            let fade_time = 0.2;
                                            let color_value = timer_value_to_alpha_function(
                                                notification.timer.remaining_secs(),
                                                notification.timer.duration().as_secs_f32()
                                                    - fade_time,
                                                fade_time,
                                                notification.timer.duration().as_secs_f32(),
                                            );
                                            ui.label(
                                                egui::RichText::new(notification.message.clone())
                                                    .color(egui::Color32::from_rgba_premultiplied(
                                                        color_value,
                                                        color_value,
                                                        color_value,
                                                        color_value,
                                                        // timer_value_to_alpha_function(
                                                        //     notification.timer.remaining_secs(),
                                                        //     notification.timer.duration().as_secs_f32()
                                                        //         - 1.0,
                                                        //     1.0,
                                                        //     notification.timer.duration().as_secs_f32(),
                                                        // ),
                                                    )),
                                            );
                                        }
                                    }
                                }
                            });
                        }
                    });
                } else if *game_state.get() == GameState::BiddingRoundEnd {
                    // Show end of bidding round information
                    ui.label("Bidding round end");
                    ui.label(format!("Artist: {}", round_end_info.artist_name));
                    ui.label(format!("Bid winner: {}", round_end_info.bid_winner_name));
                    ui.label(format!("Amount bid: {}", round_end_info.winning_bid_amount));
                    ui.label(format!("Art value: {}", round_end_info.art_value));
                }
            });
        });
}

pub fn on_enter_bidding_round(mut round_timer: ResMut<RoundTimer>) {
    // Create a new round timer
    *round_timer = RoundTimer(Timer::from_seconds(
        BIDDING_ROUND_TIME - 1.0,
        TimerMode::Once,
    ));
}

pub fn on_exit_bidding_round_end(
    mut commands: Commands,
    query: Query<Entity, With<BidImage>>,
    mut images: ResMut<Images>,
) {
    // Remove the image entity
    for entity in query.iter() {
        commands.entity(entity).despawn_recursive();
    }

    // Clear the current bid image
    images.current_bid_image = None;
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
struct InBiddingRound;

impl ComputedStates for InBiddingRound {
    type SourceStates = GameState;
    fn compute(sources: GameState) -> Option<Self> {
        match sources {
            GameState::BiddingRound => Some(Self),
            GameState::BiddingRoundEnd => Some(Self),
            _ => None,
        }
    }
}

pub fn add_bidding_round_scenes(app: &mut App) {
    app.add_computed_state::<InBiddingRound>();
    app.add_systems(
        Update,
        draw_bidding_round_ui.run_if(in_state(InBiddingRound)),
    );
    app.add_systems(OnEnter(GameState::BiddingRound), on_enter_bidding_round);
    app.add_systems(
        OnExit(GameState::BiddingRoundEnd),
        on_exit_bidding_round_end,
    );
}

// === End score screen scenes ===

pub fn draw_end_score_screen_ui(
    mut contexts: EguiContexts,
    game_end_info: Res<GameEndInfo>,
    round_timer: Res<RoundTimer>,
) {
    egui::Window::new("end_score_screen_area".to_string())
        .anchor(Align2::CENTER_TOP, (0., 0.))
        .show(contexts.ctx_mut(), |ui| {
            ui.vertical(|ui| {
                ui.label("Time left: ");
                ui.label(format!("{:.2}", round_timer.0.remaining_secs()));
            });
            ui.label("End score screen");

            for (index, player) in game_end_info.players.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "{}. {}: {}",
                        index + 1,
                        player.username,
                        player.money
                    ));
                });
            }
        });
}

pub fn on_enter_end_score_screen(mut round_timer: ResMut<RoundTimer>) {
    // Create a new round timer
    *round_timer = RoundTimer(Timer::from_seconds(
        END_SCORE_SCREEN_TIME - 1.0,
        TimerMode::Once,
    ));
}

pub fn add_end_score_screen_scenes(app: &mut App) {
    app.add_systems(
        Update,
        draw_end_score_screen_ui.run_if(in_state(GameState::EndScoreScreen)),
    );
    app.add_systems(
        OnEnter(GameState::EndScoreScreen),
        on_enter_end_score_screen,
    );
}

// Default scenes

fn draw_version_number (
    mut contexts: EguiContexts,
    query: Query<&RoomState>,
) {
    match query.get_single() {
        Err(_) => {},
        Ok(room_state) => {
            egui::Area::new("version_number".into())
                .anchor(Align2::RIGHT_BOTTOM, (-10., -10.))
                .show(contexts.ctx_mut(), |ui| {
                    ui.label(format!("Version: {}", room_state.version_number));
                });
        }
    }
}

// === Main add logic ===
pub fn add_scenes(app: &mut App) {
    app.init_state::<GameState>();
    app.add_computed_state::<InBiddingRound>();
    app.insert_resource(Images::default());
    app.add_systems(Update, draw_version_number);
    add_intro_scenes(app);
    add_waiting_room_scenes(app);
    add_image_creation_scenes(app);
    add_backend_server_connections(app);
    add_bidding_round_scenes(app);
    add_end_score_screen_scenes(app);
}
