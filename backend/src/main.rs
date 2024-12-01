use bevy::ecs::query::QueryFilter;
use bevy::log::Level;
use bevy::prelude::*;
use bevy::tasks::TaskPoolBuilder;
use bevy::{log::LogPlugin, tasks::TaskPool};
use bevy_eventwork::{
    AppNetworkMessage, ConnectionId, EventworkRuntime, Network, NetworkData, NetworkEvent,
};

use core::net::Ipv4Addr;
use std::env;
use std::fmt::Debug;
use std::net::{IpAddr, SocketAddr};
use std::ops::DerefMut;

use bevy::tasks::{futures_lite::future, AsyncComputeTaskPool, Task};

use reqwest::blocking::Client;

use serde_json::json;
use serde_json::Value;

use rand::seq::SliceRandom;
use rand::thread_rng;

use bevy_eventwork_mod_websockets::*;

extern crate server_responses;
use server_responses::*;

const DEBUG_MODE: bool = true;

#[derive(Component)]
struct PublicRoom;

#[derive(Component)]
struct InGame;

#[derive(Resource, Default)]
struct ImageGenerationInfo {
    endpoint: String,
    api_key: String,
}

#[derive(Debug, Component)]
struct ImageGenerationTask {
    task: Task<Option<String>>,
    prompt_to_complete: PromptInfoData,
    status: TaskCompletionStatus,
}

#[derive(Component, Default)]
struct RoomStateServerInfo {
    task_list: Vec<ImageGenerationTask>,
}

fn main() {
    info!("Building app");
    let mut app = App::new();

    app.add_plugins(MinimalPlugins)
        .add_plugins(LogPlugin {
            level: Level::DEBUG,
            filter: "wgpu=error,bevy_render=info,bevy_ecs=trace".to_string(),
            custom_layer: |_| None,
        })
        .add_plugins(bevy_eventwork::EventworkPlugin::<WebSocketProvider, TaskPool>::default())
        .insert_resource(EventworkRuntime(
            TaskPoolBuilder::new().num_threads(2).build(),
        ))
        .insert_resource(NetworkSettings::default())
        .insert_resource(ImageGenerationInfo::default())
        .add_systems(Startup, setup_networking)
        .add_systems(Startup, setup_connections)
        .add_systems(Update, handle_connection_events)
        .add_systems(Update, tick_timers)
        .add_systems(Update, handle_timer_events)
        .add_systems(Update, handle_image_generation_tasks)
        .listen_for_message::<RoomJoinRequest, WebSocketProvider>()
        .add_systems(Update, room_join_request)
        .listen_for_message::<StartGameRequest, WebSocketProvider>()
        .add_systems(Update, start_game_request)
        .listen_for_message::<PromptInfoDataRequest, WebSocketProvider>()
        .add_systems(Update, prompt_info_data_update)
        .listen_for_message::<GameActionRequest, WebSocketProvider>()
        .add_systems(Update, game_action_request_update)
        .run();
}

// === Helper Functions ===

async fn get_image_url(input_string: String, url: String, api_key: String) -> Option<String> {
    // Simulate a long-running task
    info!("Starting image generation task");

    if DEBUG_MODE {
        // SLeep a random time
        // let sleep_time = rand::random::<u64>() % 1;
        // std::thread::sleep(std::time::Duration::from_secs(sleep_time));

        let random_image_list = vec![
            // "https://dalleproduse.blob.core.windows.net/private/images/4756af2f-c07e-40b9-abff-06184957db4a/generated_00.png?se=2024-11-30T22%3A05%3A37Z&sig=e2W8tJT6DwB3JY10VSV%2BR8mP2SHkKH4oWawoNbe8gvU%3D&ske=2024-12-06T07%3A57%3A19Z&skoid=09ba021e-c417-441c-b203-c81e5dcd7b7f&sks=b&skt=2024-11-29T07%3A57%3A19Z&sktid=33e01921-4d64-4f8c-a055-5bdaffd5e33d&skv=2020-10-02&sp=r&spr=https&sr=b&sv=2020-10-02",
            "https://i.ebayimg.com/images/g/JPUAAOSw0n5lBnhv/s-l1200.jpg",
            "https://picsum.photos/id/674/300/300",
            "https://picsum.photos/id/675/300/300",
            "https://picsum.photos/id/676/300/300",
            "https://picsum.photos/id/677/300/300",
            "https://picsum.photos/id/678/300/300",
        ];

        return Some(
            random_image_list
                .choose(&mut thread_rng())
                .unwrap()
                .to_string(),
        );
    }

    let client = Client::new();

    let request_body = json!({
       "prompt": input_string,
        "n": 1,
        "size": "1024x1024"
    });

    let response = client
        .post(url)
        .header("api-key", api_key)
        .json(&request_body)
        .send();

    match response {
        Ok(returned_response) => {
            info!("Sent request successfully");
            info!("Response: {:?}", returned_response);

            match returned_response.json::<Value>() {
                Ok(json) => match json.get("data") {
                    Some(data) => match data.get(0) {
                        Some(data_first_element) => match data_first_element.get("url") {
                            Some(url) => {
                                info!("Got url: {}", url);
                                match url.as_str() {
                                    Some(url) => return Some(url.to_string()),
                                    None => {
                                        error!("Failed to get url");
                                        return None;
                                    }
                                }
                            }
                            None => {
                                error!("Failed to get url");
                                return None;
                            }
                        },
                        None => {
                            error!("Failed to get data");
                            return None;
                        }
                    },
                    None => {
                        error!("Failed to get data");
                        return None;
                    }
                },
                Err(e) => {
                    error!("Failed to get json: {:?}", e);
                    return None;
                }
            }
        }
        Err(e) => {
            error!("Failed to send request: {:?}", e);
            return None;
        }
    }
}

fn update_room_state_for_all_players(
    room_state: &RoomState,
    net: &Res<Network<WebSocketProvider>>,
) -> Result<(), String> {
    for player in room_state.players.iter() {
        match net.send_message(ConnectionId { id: player.id }, room_state.clone()) {
            Ok(_) => info!(
                "Sent room state response to {} with id {}",
                player.username, player.id
            ),
            Err(e) => {
                error!("Failed to send message: {:?}", e);
                return Err(format!("Failed to send message: {:?}", e));
            }
        }
    }

    Ok(())
}

fn update_round_end_info_for_all_players(
    round_end_info: &RoundEndInfo,
    room_state: &RoomState,
    net: &Res<Network<WebSocketProvider>>,
) -> Result<(), String> {
    for player in room_state.players.iter() {
        match net.send_message(ConnectionId { id: player.id }, round_end_info.clone()) {
            Ok(_) => info!(
                "Sent round end info to {} with id {}",
                player.username, player.id
            ),
            Err(e) => {
                error!("Failed to send message: {:?}", e);
                return Err(format!("Failed to send message: {:?}", e));
            }
        }
    }

    Ok(())
}

// fn update_prompts_for_all_players(
//     room_state: &RoomState,
//     net: &Res<Network<WebSocketProvider>>,
// ) -> Result<(), String> {
//     for player in room_state.players.iter() {
//         match net.send_message(ConnectionId { id: player.id }, player.prompt_data.clone()) {
//             Ok(_) => info!(
//                 "Sent prompt info to {} with id {}",
//                 player.username, player.id
//             ),
//             Err(e) => {
//                 error!("Failed to send message: {:?}", e);
//                 return Err(format!("Failed to send message: {:?}", e));
//             }
//         }
//     }

//     Ok(())
// }

fn progress_round(
    room_state: &mut RoomState,
    timer: &mut RoundTimer,
    commands: &mut Commands,
    entity: Entity,
    net: &Res<Network<WebSocketProvider>>,
) {
    match room_state.game_state {
        GameState::ImageCreation => {
            room_state.game_state = GameState::ImageGeneration;

            let mut paused_timer = Timer::from_seconds(10.0, TimerMode::Once);
            paused_timer.pause();

            timer.0 = paused_timer;
        }
        GameState::ImageGeneration => {
            room_state.game_state = GameState::BiddingRound;
            room_state.setup_next_round();
            timer.0 = Timer::from_seconds(BIDDING_ROUND_TIME, TimerMode::Once);
        }
        GameState::BiddingRound => {
            room_state.game_state = GameState::BiddingRoundEnd;
            timer.0 = Timer::from_seconds(BIDDING_ROUND_END_TIME, TimerMode::Once);
            let round_end_info_option = room_state.finalize_round();

            // Send round end info to all players
            if let Some(round_end_info) = round_end_info_option {
                let _ = update_round_end_info_for_all_players(&round_end_info, room_state, net);
            } else {
                error!("Failed to finalize round: {:?}", room_state);
            }
        }
        GameState::BiddingRoundEnd => {
            if room_state.remaining_prompts.len() > 0 {
                room_state.game_state = GameState::BiddingRound;
                timer.0 = Timer::from_seconds(BIDDING_ROUND_TIME, TimerMode::Once);
                room_state.setup_next_round();
            } else {
                room_state.game_state = GameState::Intro;
                commands.entity(entity).remove::<RoundTimer>();
            }
        }
        _ => {
            error!(
                "Progress round called but no handler found for: {:?}",
                room_state.game_state
            );
            commands.entity(entity).remove::<RoundTimer>();
        }
    }
    // TODO: Despawn and clean up everything else
}

fn find_and_handle_room<T, Q>(
    mut new_messages: EventReader<NetworkData<T>>,
    net: &Res<Network<WebSocketProvider>>,
    query: &mut Query<
        (
            Entity,
            &mut RoomState,
            &mut RoundTimer,
            &mut RoomStateServerInfo,
        ),
        Q,
    >,
    mut callback: impl FnMut(
        &mut RoomState,
        &mut RoundTimer,
        &mut RoomStateServerInfo,
        &Res<Network<WebSocketProvider>>,
        &Entity,
        &NetworkData<T>,
    ),
) where
    T: HasRoomId + Debug + Event,
    Q: QueryFilter, // Makes the filter generic, allowing flexible `Query` options
{
    for message in new_messages.read() {
        info!("Received new message: {:?}", message);
        // Get the room state entity
        for (entity, mut room_state, mut timer, mut room_state_server_info) in query.iter_mut() {
            if entity.index() == message.room_id() {
                callback(
                    &mut room_state,
                    &mut timer,
                    &mut room_state_server_info,
                    net,
                    &entity,
                    message,
                );
            }
        }
    }
}

// === Scene handling functions ===

// === Core functionality ===
fn setup_connections(mut image_generation_info: ResMut<ImageGenerationInfo>) {
    dotenv::dotenv().ok();
    let azure_ai_image_key =
        env::var("AZURE_AI_IMAGE_KEY").expect("AZURE_AI_IMAGE_KEY must be set");
    let azure_ai_image_endpoint =
        env::var("AZURE_AI_IMAGE_ENDPOINT").expect("AZURE_AI_IMAGE_ENDPOINT must be set");

    image_generation_info.endpoint = azure_ai_image_endpoint;
    image_generation_info.api_key = azure_ai_image_key;
}

fn setup_networking(
    mut net: ResMut<Network<WebSocketProvider>>,
    settings: Res<NetworkSettings>,
    task_pool: Res<EventworkRuntime<TaskPool>>,
) {
    let ip_address = "127.0.0.1".parse().expect("Could not parse ip address");

    info!("Address of the server: {}", ip_address);

    let _socket_address = SocketAddr::new(ip_address, 9999);

    match net.listen(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
        &task_pool.0,
        &settings,
    ) {
        Ok(_) => (),
        Err(err) => {
            error!("Could not start listening: {}", err);
            panic!();
        }
    }

    info!("Started listening for new connections!");
}

fn handle_connection_events(
    mut network_events: EventReader<NetworkEvent>,
    mut query: Query<(&mut RoomState)>,
    net: Res<Network<WebSocketProvider>>,
) {
    for event in network_events.read() {
        if let NetworkEvent::Connected(conn_id) = event {
            info!("New player connected: {}", conn_id);
        } else if let NetworkEvent::Disconnected(conn_id) = event {
            info!("Player disconnected: {}", conn_id);
            for (mut room_state) in query.iter_mut() {
                room_state.disconnect_player(*conn_id);

                // Send updated room state to all players
                match update_room_state_for_all_players(room_state.deref_mut(), &net) {
                    Ok(_) => info!(
                        "Updated player state for all players in room {}",
                        room_state.room_id
                    ),
                    Err(e) => error!("Failed to send message: {:?}", e),
                }
            }
        }
    }
}

fn tick_timers(time: Res<Time>, mut query: Query<&mut RoundTimer>) {
    for mut timer in query.iter_mut() {
        timer.0.tick(time.delta());
    }
}

fn handle_timer_events(
    mut query: Query<(Entity, &mut RoomState, &mut RoundTimer)>,
    mut commands: Commands,
    net: Res<Network<WebSocketProvider>>,
) {
    for (entity, mut room_state, mut timer) in query.iter_mut() {
        if timer.0.finished() {
            info!("Timer finished for room {}", room_state.room_id);
            progress_round(
                room_state.deref_mut(),
                timer.deref_mut(),
                &mut commands,
                entity,
                &net,
            );

            // Send updated room state to all players
            match update_room_state_for_all_players(&room_state, &net) {
                Ok(_) => info!(
                    "Updated player state for all players in room {}",
                    room_state.room_id
                ),
                Err(e) => error!("Failed to send message: {:?}", e),
            }
        }
    }
}

fn handle_image_generation_tasks(
    mut commands: Commands,
    net: Res<Network<WebSocketProvider>>,
    mut query: Query<(
        Entity,
        &mut RoomState,
        &mut RoundTimer,
        &mut RoomStateServerInfo,
    )>,
) {
    for (entity, mut room_state, mut timer, mut room_state_server_info) in query.iter_mut() {
        if room_state.game_state == GameState::ImageGeneration {
            for compute_task_info in room_state_server_info.task_list.iter_mut() {
                if let Some(string_option) =
                    future::block_on(future::poll_once(&mut compute_task_info.task))
                {
                    info!(
                        "Handling result of image generation task: {:?}",
                        compute_task_info
                    );

                    if let Some(string_value) = string_option {
                        info!("Task completed successfully: {:?}", string_value);
                        compute_task_info.prompt_to_complete.image_url = string_value.clone();
                        compute_task_info.status = TaskCompletionStatus::Completed;

                        // Add the prompt to the remaining prompts
                        let completed_prompt =
                            std::mem::take(&mut compute_task_info.prompt_to_complete);

                        room_state.remaining_prompts.push(completed_prompt);
                    } else {
                        error!("Task failed to complete: {:?}", compute_task_info);
                        compute_task_info.status = TaskCompletionStatus::Error;
                        // TODO: Handle if error state fails
                    }
                }
            }
            // Remove all finished tasks
            room_state_server_info
                .task_list
                .retain(|task| task.status == TaskCompletionStatus::InProgress);

            // If all tasks are completed, progress the round
            if room_state_server_info.task_list.is_empty() {
                info!(
                    "All tasks are completed for room {}, progressing to next round",
                    room_state.room_id
                );
                progress_round(
                    room_state.deref_mut(),
                    timer.deref_mut(),
                    &mut commands,
                    entity,
                    &net,
                );

                // Send updated room state to all players
                match update_room_state_for_all_players(room_state.deref_mut(), &net) {
                    Ok(_) => info!(
                        "Updated player state for all players in room {}",
                        room_state.room_id
                    ),
                    Err(e) => error!("Failed to send message: {:?}", e),
                }
            }
        }
    }
}

// === API Requests ===

fn room_join_request(
    mut new_messages: EventReader<NetworkData<RoomJoinRequest>>,
    net: Res<Network<WebSocketProvider>>,
    mut query: Query<&mut RoomState, (With<PublicRoom>, Without<InGame>)>,
    mut commands: Commands,
) {
    for new_message in new_messages.read() {
        info!("New room join request: {:?}", new_message);
        let message_source = new_message.source();
        let incoming_connection_id = message_source.id;

        // TODO: I'm sure this code is full of bugs. Need to evaluate and fix!

        // If the room already exists, its state will get updated
        for mut room_state in query.iter_mut() {
            room_state.players.push(Player::new(
                incoming_connection_id,
                new_message.username.clone(),
            ));

            // Send updated room state to all players
            match update_room_state_for_all_players(room_state.deref_mut(), &net) {
                Ok(_) => info!(
                    "Updated player state for all players in room {}",
                    room_state.room_id
                ),
                Err(e) => error!("Failed to send message: {:?}", e),
            }
            return;
        }

        // Else create a new entity with room state
        let new_room_entity = commands.spawn(PublicRoom).id();

        let response = RoomState {
            room_id: new_room_entity.index(),
            players: vec![Player::new(
                incoming_connection_id,
                new_message.username.clone(),
            )],
            game_state: GameState::WaitingRoom,
            current_art_bid: ArtBidInfo::default(),
            prompts_per_player: 3,
            remaining_prompts: vec![],
            used_prompts: vec![],
            received_prompt_count: 0,
        };

        let mut inserted_timer = Timer::from_seconds(5.0, TimerMode::Once);
        inserted_timer.pause();

        commands
            .entity(new_room_entity)
            .insert(RoundTimer(inserted_timer));
        commands.entity(new_room_entity).insert(response.clone());
        commands
            .entity(new_room_entity)
            .insert(RoomStateServerInfo::default());

        match net.send_message(
            ConnectionId {
                id: incoming_connection_id,
            },
            response,
        ) {
            Ok(_) => info!("Created room with id {} ", new_room_entity.index()),
            Err(e) => error!("Failed to send message: {:?}", e),
        }
    }
}

fn start_game_request(
    new_messages: EventReader<NetworkData<StartGameRequest>>,
    net: Res<Network<WebSocketProvider>>,
    mut query: Query<
        (
            Entity,
            &mut RoomState,
            &mut RoundTimer,
            &mut RoomStateServerInfo,
        ),
        Without<InGame>,
    >,
    mut commands: Commands,
) {
    find_and_handle_room(
        new_messages,
        &net,
        &mut query,
        |mut room_state, timer, _room_state_server_info, net, entity, _message| {
            info!("New start game request: {:?}", _message);
            room_state.game_state = GameState::ImageCreation;

            // Add InGame component to game
            commands.entity(*entity).insert(InGame);

            // Send initial prompts to all players
            for player in room_state.players.iter() {
                let mut prompt_list = Vec::new();

                for i in 0..room_state.prompts_per_player {
                    let new_prompt = PromptInfoData {
                        prompt_text: String::default(),
                        prompt_answer: String::default(),
                        image_url: String::default(),
                        owner_id: player.id,
                    };
                    prompt_list.push(new_prompt);
                }

                let new_prompt_data = PromptInfoDataRequest {
                    prompt_list: prompt_list,
                    room_id: room_state.room_id,
                };

                match net.send_message(ConnectionId { id: player.id }, new_prompt_data) {
                    Ok(_) => info!(
                        "Sent prompt info to {} with id {}",
                        player.username, player.id
                    ),
                    Err(e) => {
                        error!("Failed to send message: {:?}", e);
                    }
                }
            }

            // Set game timer
            timer.0 = Timer::from_seconds(IMAGE_CREATION_TIME, TimerMode::Once);

            match update_room_state_for_all_players(room_state.deref_mut(), &net) {
                Ok(_) => info!("Started game in room {}", room_state.room_id),
                Err(e) => error!("Failed to send message: {:?}", e),
            }
        },
    );
}

fn prompt_info_data_update(
    new_messages: EventReader<NetworkData<PromptInfoDataRequest>>,
    net: Res<Network<WebSocketProvider>>,
    mut commands: Commands,
    mut query: Query<(
        Entity,
        &mut RoomState,
        &mut RoundTimer,
        &mut RoomStateServerInfo,
    )>,
    image_generation_info: Res<ImageGenerationInfo>,
) {
    find_and_handle_room(
        new_messages,
        &net,
        &mut query,
        |mut room_state, timer, room_state_server_info, net, entity, message| {
            info!("Received prompt info data update: {:?}", message);
            let message_source = message.source();
            let incoming_connection_id = message_source.id;

            for player in room_state.players.iter_mut() {
                if player.id == incoming_connection_id {
                    for (prompt_index, prompt) in message.prompt_list.iter().enumerate() {
                        if prompt.prompt_answer != "" {
                            info!("Generating image for prompt: {:?}", prompt);

                            room_state.received_prompt_count += 1;

                            // Create a task to get the image URL for each prompt
                            let thread_pool = AsyncComputeTaskPool::get();

                            let endpoint = image_generation_info.endpoint.clone();
                            let api_key = image_generation_info.api_key.clone();
                            let input_string = prompt.prompt_answer.clone();

                            let task = thread_pool.spawn(async {
                                get_image_url(input_string, endpoint, api_key).await
                            });
                            room_state_server_info.task_list.push(ImageGenerationTask {
                                task,
                                prompt_to_complete: prompt.clone(),
                                status: TaskCompletionStatus::InProgress,
                            });
                        }
                    }
                }
            }

            // If we have enough prompts, progress the round
            if room_state.received_prompt_count
                == room_state.players.len() as u32 * room_state.prompts_per_player
            {
                progress_round(room_state, timer, &mut commands, *entity, &net);
                match update_room_state_for_all_players(room_state.deref_mut(), &net) {
                    Ok(_) => info!(
                        "Received all prompts and now progressing in room {}",
                        room_state.room_id
                    ),
                    Err(e) => error!("Failed to send message: {:?}", e),
                }
            } else {
                info!("Player prompts are not fully completed: {:?}", room_state);
            }
        },
    );
}

fn game_action_request_update(
    new_messages: EventReader<NetworkData<GameActionRequest>>,
    net: Res<Network<WebSocketProvider>>,
    mut commands: Commands,
    mut query: Query<(
        Entity,
        &mut RoomState,
        &mut RoundTimer,
        &mut RoomStateServerInfo,
    )>,
) {
    // Find and handle room
    find_and_handle_room(
        new_messages,
        &net,
        &mut query,
        |mut room_state, mut timer, _room_state_server_info, net, entity, message| {
            info!("Received game action request: {:?}", message);
            let message_source = message.source();
            let incoming_connection_id = message_source.id;

            // Get the player who sent the message
            let player_option = room_state
                .players
                .iter_mut()
                .find(|player| player.id == incoming_connection_id);

            match player_option {
                None => {
                    error!(
                        "Player with id {} not found in room {}",
                        incoming_connection_id, room_state.room_id
                    );
                    return;
                }
                Some(player) => {
                    info!("Player {:?} took action: {:?}", player.id, message.action);

                    // Handle the action
                    match message.action {
                        GameAction::Bid => {
                            info!("Player trying to bid on art");
                            if room_state.game_state == GameState::BiddingRound {
                                let new_bid_amount = room_state.current_art_bid.max_bid
                                    + room_state.current_art_bid.bid_increase_amount;
                                if player.money >= new_bid_amount {
                                    info!("Player {} bid {}", player.id, new_bid_amount);
                                    room_state.current_art_bid.max_bid_player_id = player.id;
                                    room_state.current_art_bid.max_bid = new_bid_amount;
                                }
                            }
                        }
                        GameAction::EndRound => {
                            progress_round(
                                room_state.deref_mut(),
                                timer.deref_mut(),
                                &mut commands,
                                *entity,
                                &net,
                            );
                        }
                    }
                }
            }

            match update_room_state_for_all_players(room_state.deref_mut(), &net) {
                Ok(_) => info!(
                    "Updated player state for all players in room {}",
                    room_state.room_id
                ),
                Err(e) => error!("Failed to send message: {:?}", e),
            }
        },
    );
}
