use serde::{Deserialize, Serialize};
use ts_rs::TS;
#[derive(Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/generated/library/")]
pub struct ReplayJob {
    pub id: String,
    pub state: String,
    pub received: f64,
    pub total: Option<f64>,
    pub version: Option<String>,
    pub error: Option<String>,
}
