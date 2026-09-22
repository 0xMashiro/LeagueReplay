pub mod following;
pub mod game;
pub mod replay;
pub mod review;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "../../src/generated/live/")]
pub struct Account {
    pub id: String,
    pub platform: String,
    pub puuid: String,
    pub summoner_id: String,
    pub riot_id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "../../src/generated/live/")]
pub struct SavedAccount {
    pub account: Account,
    pub server: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "../../src/generated/live/")]
pub struct ClientStatus {
    pub state: String,
    pub message: String,
    pub phase: Option<String>,
    pub account: Option<Account>,
    pub active_game_id: Option<String>,
    pub last_checked_at: f64,
}

impl ClientStatus {
    pub fn unavailable(state: &str, message: &str, now: i64) -> Self {
        Self {
            state: state.into(),
            message: message.into(),
            phase: None,
            account: None,
            active_game_id: None,
            last_checked_at: now as f64,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ObservedGame {
    pub id: String,
    pub game_id: String,
    pub champion_id: u32,
    pub queue_name: String,
}

pub struct Sample {
    pub account: Account,
    pub phase: String,
    pub observed_game: Option<ObservedGame>,
    pub queue_active: bool,
}

impl Sample {
    pub fn is_playing(&self) -> bool {
        self.observed_game.is_some() && matches!(self.phase.as_str(), "InProgress" | "Reconnect")
    }
}

#[derive(Clone, Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "../../src/generated/live/")]
pub struct ObservedMatch {
    pub id: String,
    pub observation_id: String,
    pub platform: String,
    pub game_id: String,
    pub account_id: String,
    pub segment_id: String,
    pub first_seen: f64,
    pub last_seen: f64,
    pub champion_id: u32,
    pub queue_name: String,
    pub state: String,
    pub game: Option<game::GameSummary>,
    pub data_error: Option<String>,
    pub automatic: bool,
    pub manual: bool,
}

#[derive(Clone, Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "../../src/generated/live/")]
pub struct SessionSegment {
    pub id: String,
    pub account_id: String,
    pub match_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "../../src/generated/live/")]
pub struct PlaySession {
    pub id: String,
    pub title: String,
    pub started_at: f64,
    pub ended_at: Option<f64>,
    pub end_reason: Option<String>,
    pub segments: Vec<SessionSegment>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "../../src/generated/live/")]
pub struct LiveUiState {
    #[serde(default)]
    pub my_accounts: Vec<SavedAccount>,
    pub notes: Vec<LiveNote>,
    pub reviewed: Vec<String>,
    pub theme: Option<String>,
    #[serde(default)]
    pub players: Vec<PlayerProfile>,
    #[serde(default)]
    pub premades: Vec<PremadeMark>,
    #[serde(default)]
    pub identity_links: Vec<PlayerIdentityLink>,
}

#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "../../src/generated/live/")]
pub struct PlayerIdentityLink {
    pub id: String,
    pub player_id: String,
    pub account_id: String,
    pub match_id: Option<String>,
    pub from: Option<f64>,
    pub to: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "../../src/generated/live/")]
pub struct PlayerProfile {
    pub id: String,
    pub name: String,
    pub account_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "../../src/generated/live/")]
pub struct PremadeMark {
    pub match_id: String,
    pub account_id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "../../src/generated/live/")]
pub struct LiveNote {
    pub id: String,
    pub match_id: String,
    pub body: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub at: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub participant_id: Option<u32>,
    pub tags: Vec<String>,
    pub updated_at: String,
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/generated/live/")]
pub struct LiveWorkspace {
    pub status: ClientStatus,
    pub accounts: Vec<Account>,
    pub matches: Vec<ObservedMatch>,
    pub sessions: Vec<PlaySession>,
    pub ui: LiveUiState,
    pub ui_revision: u32,
    pub idle_minutes: u32,
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/generated/live/")]
pub struct WorkspaceUpdate {
    pub cursor: f64,
    pub reset: bool,
    pub removed: Vec<String>,
    pub status: ClientStatus,
    pub workspace: Option<LiveWorkspace>,
}

pub fn validate_game_id(value: &str) -> Result<(), String> {
    value
        .parse::<u64>()
        .ok()
        .filter(|v| *v > 0 && v.to_string() == value)
        .map(|_| ())
        .ok_or("game.invalidId".into())
}

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
pub mod resources;

#[derive(Clone, Copy, Default, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/generated/live/")]
pub enum SessionScope {
    #[default]
    All,
    Current,
    History,
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/generated/live/")]
pub struct PlayPage {
    pub page: u32,
    pub total_sessions: u32,
    pub summary: PlaySummary,
    pub sessions: Vec<PlaySession>,
    pub matches: Vec<ObservedMatch>,
}

#[derive(Clone, Copy, Default, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "../../src/generated/live/")]
pub enum PlaySource {
    #[default]
    All,
    Automatic,
    Manual,
}

#[derive(Clone, Default, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/generated/live/")]
pub struct PlayQuery {
    pub page: u32,
    pub scope: SessionScope,
    pub text: String,
    pub champion_names: std::collections::BTreeMap<u32, String>,
    pub player: Option<String>,
    pub only_premade: bool,
    pub account: Option<String>,
    pub win: Option<bool>,
    pub source: PlaySource,
    pub from: Option<f64>,
    pub to: Option<f64>,
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "../../src/generated/live/")]
pub struct PlaySummary {
    pub sessions: u32,
    pub games: u32,
    pub account_ids: Vec<String>,
    pub settled: u32,
    pub wins: u32,
    pub note_count: u32,
}
