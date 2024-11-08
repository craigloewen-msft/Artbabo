use bevy_eventwork::NetworkMessage;
use serde::Deserialize;
use serde::Serialize;
use bevy::prelude::*;

#[derive(Debug, Event, Clone, Serialize, Deserialize, Default)]
pub struct RoomCreationRequest {
    pub username: String,
    pub room_id: String,
}

#[derive(Debug, Event, Clone, Serialize, Deserialize, Default)]
pub struct RoomCreationResponse {
    pub success: bool,
}

impl NetworkMessage for RoomCreationResponse {
    const NAME: &'static str = "example:UserChatMessage";
}