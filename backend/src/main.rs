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

#[derive(Component)]
struct PublicRoom;

#[derive(Component)]
struct InGame;

#[derive(Resource, Default)]
struct ImageGenerationInfo {
    endpoint: String,
    api_key: String,
}

#[derive(Component)]
struct ImageGenerationTask {
    task: Task<Option<String>>,
    prompt_number: usize,
    player_id: u32,
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
        .listen_for_message::<PromptInfoDataList, WebSocketProvider>()
        .add_systems(Update, prompt_info_data_update)
        .listen_for_message::<GameActionRequest, WebSocketProvider>()
        .add_systems(Update, game_action_request_update)
        .run();
}

// === Helper Functions ===

async fn get_image_url(input_string: String, url: String, api_key: String) -> Option<String> {
    // Simulate a long-running task
    info!("Starting image generation task");

    // SLeep a random time
    let sleep_time = rand::random::<u64>() % 8;
    std::thread::sleep(std::time::Duration::from_secs(sleep_time));
    return Some("https://i.ebayimg.com/images/g/JPUAAOSw0n5lBnhv/s-l1200.jpg".to_string());

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
                                return Some(url.to_string());
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

fn update_prompts_for_all_players(
    room_state: &RoomState,
    net: &Res<Network<WebSocketProvider>>,
) -> Result<(), String> {
    for player in room_state.players.iter() {
        match net.send_message(ConnectionId { id: player.id }, player.prompt_data.clone()) {
            Ok(_) => info!(
                "Sent prompt info to {} with id {}",
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

fn finalize_and_setup_new_round(room_state: &mut RoomState) {
    let current_art_bid_number = room_state.current_art_bid.art_bid_number;

    // Check if max bid is greater than 0 and handle existing bid info
    if room_state.current_art_bid.max_bid > 0 {
        let winning_player = room_state
            .players
            .iter_mut()
            .find(|player| player.id == room_state.current_art_bid.owner_player_id);

        match winning_player {
            Some(player) => {
                player.money -= room_state.current_art_bid.max_bid;
                // TODO: Add art to player's collection
            }
            None => {
                error!(
                    "Could not find winning player with id {}",
                    room_state.current_art_bid.owner_player_id
                );
            }
        }
    }

    // Prepare the next bid info
    room_state.current_art_bid = ArtBidInfo::default();
    room_state.current_art_bid.bid_increase_amount = 100;
    room_state.current_art_bid.art_bid_number = current_art_bid_number + 1;

    // Choose a random player and prompt number
    let random_player = room_state.players.choose(&mut thread_rng()).unwrap();
    let random_prompt_number =
        rand::random::<u32>() % random_player.prompt_data.prompt_list.len() as u32;

    room_state.current_art_bid.owner_player_id = random_player.id;
    room_state.current_art_bid.owner_prompt_number = random_prompt_number;
    info!(
        "Picked player {} and prompt number {} for new round in room {}",
        random_player.id, random_prompt_number, room_state.room_id
    );

    if let Some(prompt_data) = random_player
        .prompt_data
        .prompt_list
        .get(random_prompt_number as usize)
    {
        info!("Setting prompt image URL for player {} and prompt number {} to {}", random_player.id, random_prompt_number, prompt_data.prompt_image_url);
        room_state.current_art_bid.image_url = prompt_data.prompt_image_url.clone();
    } else {
        error!(
            "Failed to get image URL for player {} and prompt number {}",
            random_player.id, random_prompt_number
        );
    }
}

fn progress_round(
    room_state: &mut RoomState,
    timer: &mut RoundTimer,
    commands: &mut Commands,
    entity: Entity,
) {
    match room_state.game_state {
        GameState::ImageCreation => {
            room_state.game_state = GameState::ImageGeneration;

            let mut paused_timer = Timer::from_seconds(10.0, TimerMode::Once);
            paused_timer.pause();

            timer.0 = paused_timer;
            info!("Progressed to image generation",);
        }
        GameState::ImageGeneration => {
            room_state.game_state = GameState::Round1;
            finalize_and_setup_new_round(room_state);
            timer.0 = Timer::from_seconds(ROUND_1_TIME, TimerMode::Once);
            info!("Progressed to round 1");
        }
        GameState::Round1 => {
            room_state.game_state = GameState::Round2;
            finalize_and_setup_new_round(room_state);
            timer.0 = Timer::from_seconds(ROUND_2_TIME, TimerMode::Once);
        }
        GameState::Round2 => {
            room_state.game_state = GameState::Intro;
            commands.entity(entity).remove::<RoundTimer>();
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

fn handle_connection_events(mut network_events: EventReader<NetworkEvent>) {
    for event in network_events.read() {
        if let NetworkEvent::Connected(conn_id) = event {
            info!("New player connected: {}", conn_id);
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
                if let Some(string_value) =
                    future::block_on(future::poll_once(&mut compute_task_info.task))
                {
                    // Find the prompt that matches the task and set its image url
                    if let Some(player) = room_state
                        .players
                        .iter_mut()
                        .find(|player| player.id == compute_task_info.player_id)
                    {
                        if let Some(prompt) = player
                            .prompt_data
                            .prompt_list
                            .get_mut(compute_task_info.prompt_number)
                        {
                            if let Some(image_url) = string_value {
                                prompt.prompt_image_url = image_url;
                                compute_task_info.status = TaskCompletionStatus::Completed;
                            } else {
                                error!("Failed to get image URL");
                                compute_task_info.status = TaskCompletionStatus::Error;
                            }
                        } else {
                            error!("Failed to get prompt");
                            compute_task_info.status = TaskCompletionStatus::Error;
                        }
                    } else {
                        error!("Failed to get player");
                        compute_task_info.status = TaskCompletionStatus::Error;
                    }
                }
                // TODO: Handle error state if the task fails
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
                );

                info!("ROom state before sending out to all players after image generation: {:?}", room_state);

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

            // Generate prompts for all players
            for player in room_state.players.iter_mut() {
                player.prompt_data = PromptInfoDataList {
                    prompt_list: vec![
                        PromptInfoData {
                            prompt_text: "A dog holding a frisbee".to_string(),
                            prompt_answer:
                                "A furry four legged animal with ears and a tail holding a disk"
                                    .to_string(),
                            prompt_image_url: "https://i.ebayimg.com/images/g/JPUAAOSw0n5lBnhv/s-l1200.jpg".to_string(),
                        },
                        // PromptInfoData {
                        //     prompt_text: "A cat playing with a ball of yarn".to_string(),
                        //     prompt_answer: "Hello other World".to_string(),
                        //     prompt_image_url: "".to_string(),
                        // },
                    ],
                    room_id: room_state.room_id,
                }
            }

            match update_prompts_for_all_players(room_state.deref_mut(), &net) {
                Ok(_) => info!("Sent prompts to all players in room {}", room_state.room_id),
                Err(e) => error!("Failed to send message: {:?}", e),
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
    new_messages: EventReader<NetworkData<PromptInfoDataList>>,
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
                    player.prompt_data = message.additional_clone();

                    for (prompt_index, prompt) in player.prompt_data.prompt_list.iter().enumerate()
                    {
                        if prompt.prompt_answer != "" {
                            info!("Generating image for prompt: {:?}", prompt);
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
                                prompt_number: prompt_index,
                                player_id: player.id,
                                status: TaskCompletionStatus::InProgress,
                            });
                        }
                    }
                }
            }

            // If all players have submitted their prompts (AKA each player has atleast one prompt answer without "") then set the next state
            if room_state.players.iter().all(|player| {
                player
                    .prompt_data
                    .prompt_list
                    .iter()
                    .all(|prompt| prompt.prompt_answer != "")
            }) {
                // TODO: Start image generation state
                progress_round(room_state, timer, &mut commands, *entity);

                match update_room_state_for_all_players(room_state.deref_mut(), &net) {
                    Ok(_) => info!("Received all prompts and now progressing in room {}", room_state.room_id),
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
                            let new_bid_amount = room_state.current_art_bid.max_bid
                                + room_state.current_art_bid.bid_increase_amount;
                            if player.money >= new_bid_amount {
                                info!("Player {} bid {}", player.id, new_bid_amount);
                                room_state.current_art_bid.owner_player_id = player.id;
                                room_state.current_art_bid.max_bid = new_bid_amount;
                            }
                        }
                        GameAction::EndRound => {
                            progress_round(
                                room_state.deref_mut(),
                                timer.deref_mut(),
                                &mut commands,
                                *entity,
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
