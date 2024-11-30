use bevy::prelude::*;
use bevy_eventwork::NetworkMessage;
use serde::Deserialize;
use serde::Serialize;

pub const IMAGE_CREATION_TIME: f32 = 120.0;
pub const BIDDING_ROUND_TIME: f32 = 10.0;
pub const BIDDING_ROUND_END_TIME: f32 = 5.0;

#[derive(Component, Resource)]
pub struct RoundTimer(pub Timer);

pub trait HasRoomId {
    fn room_id(&self) -> u32;
}

#[derive(States, Clone, Eq, PartialEq, Debug, Hash, Serialize, Deserialize, Default)]
pub enum GameState {
    #[default]
    Intro,
    WaitingRoom,
    ImageCreation,
    ImageGeneration,
    BiddingRound,
    BiddingRoundEnd,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Player {
    pub username: String,
    #[serde(skip)]
    pub money: u32,
    pub id: u32,
}

// Make a constructor for Player with a string input
impl Player {
    pub fn new(id: u32, username: String) -> Self {
        Self {
            username,
            money: 3000,
            id,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ArtBidInfo {
    pub prompt_info: PromptInfoData,
    pub max_bid: u32,
    pub max_bid_player_id: u32,
    pub bid_increase_amount: u32,
    pub art_bid_number: u32,
}

#[derive(Debug, Event, Clone, Serialize, Deserialize, Default)]
pub struct RoomState {
    pub room_id: u32,
    pub players: Vec<Player>,
    pub game_state: GameState,
    pub current_art_bid: ArtBidInfo,
    pub prompts_per_player: u32,
    #[serde(skip)]
    pub remaining_prompts: Vec<PromptInfoData>,
    pub used_prompts: Vec<PromptInfoData>,
    pub received_prompt_count: u32,
}

impl NetworkMessage for RoomState {
    const NAME: &'static str = "RoomState";
}

impl RoomState {
    // Need this due to the networking event system not showing clone well
    pub fn additional_clone(&self) -> Self {
        self.clone()
    }
}

#[derive(Debug, Event, Clone, Serialize, Deserialize, Default)]
pub struct RoomJoinRequest {
    pub username: String,
    pub room_id: u32,
}

impl NetworkMessage for RoomJoinRequest {
    const NAME: &'static str = "RoomCreationRequest";
}

#[derive(Debug, Event, Clone, Serialize, Deserialize, Default)]
pub struct StartGameRequest {
    pub room_id: u32,
}

impl NetworkMessage for StartGameRequest {
    const NAME: &'static str = "StartGameRequest";
}

impl HasRoomId for StartGameRequest {
    fn room_id(&self) -> u32 {
        self.room_id
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PromptInfoData {
    pub prompt_text: String,
    pub prompt_answer: String,
    pub image_url: String,
    pub owner_id: u32,
}

#[derive(Debug, Event, Clone, Serialize, Deserialize, Resource, Default)]
pub struct PromptInfoDataRequest {
    pub prompt_list: Vec<PromptInfoData>,
    pub room_id: u32,
}

impl HasRoomId for PromptInfoDataRequest {
    fn room_id(&self) -> u32 {
        self.room_id
    }
}

impl PromptInfoDataRequest {
    // Need this due to the networking event system not showing clone well
    pub fn additional_clone(&self) -> Self {
        self.clone()
    }
}

impl NetworkMessage for PromptInfoDataRequest {
    const NAME: &'static str = "PromptInfoDataRequest";
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GameAction {
    Bid,
    EndRound,
}

#[derive(Debug, Event, Clone, Serialize, Deserialize)]
pub struct GameActionRequest {
    pub room_id: u32,
    pub player_id: u32,
    pub action: GameAction,
}

impl NetworkMessage for GameActionRequest {
    const NAME: &'static str = "GameActionRequest";
}

impl HasRoomId for GameActionRequest {
    fn room_id(&self) -> u32 {
        self.room_id
    }
}

#[derive(PartialEq, Eq, Debug, Default)]
pub enum TaskCompletionStatus {
    #[default]
    InProgress,
    Completed,
    Error,
}