use bevy::prelude::*;

#[derive(Resource)]
pub struct PlayerSettings {
    pub username: String,
    pub button_submitted: bool,
}

#[derive(Resource)]
pub struct CurrentPlayerData {
    pub player_id: u32,
}