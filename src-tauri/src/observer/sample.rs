use crate::{
    client::lcu::{parse_account, LcuError, LocalClient},
    domain::{Account, ObservedGame, Sample},
};
use serde_json::Value;

fn has_game(phase: &str) -> bool {
    matches!(
        phase,
        "InProgress" | "Reconnect" | "WaitingForStats" | "PreEndOfGame" | "EndOfGame"
    )
}

impl LocalClient {
    pub async fn sample(&self) -> Result<Sample, LcuError> {
        let before = self.get("/lol-summoner/v1/current-summoner").await?;
        let phase = self.get("/lol-gameflow/v1/gameflow-phase").await?;
        let phase = phase.as_str().ok_or(LcuError::InvalidData)?;
        let session = if has_game(phase) {
            match self.get("/lol-gameflow/v1/session").await {
                Ok(session) => Some(session),
                Err(error) if phase == "InProgress" => return Err(error),
                Err(_) => None,
            }
        } else {
            None
        };
        // Freeze only an internally consistent sample across a login switch.
        let after = self.get("/lol-summoner/v1/current-summoner").await?;
        parse_sample(&self.platform, &before, &after, phase, session.as_ref())
    }
}
pub fn parse_sample(
    platform: &str,
    before: &Value,
    after: &Value,
    phase: &str,
    session: Option<&Value>,
) -> Result<Sample, LcuError> {
    let account = parse_account(platform, before)?;
    let confirmed = parse_account(platform, after)?;
    if account.id != confirmed.id || account.summoner_id != confirmed.summoner_id {
        return Err(LcuError::IdentityChanged);
    }
    let observed_game = if phase == "InProgress" {
        Some(parse_game(&account, phase, session)?)
    } else if has_game(phase) {
        // A missing/stale post-game session must not block enrichment of known games.
        parse_game(&account, phase, session).ok()
    } else {
        None
    };
    let queue_active = matches!(
        phase,
        "Matchmaking"
            | "ReadyCheck"
            | "ChampSelect"
            | "GameStart"
            | "InProgress"
            | "Reconnect"
            | "WaitingForStats"
            | "PreEndOfGame"
    );
    Ok(Sample {
        account,
        phase: phase.into(),
        observed_game,
        queue_active,
    })
}

fn parse_game(
    account: &Account,
    phase: &str,
    session: Option<&Value>,
) -> Result<ObservedGame, LcuError> {
    let session = session.ok_or(LcuError::InvalidData)?;
    if session["phase"].as_str() != Some(phase)
        || (phase == "InProgress" && session["gameClient"]["running"].as_bool() != Some(true))
    {
        return Err(LcuError::IdentityChanged);
    }
    let data = &session["gameData"];
    let player = ["teamOne", "teamTwo"]
        .iter()
        .flat_map(|team| data[*team].as_array().into_iter().flatten())
        .find(|player| player["puuid"].as_str() == Some(account.puuid.as_str()))
        .ok_or(LcuError::InvalidData)?;
    let game_id = data["gameId"]
        .as_u64()
        .filter(|v| *v > 0)
        .ok_or(LcuError::InvalidData)?
        .to_string();
    let champion_id = player["championId"]
        .as_u64()
        .filter(|v| *v > 0 && *v <= u32::MAX as u64)
        .ok_or(LcuError::InvalidData)? as u32;
    // Explicit membership in the current local session is evidence of play.
    // An account's unrelated history is not.
    Ok(ObservedGame {
        id: format!("{}_{game_id}", account.platform),
        game_id,
        champion_id,
        queue_name: data["queue"]["name"]
            .as_str()
            .filter(|v| !v.is_empty())
            .unwrap_or("客户端对局")
            .into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn reconnect_and_post_game_recover_only_an_identified_local_participant() {
        let account = json!({"puuid":"borrowed-account","summonerId":1,"gameName":"Borrowed","tagLine":"TEST"});
        for phase in ["Reconnect", "WaitingForStats", "PreEndOfGame", "EndOfGame"] {
            let session = json!({"phase":phase,"gameClient":{"running":false},"gameData":{"gameId":91,"teamOne":[{"puuid":"borrowed-account","championId":103}],"teamTwo":[]}});
            let sample = parse_sample("HN1", &account, &account, phase, Some(&session)).unwrap();
            assert_eq!(sample.observed_game.as_ref().unwrap().id, "HN1_91");
            assert_eq!(sample.is_playing(), phase == "Reconnect");
            // Missing or stale optional evidence leaves the account connected but
            // never creates a new play record or guesses from recent match history.
            for invalid in [
                json!({}),
                json!({"phase":phase,"gameData":{"gameId":91,"teamOne":[{"puuid":"someone-else","championId":103}]}}),
                json!({"phase":"Lobby","gameData":session["gameData"]}),
                json!({"phase":phase,"gameData":{"gameId":0,"teamOne":[{"puuid":"borrowed-account","championId":103}]}}),
            ] {
                assert!(
                    parse_sample("HN1", &account, &account, phase, Some(&invalid))
                        .unwrap()
                        .observed_game
                        .is_none()
                );
            }
            assert!(parse_sample("HN1", &account, &account, phase, None)
                .unwrap()
                .observed_game
                .is_none());
            let switched =
                json!({"puuid":"other-account","summonerId":2,"gameName":"Other","tagLine":"TEST"});
            assert!(matches!(
                parse_sample("HN1", &account, &switched, phase, Some(&session)),
                Err(LcuError::IdentityChanged)
            ));
        }
    }
    #[test]
    fn only_confirmed_membership_records_play_and_rejects_switch_races() {
        let a = json!({"puuid":"player-a","summonerId":1,"gameName":"A","tagLine":"TEST"});
        let b = json!({"puuid":"player-b","summonerId":2,"gameName":"B","tagLine":"TEST"});
        let game = json!({"phase":"InProgress","gameClient":{"running":true},"gameData":{"gameId":91,"teamOne":[{"puuid":"player-a","championId":103}],"teamTwo":[],"queue":{"name":"排位"}}});
        assert_eq!(
            parse_sample("HN1", &a, &a, "InProgress", Some(&game))
                .unwrap()
                .observed_game
                .unwrap()
                .id,
            "HN1_91"
        );
        assert!(matches!(
            parse_sample("HN1", &a, &b, "InProgress", Some(&game)),
            Err(LcuError::IdentityChanged)
        ));
        assert!(parse_sample("HN1", &b, &b, "InProgress", Some(&game)).is_err());
        for phase in ["WatchInProgress", "ChampSelect", "Lobby", "GameStart"] {
            assert!(parse_sample("HN1", &a, &a, phase, Some(&game))
                .unwrap()
                .observed_game
                .is_none());
        }
    }
}
