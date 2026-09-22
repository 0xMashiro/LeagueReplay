use crate::client::lcu::LocalClient;
pub use crate::regions::{check_history_access, endpoint, platform, Region};

pub fn current(client: &LocalClient) -> String {
    if client.region == "TENCENT" {
        return format!("TENCENT_{}", client.platform.trim_start_matches("TENCENT_"));
    }
    match client.platform.as_str() {
        "EUW1" => "EUW",
        "JP1" => "JP",
        "PBE1" => "PBE",
        other => other,
    }
    .to_owned()
}

pub fn list(client: Option<&LocalClient>) -> Vec<Region> {
    let source = client.map(current).unwrap_or_default();
    crate::regions::list_for_source(&source)
}
