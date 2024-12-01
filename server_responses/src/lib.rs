use bevy::prelude::*;
use bevy_eventwork::NetworkMessage;
use serde::Deserialize;
use serde::Serialize;

use rand::seq::SliceRandom;
use rand::thread_rng;

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

    pub fn finalize_and_setup_new_round(&mut self) {
        let current_art_bid_number = self.current_art_bid.art_bid_number;

        // Check if max bid is greater than 0 and handle existing bid info
        if self.current_art_bid.max_bid > 0 {
            let winning_player = self
                .players
                .iter_mut()
                .find(|player| player.id == self.current_art_bid.max_bid_player_id);

            match winning_player {
                Some(player) => {
                    player.money -= self.current_art_bid.max_bid;
                    // TODO: Add art to player's collection
                }
                None => {
                    error!(
                        "Could not find winning player with id {}",
                        self.current_art_bid.max_bid_player_id
                    );
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
                    player.money += self.current_art_bid.max_bid;
                }
                None => {
                    error!(
                        "Could not find art creator with id {}",
                        current_prompt_info.owner_id
                    );
                }
            }

            // Move the prompt to the completed list
            self.used_prompts.push(std::mem::replace(
                &mut self.current_art_bid.prompt_info,
                PromptInfoData::default(),
            ));

            info!(
                "After removing prompt, current prompt_list: {:?}",
                self.remaining_prompts,
            );
        }

        if !self.remaining_prompts.is_empty() {
            // Prepare the next bid info
            self.current_art_bid = ArtBidInfo::default();
            self.current_art_bid.bid_increase_amount = 100;
            self.current_art_bid.art_bid_number = current_art_bid_number + 1;

            // Prepare next round info
            // Choose a random prompt
            let random_prompt_number =
                rand::random::<u32>() % self.remaining_prompts.len() as u32;

            // Remove the prompt from the remaining prompts and insert it into current art bid
            let random_prompt = self
                .remaining_prompts
                .remove(random_prompt_number as usize);
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
