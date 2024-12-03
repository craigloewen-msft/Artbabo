use std::char::MAX;

use bevy::prelude::*;
use bevy_eventwork::ConnectionId;
use bevy_eventwork::NetworkMessage;
use rand::Rng;
use serde::Deserialize;
use serde::Serialize;

use rand::seq::SliceRandom;
use rand::thread_rng;

pub const IMAGE_CREATION_TIME: f32 = 120.0;
pub const BIDDING_ROUND_TIME: f32 = 10.0;
pub const BIDDING_ROUND_END_TIME: f32 = 5.0;

pub const MIN_ART_VALUE: u32 = 100;
pub const MAX_ART_VALUE: u32 = 3500;

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
    pub money: i32,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtBidInfo {
    pub prompt_info: PromptInfoData,
    pub max_bid: u32,
    pub max_bid_player_id: u32,
    pub bid_increase_amount: u32,
    #[serde(skip)]
    pub art_value: u32,
}

impl Default for ArtBidInfo {
    fn default() -> Self {
        Self {
            prompt_info: Default::default(),
            max_bid: Default::default(),
            max_bid_player_id: Default::default(),
            bid_increase_amount: Default::default(),
            // Assign a random value to the art
            art_value: thread_rng().gen_range(MIN_ART_VALUE..MAX_ART_VALUE),
        }
    }
}

#[derive(Debug, Event, Clone, Serialize, Deserialize, Resource)]
pub struct RoundEndInfo {
    pub artist_name: String,
    pub bid_winner_name: String,
    pub winning_bid_amount: u32,
    pub art_value: u32,
}

impl NetworkMessage for RoundEndInfo {
    const NAME: &'static str = "RoundEndInfo";
}

impl Default for RoundEndInfo {
    fn default() -> Self {
        RoundEndInfo {
            artist_name: String::from("Unknown Artist"),
            bid_winner_name: String::from("No one"),
            winning_bid_amount: 0,
            art_value: 0,
        }
    }
}

impl RoundEndInfo {
    pub fn additional_clone(&self) -> Self {
        self.clone()
    }
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

    pub fn finalize_round(&mut self) -> Option<RoundEndInfo> {
        let mut round_end_info = RoundEndInfo::default();

        // Put the artist name down
        let artist_option = self
            .players
            .iter()
            .find(|player| player.id == self.current_art_bid.prompt_info.owner_id);

        if let Some(artist) = artist_option {
            round_end_info.artist_name = artist.username.clone();
        }

        // Record art value and winning bid amount
        round_end_info.art_value = self.current_art_bid.art_value;
        round_end_info.winning_bid_amount = self.current_art_bid.max_bid;

        // Check if max bid is greater than 0 and handle existing bid info
        if self.current_art_bid.max_bid > 0 {
            let winning_player = self
                .players
                .iter_mut()
                .find(|player| player.id == self.current_art_bid.max_bid_player_id);

            match winning_player {
                Some(player) => {
                    player.money += self.current_art_bid.art_value as i32 - self.current_art_bid.max_bid as i32;
                    // TODO: Add art to player's collection
                    round_end_info.bid_winner_name = player.username.clone();
                }
                None => {
                    error!(
                        "Could not find winning player with id {}",
                        self.current_art_bid.max_bid_player_id
                    );
                    return None;
                }
            }

            let current_prompt_info = &self.current_art_bid.prompt_info;

            // Award money to art creator
            let art_creator = self
                .players
                .iter_mut()
                .find(|player| player.id == current_prompt_info.owner_id);

            match art_creator {
                Some(player) => {
                    player.money += self.current_art_bid.max_bid as i32;
                    round_end_info.artist_name = player.username.clone();
                }
                None => {
                    error!(
                        "Could not find art creator with id {}",
                        current_prompt_info.owner_id
                    );
                    return None;
                }
            }
        }

        return Some(round_end_info);
    }

    pub fn setup_next_round(&mut self) {
        // Move the prompt to the completed list
        self.used_prompts.push(std::mem::replace(
            &mut self.current_art_bid.prompt_info,
            PromptInfoData::default(),
        ));

        info!(
            "After removing prompt, current prompt_list: {:?}",
            self.remaining_prompts,
        );

        if !self.remaining_prompts.is_empty() {
            // Prepare the next bid info
            self.current_art_bid = ArtBidInfo::default();
            self.current_art_bid.bid_increase_amount = 100;

            // Prepare next round info
            // Choose a random prompt
            let random_prompt_number = rand::random::<u32>() % self.remaining_prompts.len() as u32;

            // Remove the prompt from the remaining prompts and insert it into current art bid
            let random_prompt = self.remaining_prompts.remove(random_prompt_number as usize);
            self.current_art_bid.prompt_info = random_prompt;

            info!(
                "Picked prompt {:?} for next round in room {}",
                self.current_art_bid.prompt_info, self.room_id
            );

            info!(
                "After progressing to new round, current art bid info: {:?}",
                self
            );
        }
    }

    pub fn disconnect_player(&mut self, player_id: ConnectionId) {
        let player_index = self
            .players
            .iter()
            .position(|player| player.id == player_id.id);

        match player_index {
            Some(index) => {
                self.players.remove(index);
            }
            None => {
                error!("Could not find player with id {}", player_id);
            }
        }
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
