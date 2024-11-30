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

const SERVER_CONNECTION_ID: ConnectionId = ConnectionId { id: 0 };

// Send message functions

pub fn send_random_room_request(username: &str, net: Res<Network<WebSocketProvider>>) {
    let request = RoomJoinRequest {
        username: username.to_string(),
        room_id: 0,
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

pub fn send_completed_prompts(prompt_info_data: PromptInfoDataRequest, net: Res<Network<WebSocketProvider>>) {
    match net.send_message(SERVER_CONNECTION_ID, prompt_info_data) {
        Ok(_) => info!("Sent completed prompts"),
        Err(e) => error!("Failed to send message: {:?}", e),
    }
}

pub fn send_bid_action(player_id: u32, room_id: u32, net: &Network<WebSocketProvider>) {
    match net.send_message(SERVER_CONNECTION_ID, GameActionRequest { player_id, room_id, action: GameAction::Bid }) {
        Ok(_) => info!("Player: {} sent bid action", player_id),
        Err(e) => error!("Failed to send message: {:?}", e),
    }
}

pub fn send_end_round_action(player_id: u32, room_id: u32, net: &Network<WebSocketProvider>) {
    match net.send_message(SERVER_CONNECTION_ID, GameActionRequest { player_id, room_id, action: GameAction::EndRound }) {
        Ok(_) => info!("Player: {} sent bid action", player_id),
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
    mut prompt_info_data: ResMut<PromptInfoDataRequest>,
) {
    for new_message in new_messages.read() {
        info!("Received new prompt info message: {:?}", new_message);
        *prompt_info_data = new_message.additional_clone();
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
    info!("Setting up networking and wanting to connect");
    net.connect(
        url::Url::parse("ws://127.0.0.1:8081").unwrap(),
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
        .add_systems(Update, prompt_info_response);
}
