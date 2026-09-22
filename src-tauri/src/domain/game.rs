use serde::Serialize;
use ts_rs::TS;

#[derive(Clone, Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "../../src/generated/live/")]
pub struct GameSummary {
    pub started_at: f64,
    pub duration: u32,
    pub queue_id: u32,
    pub version: String,
    pub participants: Vec<GameParticipant>,
}

#[derive(Clone, Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "../../src/generated/live/")]
pub struct GameParticipant {
    pub id: u32,
    pub puuid: Option<String>,
    pub name: Option<String>,
    pub team: u32,
    pub placement: Option<u32>,
    pub champion_id: u32,
    pub role: String,
    pub win: Option<bool>,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub cs: u32,
    pub gold: u32,
    pub damage: u32,
    pub items: Vec<u32>,
    pub double_kills: u32,
    pub triple_kills: u32,
    pub quadra_kills: u32,
    pub penta_kills: u32,
}
