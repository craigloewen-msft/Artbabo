use bevy::prelude::*;
use server_responses::PromptInfoDataRequest;

#[derive(Resource)]
pub struct PlayerSettings {
    pub username: String,
}

#[derive(Resource)]
pub struct CurrentPlayerData {
    pub player_id: u32,
}

#[derive(Resource, Default)]
pub struct FrontEndPromptList {
    pub prompt_data_list: Vec<PromptInfoDataRequest>,
}