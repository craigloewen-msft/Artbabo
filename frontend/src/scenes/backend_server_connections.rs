use std::time::Duration;

use bevy::{
    prelude::*,
    tasks::{TaskPool, TaskPoolBuilder},
};

use crate::resources::CurrentPlayerData;

use bevy_eventwork::{
    AppNetworkMessage, ConnectionId, EventworkRuntime, Network, NetworkData, NetworkEvent,
};
use bevy_eventwork_mod_websockets::*;
use server_responses::*;

use super::FrontEndPromptList;

const SERVER_CONNECTION_ID: ConnectionId = ConnectionId { id: 0 };

// Send message functions

pub fn send_random_room_request(username: &str, net: &Res<Network<WebSocketProvider>>) {
    let request = RoomJoinRequest {
        username: username.to_string(),
        room_code: "".to_string(),
    };

    match net.send_message(SERVER_CONNECTION_ID, request) {
        Ok(_) => info!("Sent random room request"),
        Err(e) => error!("Failed to send message: {:?}", e),
    }
}

pub fn send_private_room_request(
    username: &str,
    room_code: &str,
    net: &Res<Network<WebSocketProvider>>,
) {
    let request = RoomJoinRequest {
        username: username.to_string(),
        room_code: room_code.to_string(),
    };

    match net.send_message(SERVER_CONNECTION_ID, request) {
        Ok(_) => info!("Sent random room request"),
        Err(e) => error!("Failed to send message: {:?}", e),
    }
}

pub fn send_start_game_request(room_id: u32, net: Res<Network<WebSocketProvider>>) {
    let request = StartGameRequest { room_id: room_id };

    match net.send_message(SERVER_CONNECTION_ID, request) {
        Ok(_) => info!("Sent start game request"),
        Err(e) => error!("Failed to send message: {:?}", e),
    }
}

pub fn send_completed_prompt(
    prompt_info_data: &mut PromptInfoDataRequest,
    prompt_index: usize,
    net: &Res<Network<WebSocketProvider>>,
) {
    prompt_info_data.state = PromptState::SentForFeedback;
    prompt_info_data.front_end_prompt_index = Some(prompt_index);
    match net.send_message(SERVER_CONNECTION_ID, prompt_info_data.clone()) {
        Ok(_) => info!("Sent completed prompts"),
        Err(e) => error!("Failed to send message: {:?}", e),
    }
}

pub fn send_bid_action(requestor_player_id: u32, room_id: u32, net: &Network<WebSocketProvider>) {
    match net.send_message(
        SERVER_CONNECTION_ID,
        GameActionRequest {
            requestor_player_id,
            target_player_id: 0,
            room_id,
            action: GameAction::Bid,
        },
    ) {
        Ok(_) => info!("Player: {} sent bid action", requestor_player_id),
        Err(e) => error!("Failed to send message: {:?}", e),
    }
}

// pub fn send_end_round_action(
//     requestor_player_id: u32,
//     room_id: u32,
//     net: &Network<WebSocketProvider>,
// ) {
//     match net.send_message(
//         SERVER_CONNECTION_ID,
//         GameActionRequest {
//             requestor_player_id,
//             target_player_id: 0,
//             room_id,
//             action: GameAction::EndRound,
//         },
//     ) {
//         Ok(_) => info!("Player: {} sent end round action", requestor_player_id),
//         Err(e) => error!("Failed to send message: {:?}", e),
//     }
// }

pub fn send_force_bid_action(
    requestor_player_id: u32,
    target_player_id: u32,
    room_id: u32,
    net: &Network<WebSocketProvider>,
) {
    match net.send_message(
        SERVER_CONNECTION_ID,
        GameActionRequest {
            requestor_player_id,
            target_player_id,
            room_id,
            action: GameAction::ForceBid,
        },
    ) {
        Ok(_) => info!("Player: {} sent force bid action", requestor_player_id),
        Err(e) => error!("Failed to send message: {:?}", e),
    }
}

// Receive message functions

fn room_state_response(
    mut new_messages: EventReader<NetworkData<RoomState>>,
    mut query: Query<&mut RoomState>,
    mut commands: Commands,
    state: Res<State<GameState>>,
    mut current_player_data: ResMut<CurrentPlayerData>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    for new_message in new_messages.read() {
        info!("Received new room state message: {:?}", new_message);
        let updated_players = &new_message.players;

        if state.get() != &new_message.game_state {
            next_state.set(new_message.game_state.clone());
        }

        // If an entity with room state exists, update it
        for mut room_state in query.iter_mut() {
            *room_state = new_message.additional_clone();
            return;
        }

        // Else create a new entity with room state

        // Find the player id of the current player, the last player added to the list
        let player_id = updated_players.last().unwrap().id;
        *current_player_data = CurrentPlayerData { player_id };

        commands.spawn(new_message.additional_clone());
    }
}

fn prompt_info_response(
    mut new_messages: EventReader<NetworkData<PromptInfoDataRequest>>,
    mut front_end_prompt_list: ResMut<FrontEndPromptList>,
) {
    for new_message in new_messages.read() {
        info!("Received new prompt info message: {:?}", new_message);
        if new_message.state == PromptState::Proposed {
            front_end_prompt_list
                .prompt_data_list
                .push(new_message.additional_clone());
        } else {
            if let Some(prompt_index) = new_message.front_end_prompt_index {
                if front_end_prompt_list.prompt_data_list.get(prompt_index).is_some() {
                    front_end_prompt_list.prompt_data_list[prompt_index] = new_message.additional_clone();
                } else {
                    error!("Prompt not found when accessing index");
                }
            } else {
                error!("Prompt index not found in prompt info response");
            }
        }
    }
}

fn round_end_info_response(
    mut new_messages: EventReader<NetworkData<RoundEndInfo>>,
    mut round_end_info_data: ResMut<RoundEndInfo>,
) {
    for new_message in new_messages.read() {
        info!("Received new round end info message: {:?}", new_message);
        *round_end_info_data = new_message.additional_clone();
    }
}

fn game_end_info_response(
    mut new_messages: EventReader<NetworkData<GameEndInfo>>,
    mut game_end_info_data: ResMut<GameEndInfo>,
) {
    for new_message in new_messages.read() {
        info!("Received new round end info message: {:?}", new_message);
        *game_end_info_data = new_message.additional_clone();
    }
}

fn game_player_notification_response(
    mut new_messages: EventReader<NetworkData<GamePlayerNotificationRequest>>,
    mut commands: Commands,
    mut timer: ResMut<RoundTimer>,
) {
    for new_message in new_messages.read() {
        info!("Received new round end info message: {:?}", new_message);
        commands.spawn(new_message.get_notification());

        match new_message.action {
            GameAction::Bid => {
                let current_duration = timer.0.duration().as_secs_f32();
                if timer.0.remaining_secs() < BID_INCREASE_TIMER_START_WINDOW {
                    timer.0.set_duration(Duration::from_secs(
                        (current_duration + BID_INCREASE_TIMER_VALUE) as u64,
                    ));
                }
            }
            GameAction::EndRound => {}
            GameAction::ForceBid => {}
        }
    }
}

// Etc. functions

fn handle_network_events(mut new_network_events: EventReader<NetworkEvent>) {
    for event in new_network_events.read() {
        info!("Received event");
        match event {
            NetworkEvent::Connected(conn_id) => {
                info!("Connected to server with id: {}", conn_id);
            }

            NetworkEvent::Disconnected(_) => {
                info!("Disconnected from server!");
            }
            NetworkEvent::Error(err) => {
                error!("Error: {:?}", err);
            }
        }
    }
}

fn setup_networking(
    net: ResMut<Network<WebSocketProvider>>,
    settings: Res<NetworkSettings>,
    task_pool: Res<EventworkRuntime<TaskPool>>,
) {

    let connect_string = if LOCAL_CONNECTION_MODE {
        "ws://127.0.0.1:8081"
    } else {
        "ws://52.180.68.180:8081"
    };

    info!("Setting up networking and wanting to connect at {}", connect_string);

    net.connect(
        url::Url::parse(connect_string).unwrap(),
        &task_pool.0,
        &settings,
    );
}

pub fn add_backend_server_connections(app: &mut App) {
    app.add_plugins(bevy_eventwork::EventworkPlugin::<
        WebSocketProvider,
        bevy::tasks::TaskPool,
    >::default())
        .insert_resource(EventworkRuntime(
            TaskPoolBuilder::new().num_threads(2).build(),
        ))
        .insert_resource(NetworkSettings::default())
        .add_systems(Update, handle_network_events)
        .add_systems(Startup, setup_networking)
        .listen_for_message::<RoomState, WebSocketProvider>()
        .add_systems(Update, room_state_response)
        .listen_for_message::<PromptInfoDataRequest, WebSocketProvider>()
        .add_systems(Update, prompt_info_response)
        .listen_for_message::<RoundEndInfo, WebSocketProvider>()
        .add_systems(Update, round_end_info_response)
        .listen_for_message::<GameEndInfo, WebSocketProvider>()
        .add_systems(Update, game_end_info_response)
        .listen_for_message::<GamePlayerNotificationRequest, WebSocketProvider>()
        .add_systems(Update, game_player_notification_response);
}
