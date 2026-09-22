use crate::domain::game::{GameParticipant, GameSummary};
use serde_json::Value;

fn number(value: &Value) -> u32 {
    value.as_u64().and_then(|n| n.try_into().ok()).unwrap_or(0)
}

pub fn project(detail: &Value) -> Result<GameSummary, String> {
    let identities = detail["participantIdentities"]
        .as_array()
        .ok_or("game.invalidData")?;
    let people = detail["participants"]
        .as_array()
        .filter(|p| !p.is_empty())
        .ok_or("game.invalidData")?;
    let mut participants = Vec::with_capacity(people.len());
    for person in people {
        let id = number(&person["participantId"]);
        if id == 0 || participants.iter().any(|p: &GameParticipant| p.id == id) {
            return Err("game.invalidData".into());
        }
        let player = identities
            .iter()
            .find(|identity| identity["participantId"] == person["participantId"])
            .map(|identity| &identity["player"]);
        let player = player.unwrap_or(&Value::Null);
        let stats = &person["stats"];
        let game_name = player["gameName"].as_str().filter(|s| !s.is_empty());
        let name = game_name
            .map(
                |name| match player["tagLine"].as_str().filter(|s| !s.is_empty()) {
                    Some(tag) => format!("{name}#{tag}"),
                    None => name.into(),
                },
            )
            .or_else(|| {
                player["summonerName"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
            });
        let subteam = number(&stats["playerSubteamId"]);
        let placement = match number(&stats["subteamPlacement"]) {
            0 => number(&stats["placement"]),
            placement => placement,
        };
        participants.push(GameParticipant {
            id,
            puuid: player["puuid"]
                .as_str()
                .filter(|s| !s.is_empty() && *s != "00000000-0000-0000-0000-000000000000")
                .map(str::to_owned),
            name,
            team: if subteam > 0 {
                subteam
            } else {
                number(&person["teamId"])
            },
            placement: (placement > 0).then_some(placement),
            champion_id: number(&person["championId"]),
            role: person["teamPosition"]
                .as_str()
                .filter(|s| !s.is_empty())
                .or_else(|| person["timeline"]["lane"].as_str())
                .unwrap_or("")
                .into(),
            win: stats["win"].as_bool(),
            kills: number(&stats["kills"]),
            deaths: number(&stats["deaths"]),
            assists: number(&stats["assists"]),
            cs: number(&stats["totalMinionsKilled"])
                .saturating_add(number(&stats["neutralMinionsKilled"])),
            gold: number(&stats["goldEarned"]),
            damage: number(&stats["totalDamageDealtToChampions"]),
            items: (0..7)
                .map(|i| number(&stats[format!("item{i}")]))
                .filter(|id| *id > 0)
                .collect(),
            double_kills: number(&stats["doubleKills"]),
            triple_kills: number(&stats["tripleKills"]),
            quadra_kills: number(&stats["quadraKills"]),
            penta_kills: number(&stats["pentaKills"]),
        });
    }
    participants.sort_by_key(|p| (p.team, p.id));
    Ok(GameSummary {
        started_at: detail["gameCreation"].as_f64().unwrap_or(0.0),
        duration: number(&detail["gameDuration"]),
        queue_id: number(&detail["queueId"]),
        version: detail["gameVersion"].as_str().unwrap_or("").into(),
        participants,
    })
}
pub fn search_result(
    page: crate::domain::review::HistoryPage,
) -> Result<crate::domain::review::SearchResult, String> {
    use crate::domain::review::{SearchGame, SearchResult};
    let games = page
        .games
        .iter()
        .map(|detail| {
            let game_id = detail["gameId"]
                .as_u64()
                .filter(|id| *id > 0)
                .ok_or("game.invalidId")?
                .to_string();
            Ok(SearchGame {
                game_id,
                game: project(detail)?,
            })
        })
        .collect::<Result<_, String>>()?;
    Ok(SearchResult {
        account: page.account,
        server: page.server,
        games,
        start: page.start,
        has_more: page.has_more,
        skipped_games: page.skipped_games,
        source: page.source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn projects_identity_stats_and_subteams_without_inventing_results() {
        let game = json!({
            "gameCreation": 1000, "gameDuration": 1800, "queueId": 1700, "gameVersion": "16.18",
            "participantIdentities": [
                {"participantId": 1, "player": {"puuid": "a", "gameName": "Alice", "tagLine": "TEST"}},
                {"participantId": 2, "player": {"puuid": "00000000-0000-0000-0000-000000000000", "summonerName": "Bot"}}
            ],
            "participants": [
                {"participantId": 1, "teamId": 100, "championId": 103, "teamPosition": "MIDDLE", "stats": {
                    "playerSubteamId": 3, "subteamPlacement": 2, "placement": 8,
                    "win": true, "kills": 12, "deaths": 3, "assists": 8,
                    "totalMinionsKilled": 100, "neutralMinionsKilled": 20,
                    "goldEarned": 14000, "totalDamageDealtToChampions": 25000,
                    "item0": 1001, "item1": 0, "item6": 3363, "tripleKills": 1
                }},
                {"participantId": 2, "teamId": 200, "timeline": {"lane": "TOP"}, "stats": {}}
            ]
        });
        let result = project(&game).unwrap();
        assert_eq!(result.started_at, 1000.0);
        assert_eq!(result.duration, 1800);
        assert_eq!(result.queue_id, 1700);
        assert_eq!(result.version, "16.18");
        let a = &result.participants[0];
        assert_eq!(a.name.as_deref(), Some("Alice#TEST"));
        assert_eq!(a.puuid.as_deref(), Some("a"));
        assert_eq!((a.team, a.placement, a.champion_id), (3, Some(2), 103));
        assert_eq!((a.kills, a.deaths, a.assists, a.cs), (12, 3, 8, 120));
        assert_eq!((a.gold, a.damage, a.triple_kills), (14000, 25000, 1));
        assert_eq!(a.items, vec![1001, 3363]);
        assert_eq!(a.role, "MIDDLE");
        assert_eq!(a.win, Some(true));
        let bot = &result.participants[1];
        assert_eq!(bot.name.as_deref(), Some("Bot"));
        assert!(bot.puuid.is_none());
        assert!(bot.win.is_none());
        assert_eq!(bot.role, "TOP");
    }

    #[test]
    fn rejects_missing_empty_or_ambiguous_rosters() {
        for value in [
            json!({}),
            json!({"participantIdentities": [], "participants": []}),
            json!({"participantIdentities": [], "participants": [{"participantId": 0}]}),
            json!({"participantIdentities": [], "participants": [{"participantId": 1}, {"participantId": 1}]}),
        ] {
            assert_eq!(project(&value).unwrap_err(), "game.invalidData");
        }
    }
}
