use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RoomCreationRequest {
    pub username: String,
    pub room_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RoomCreationResponse {
    pub success: bool,
}