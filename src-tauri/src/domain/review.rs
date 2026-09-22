use crate::domain::Account;
use serde::Serialize;
use serde_json::Value;
use ts_rs::TS;

#[derive(Clone)]
pub struct HistoryPage {
    pub account: Account,
    pub server: String,
    pub games: Vec<Value>,
    pub start: u32,
    pub has_more: bool,
    pub skipped_games: u32,
    pub source: String,
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/generated/library/")]
pub struct SearchResult {
    pub account: Account,
    pub server: String,
    pub games: Vec<SearchGame>,
    pub start: u32,
    pub has_more: bool,
    pub skipped_games: u32,
    pub source: String,
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "../../src/generated/library/")]
pub struct SearchGame {
    pub game_id: String,
    pub game: crate::domain::game::GameSummary,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(serde::Deserialize))]
pub struct StoredReview {
    pub id: String,
    pub server: String,
    pub account: Account,
    pub detail: Value,
    pub timeline: Option<Value>,
    pub source: String,
    pub timeline_error: Option<String>,
    pub bookmarked: bool,
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/generated/library/")]
pub struct LibraryEntry {
    pub id: String,
    pub server: String,
    pub account: Account,
    pub champion_id: u32,
    pub started_at: f64,
    pub duration: u32,
    pub win: bool,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub version: String,
    pub bookmarked: bool,
    pub items: Vec<u32>,
    pub queue_id: u32,
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/generated/library/")]
pub struct ReviewGame {
    pub id: String,
    pub game_id: String,
    pub server: String,
    pub account: Account,
    pub game: crate::domain::game::GameSummary,
    pub queue_name: String,
    pub timeline: Option<Vec<ReviewEvent>>,
    pub source: String,
    pub timeline_error: Option<String>,
    pub bookmarked: bool,
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "../../src/generated/library/")]
pub struct ReviewEvent {
    pub id: String,
    pub at: u32,
    pub participant_id: u32,
    pub kind: EventKind,
    pub item_id: Option<u32>,
    pub restored_item_id: Option<u32>,
    pub label: EventLabel,
    pub monster: String,
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "../../src/generated/library/")]
pub enum EventKind {
    Purchase,
    Sale,
    Undo,
    Destroy,
    Kill,
    Objective,
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "../../src/generated/library/")]
pub enum EventLabel {
    Purchase,
    Sale,
    UndoSale,
    UndoPurchase,
    Undo,
    Destroy,
    Kill,
    Objective,
}
