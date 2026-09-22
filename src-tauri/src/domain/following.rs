use super::{review::LibraryEntry, Account};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/generated/library/")]
pub struct Subscription {
    pub id: String,
    pub account: Account,
    pub server: String,
    pub label: String,
    pub paused: bool,
    pub last_synced: Option<f64>,
    pub last_error: Option<String>,
    pub limited: bool,
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/generated/library/")]
pub struct FollowingState {
    pub subscriptions: Vec<Subscription>,
    pub syncing: bool,
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/generated/library/")]
pub struct FeedEntry {
    pub subscription_id: String,
    pub game: LibraryEntry,
}
