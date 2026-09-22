use crate::domain::Account;
mod projection;
mod review;
use crate::regions;
pub use projection::{project, search_result};
pub use review::review_game;
use serde_json::{json, Value};

pub fn summary(server: &str, payload: &Value) -> Result<Value, String> {
    let game = payload.get("json").unwrap_or(payload);
    let platform = regions::platform(server)?;
    let actual = game["platformId"].as_str().ok_or("game.invalidData")?;
    let sub = regions::path_region(server)?;
    if ![platform.as_str(), sub.as_str(), server]
        .iter()
        .any(|v| actual.eq_ignore_ascii_case(v))
    {
        return Err("game.identityMismatch".into());
    }
    let game_id = game["gameId"]
        .as_u64()
        .filter(|v| *v > 0)
        .ok_or("game.invalidData")?;
    let people = game["participants"]
        .as_array()
        .filter(|v| !v.is_empty())
        .ok_or("game.invalidData")?;
    if let Some(identity) = payload["metadata"]["match_id"].as_str() {
        if ![
            format!("{sub}_{game_id}"),
            format!("{platform}_{game_id}"),
            format!("{server}_{game_id}"),
        ]
        .iter()
        .any(|v| v.eq_ignore_ascii_case(identity))
        {
            return Err("game.identityMismatch".into());
        }
    }
    let mut normalized = game.clone();
    normalized["platformId"] = json!(platform);
    if !game["participantIdentities"].is_array() {
        let identities: Vec<_> = people.iter().map(|p| json!({
            "participantId":p["participantId"],
            "player":{"puuid":p["puuid"],"summonerId":p["summonerId"],"gameName":p["riotIdGameName"],"tagLine":p["riotIdTagline"],"summonerName":p["summonerName"],"platformId":platform}
        })).collect();
        let participants: Vec<_> = people.iter().map(|p| json!({
            "participantId":p["participantId"],"teamId":p["teamId"],"championId":p["championId"],"spell1Id":p["summoner1Id"],"spell2Id":p["summoner2Id"],
            "stats":p,"timeline":{"lane":p["teamPosition"],"role":p["role"]}
        })).collect();
        normalized["participantIdentities"] = json!(identities);
        normalized["participants"] = json!(participants);
    }
    Ok(normalized)
}

pub fn lcu_timeline(server: &str, game_id: &str, payload: Value) -> Result<Value, String> {
    // LCU returns { frames }; identity comes from the requested endpoint.
    if payload.get("json").is_some() {
        return Err("game.invalidTimeline".into());
    }
    validate_timeline(server, game_id, &payload, &payload, false)?;
    Ok(payload)
}

pub fn sgp_timeline(server: &str, game_id: &str, mut payload: Value) -> Result<Value, String> {
    let value = payload.get("json").ok_or("game.invalidTimeline")?;
    validate_timeline(server, game_id, &payload, value, true)?;
    Ok(payload["json"].take())
}

fn validate_timeline(
    server: &str,
    game_id: &str,
    payload: &Value,
    value: &Value,
    require_id: bool,
) -> Result<(), String> {
    crate::domain::validate_game_id(game_id)?;
    if (require_id || value.get("gameId").is_some())
        && value["gameId"].as_u64().map(|v| v.to_string()).as_deref() != Some(game_id)
    {
        return Err("game.invalidTimeline".into());
    }
    let frames = value["frames"].as_array().ok_or("game.invalidTimeline")?;
    if frames
        .iter()
        .any(|frame| !frame.is_object() || !frame["events"].is_array())
    {
        return Err("game.invalidTimeline".into());
    }
    if let Some(identity) = payload.get("metadata").and_then(|m| m.get("match_id")) {
        let id = identity.as_str().ok_or("game.identityMismatch")?;
        let sub = regions::path_region(server)?;
        let platform = regions::platform(server)?;
        if ![
            format!("{sub}_{game_id}"),
            format!("{platform}_{game_id}"),
            format!("{server}_{game_id}"),
        ]
        .iter()
        .any(|expected| id.eq_ignore_ascii_case(expected))
        {
            return Err("game.identityMismatch".into());
        }
    }
    Ok(())
}

pub fn account_in_game(platform: &str, puuid: &str, detail: &Value) -> Result<Account, String> {
    let player = detail["participantIdentities"]
        .as_array()
        .and_then(|v| {
            v.iter()
                .find(|p| p["player"]["puuid"].as_str() == Some(puuid))
        })
        .ok_or("game.playerMissing")?["player"]
        .clone();
    participant_account(platform, &player)
}

fn participant_account(platform: &str, player: &Value) -> Result<Account, String> {
    if !player.is_object() {
        return Err("game.invalidData".into());
    }
    let mut player = player.clone();
    if let Some(id) = player["summonerId"]
        .as_str()
        .and_then(|s| s.parse::<u64>().ok())
    {
        player["summonerId"] = json!(id);
    }
    player["displayName"] = player["summonerName"].clone();
    account_from_profile(platform, &player)
}

pub fn participant_accounts(platform: &str, detail: &Value) -> Vec<Account> {
    detail["participantIdentities"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|identity| participant_account(platform, &identity["player"]).ok())
        .collect()
}

pub fn account_from_profile(platform: &str, value: &Value) -> Result<Account, String> {
    let puuid = value["puuid"]
        .as_str()
        .filter(|v| !v.is_empty() && v.len() <= 128 && *v != "00000000-0000-0000-0000-000000000000")
        .ok_or("game.invalidData")?;
    let summoner_id = value["summonerId"]
        .as_u64()
        .filter(|v| *v > 0)
        .ok_or("game.invalidData")?;
    let game_name = value["gameName"].as_str().filter(|v| !v.is_empty());
    let tag = value["tagLine"].as_str().filter(|v| !v.is_empty());
    let riot_id = match (game_name, tag) {
        (Some(name), Some(tag)) => format!("{name}#{tag}"),
        _ => value["displayName"]
            .as_str()
            .filter(|v| !v.is_empty())
            .ok_or("game.invalidData")?
            .to_owned(),
    };
    Ok(Account {
        id: format!("{platform}:{puuid}"),
        platform: platform.into(),
        puuid: puuid.into(),
        summoner_id: summoner_id.to_string(),
        riot_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timeline_sources_have_distinct_contracts_and_verify_supplied_identities() {
        let raw = json!({"frames":[{"events":[{"type":"ITEM_PURCHASED","timestamp":42000,"participantId":1,"itemId":1001}]}]});
        let sgp =
            json!({"metadata":{"match_id":"HN1_42"},"json":{"gameId":42,"frames":raw["frames"]}});
        assert_eq!(lcu_timeline("TENCENT_HN1", "42", raw.clone()).unwrap(), raw);
        assert_eq!(
            sgp_timeline("TENCENT_HN1", "42", sgp.clone()).unwrap(),
            sgp["json"]
        );
        assert!(lcu_timeline("TENCENT_HN1", "42", json!({"frames":[]})).is_ok());
        assert!(sgp_timeline("TENCENT_HN1", "42", raw).is_err());
        assert!(lcu_timeline("TENCENT_HN1", "42", sgp).is_err());
        for invalid in [
            json!({"gameId":43,"frames":[]}),
            json!({"gameId":null,"frames":[]}),
            json!({"metadata":{"match_id":null},"frames":[]}),
            json!({"json":{"frames":[]}}),
            json!({"metadata":{"match_id":"HN10_42"},"json":{"gameId":42,"frames":[]}}),
            json!({"frames":{}}),
            json!({"frames":[null]}),
            json!({"frames":[{"events":{}}]}),
            json!({}),
        ] {
            assert!(lcu_timeline("TENCENT_HN1", "42", invalid.clone()).is_err());
            assert!(sgp_timeline("TENCENT_HN1", "42", invalid).is_err());
        }
        for value in [
            json!({"frames":[]}),
            json!({"gameId":43,"frames":[]}),
            json!({"gameId":42,"frames":[null]}),
        ] {
            assert!(sgp_timeline("TENCENT_HN1", "42", json!({"json":value})).is_err());
        }
    }
    #[test]
    fn participant_identity_normalization_is_shared_and_skips_incomplete_players() {
        let game = json!({"participantIdentities":[
            {"player":{"puuid":"a","summonerId":"12","gameName":"Player","tagLine":"TEST"}},
            {"player":{"puuid":"b","summonerId":13,"summonerName":"Previous name"}},
            {"player":{"puuid":"missing-id","gameName":"Incomplete","tagLine":"TEST"}},
            {"player":{"puuid":"00000000-0000-0000-0000-000000000000","summonerId":14,"summonerName":"Invalid"}},
            {"player":null},{"player":[]},{"player":"invalid"}
        ]});
        let accounts = participant_accounts("HN1", &game);
        assert_eq!(accounts.len(), 2);
        assert_eq!(accounts[0].id, "HN1:a");
        assert_eq!(accounts[0].summoner_id, "12");
        assert_eq!(accounts[0].riot_id, "Player#TEST");
        assert_eq!(accounts[1].riot_id, "Previous name");
        assert_eq!(
            account_in_game("HN1", "a", &game).unwrap().id,
            accounts[0].id
        );
        assert_ne!(participant_accounts("HN10", &game)[0].id, accounts[0].id);
        assert!(account_in_game("HN1", "missing-id", &game).is_err());
    }
    #[test]
    fn projection_preserves_stats_and_checks_region_and_identity() {
        let raw = json!({"metadata":{"match_id":"HN1_42"},"json":{"platformId":"HN1","gameId":42,"participants":[{"participantId":1,"teamId":100,"championId":103,"summonerId":"12","puuid":"a","riotIdGameName":"Player","riotIdTagline":"TEST","win":true,"doubleKills":2}]}});
        let detail = summary("TENCENT_HN1", &raw).unwrap();
        assert_eq!(detail["participants"][0]["stats"]["doubleKills"], 2);
        assert_eq!(
            account_in_game("HN1", "a", &detail).unwrap().summoner_id,
            "12"
        );
        assert!(summary("TENCENT_HN10", &raw).is_err());
        assert!(account_in_game("HN1", "someone-else", &detail).is_err());
        assert!(sgp_timeline(
            "TENCENT_HN1",
            "42",
            json!({"json":{"gameId":43,"frames":[]}})
        )
        .is_err());
    }
}
