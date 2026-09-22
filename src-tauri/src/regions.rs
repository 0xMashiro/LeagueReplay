use serde::Serialize;
use serde_json::Value;
use std::sync::OnceLock;
use ts_rs::TS;

// Snapshot of LeagueAkari's version-2 built-in configuration; see third-party notice.
fn config() -> &'static Value {
    static CONFIG: OnceLock<Value> = OnceLock::new();
    CONFIG.get_or_init(|| {
        serde_json::from_str(include_str!("client/league-servers.json"))
            .expect("bundled region config")
    })
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/generated/library/")]
pub struct Region {
    pub id: String,
    pub platform: String,
    pub name: String,
    pub english_name: String,
    pub available: bool,
    pub current: bool,
}

pub fn server_for_platform(value: &str) -> Result<String, String> {
    validate_code(value.strip_prefix("TENCENT_").unwrap_or(value))?;
    Ok(match value {
        "EUW1" => "EUW".into(),
        "JP1" => "JP".into(),
        "PBE1" => "PBE".into(),
        value if legacy_tencent_platform(value) => format!("TENCENT_{value}"),
        value => value.into(),
    })
}

fn validate_code(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 32
        || !value
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
    {
        return Err("region.unsupported".into());
    }
    Ok(())
}

// Keep existing archive identities stable without depending on SGP routing.
fn legacy_tencent_platform(value: &str) -> bool {
    ["HN", "WT", "TJ", "NJ", "GZ", "CQ", "BGP"]
        .iter()
        .any(|prefix| {
            value
                .strip_prefix(prefix)
                .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
        })
}

pub fn platform(id: &str) -> Result<String, String> {
    validate_code(id.strip_prefix("TENCENT_").unwrap_or(id))?;
    let value = match id {
        "EUW" => "EUW1",
        "JP" => "JP1",
        "PBE" => "PBE1",
        // Unknown Tencent codes retain their namespace, just like its test realms.
        _ => id
            .strip_prefix("TENCENT_")
            .filter(|v| legacy_tencent_platform(v))
            .unwrap_or(id),
    };
    if server_for_platform(value)? != id {
        return Err("region.unsupported".into());
    }
    Ok(value.into())
}

// Entitlements-backed match history, detail, timeline and replay endpoints only.
// This does not grant access to regional summoner/profile or ranked endpoints.
pub fn check_history_access(source: &str, target: &str) -> Result<(), String> {
    if source.is_empty() {
        return Err("client.Offline".into());
    }
    if !config()["servers"][source].is_object()
        || !config()["servers"][target]["matchHistory"].is_string()
    {
        return Err("region.unsupported".into());
    }
    if source.starts_with("TENCENT_") != target.starts_with("TENCENT_") {
        return Err("region.operatorMismatch".into());
    }
    Ok(())
}

pub fn list_for_source(source: &str) -> Vec<Region> {
    let mut ids: Vec<_> = config()["servers"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    if !source.is_empty() && platform(source).is_ok() && !ids.iter().any(|id| id == source) {
        ids.push(source.into());
    }
    ids.iter()
        .map(|id| Region {
            id: id.clone(),
            platform: platform(id).unwrap(),
            name: config()["serverNames"]["zh-CN"][id]
                .as_str()
                .unwrap_or(id)
                .into(),
            english_name: config()["serverNames"]["en"][id]
                .as_str()
                .unwrap_or(id)
                .into(),
            available: source == id || check_history_access(source, id).is_ok(),
            current: source == *id,
        })
        .collect()
}

// A path alias is identity metadata, not permission to send a remote request.
pub fn path_region(id: &str) -> Result<String, String> {
    platform(id)?;
    Ok(config()["servers"][id]["regionPathParam"]
        .as_str()
        .unwrap_or(id.strip_prefix("TENCENT_").unwrap_or(id))
        .into())
}

pub fn endpoint(id: &str, history: bool) -> Result<(&'static str, String), String> {
    let entry = &config()["servers"][id];
    let host = entry[if history { "matchHistory" } else { "common" }]
        .as_str()
        .ok_or("region.unsupported")?;
    // Hosts come only from the embedded configuration, never IPC or a remote redirect.
    let sub = entry["regionPathParam"]
        .as_str()
        .unwrap_or(id.strip_prefix("TENCENT_").unwrap_or(id));
    Ok((host, sub.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_identity_and_current_region_do_not_require_sgp_routes() {
        for id in ["EUN1", "ME1", "TENCENT_HN20", "TENCENT_NEWREALM"] {
            let p = platform(id).unwrap();
            assert_eq!(server_for_platform(&p).unwrap(), id);
            assert!(endpoint(id, true).is_err());
            assert!(check_history_access(id, id).is_err());
            let items = list_for_source(id);
            let local = items.iter().find(|r| r.id == id).unwrap();
            assert!(local.current && local.available);
            assert!(items.iter().filter(|r| r.id != id).all(|r| !r.available));
        }
        for id in config()["servers"].as_object().unwrap().keys() {
            assert_eq!(server_for_platform(&platform(id).unwrap()).unwrap(), *id);
        }
        for invalid in [
            "",
            "https://example.com",
            "../NA1",
            "na1",
            "TENCENT_",
            "EUW1",
        ] {
            assert!(platform(invalid).is_err());
        }
        assert_eq!(platform("TENCENT_HN1").unwrap(), "HN1");
        assert_eq!(platform("TENCENT_PBE").unwrap(), "TENCENT_PBE");
        assert_eq!(platform("PBE").unwrap(), "PBE1");
    }

    #[test]
    fn history_access_preserves_operator_and_configuration_boundaries() {
        for (source, target) in [
            ("TENCENT_HN1", "TENCENT_HN10"),
            ("NA1", "KR"),
            ("JP", "BR1"),
        ] {
            assert!(check_history_access(source, target).is_ok());
            assert!(
                list_for_source(source)
                    .iter()
                    .find(|r| r.id == target)
                    .unwrap()
                    .available
            );
        }
        for (source, target) in [("TENCENT_HN1", "KR"), ("KR", "TENCENT_HN1")] {
            assert_eq!(
                check_history_access(source, target),
                Err("region.operatorMismatch".into())
            );
            assert!(
                !list_for_source(source)
                    .iter()
                    .find(|r| r.id == target)
                    .unwrap()
                    .available
            );
        }
        for (source, target) in [
            ("NA1", "UNKNOWN"),
            ("UNKNOWN", "NA1"),
            ("TENCENT_UNKNOWN", "TENCENT_HN1"),
        ] {
            assert_eq!(
                check_history_access(source, target),
                Err("region.unsupported".into())
            );
        }
        assert_eq!(check_history_access("", "KR"), Err("client.Offline".into()));
        assert!(list_for_source("").iter().all(|r| !r.available));
        assert!(endpoint("https://example.com", true).is_err());
        assert_ne!(platform("PBE").unwrap(), platform("TENCENT_PBE").unwrap());
    }
}
