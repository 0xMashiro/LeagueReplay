use crate::domain::review::{EventKind, EventLabel, ReviewEvent, ReviewGame, StoredReview};
use serde_json::Value;

fn number(value: &Value) -> u32 {
    value.as_u64().and_then(|v| v.try_into().ok()).unwrap_or(0)
}

pub fn review_game(raw: StoredReview) -> Result<ReviewGame, String> {
    let game = super::project(&raw.detail)?;
    let game_id = raw.detail["gameId"]
        .as_u64()
        .filter(|id| *id > 0)
        .ok_or("game.invalidId")?
        .to_string();
    let timeline = raw
        .timeline
        .as_ref()
        .and_then(|t| t["frames"].as_array())
        .map(|frames| {
            let mut events = Vec::new();
            for (frame_index, frame) in frames.iter().enumerate() {
                let Some(rows) = frame["events"].as_array() else {
                    continue;
                };
                for (event_index, event) in rows.iter().enumerate() {
                    let (kind, label, item) = match event["type"].as_str() {
                        Some("ITEM_PURCHASED") => (EventKind::Purchase, EventLabel::Purchase, true),
                        Some("ITEM_SOLD") => (EventKind::Sale, EventLabel::Sale, true),
                        Some("ITEM_UNDO") => (
                            EventKind::Undo,
                            if number(&event["beforeId"]) == 0 {
                                EventLabel::UndoSale
                            } else if number(&event["afterId"]) == 0 {
                                EventLabel::UndoPurchase
                            } else {
                                EventLabel::Undo
                            },
                            true,
                        ),
                        Some("ITEM_DESTROYED") => (EventKind::Destroy, EventLabel::Destroy, true),
                        Some("CHAMPION_KILL") => (EventKind::Kill, EventLabel::Kill, false),
                        Some("BUILDING_KILL" | "ELITE_MONSTER_KILL") => {
                            (EventKind::Objective, EventLabel::Objective, false)
                        }
                        _ => continue,
                    };
                    let Some(timestamp) = event["timestamp"].as_u64() else {
                        continue;
                    };
                    let at = timestamp / 1000;
                    let participant_id =
                        number(&event[if item { "participantId" } else { "killerId" }]);
                    if at > u64::from(game.duration)
                        || !game.participants.iter().any(|p| p.id == participant_id)
                    {
                        continue;
                    }
                    let restored_item_id =
                        matches!(kind, EventKind::Undo).then(|| number(&event["afterId"]));
                    let item_id = item.then(|| {
                        ["itemId", "beforeId", "afterId"]
                            .iter()
                            .map(|key| number(&event[*key]))
                            .find(|id| *id > 0)
                            .unwrap_or(0)
                    });
                    events.push(ReviewEvent {
                        id: format!("{}:{frame_index}:{event_index}", raw.id),
                        at: at as u32,
                        participant_id,
                        kind,
                        label,
                        item_id,
                        restored_item_id,
                        monster: event["monsterType"].as_str().unwrap_or("").into(),
                    });
                }
            }
            events
        });
    Ok(ReviewGame {
        id: raw.id,
        game_id,
        server: raw.server,
        account: raw.account,
        game,
        timeline,
        queue_name: raw.detail["gameMode"]
            .as_str()
            .filter(|s| !s.is_empty())
            .or_else(|| raw.detail["gameType"].as_str())
            .unwrap_or("")
            .into(),
        source: raw.source,
        timeline_error: raw.timeline_error,
        bookmarked: raw.bookmarked,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture() -> StoredReview {
        StoredReview {
            id: "HN1_1".into(),
            server: "TENCENT_HN1".into(),
            account: crate::domain::Account {
                id: "HN1:a".into(),
                platform: "HN1".into(),
                puuid: "a".into(),
                summoner_id: "1".into(),
                riot_id: "A#TEST".into(),
            },
            detail: json!({"gameId":1,"gameDuration":100,"gameMode":"CLASSIC",
                "participantIdentities":[{"participantId":1,"player":{"puuid":"a"}}],
                "participants":[{"participantId":1,"stats":{"win":true,"tripleKills":1,"playerSubteamId":3,"subteamPlacement":2}}]}),
            timeline: None,
            source: "archive".into(),
            timeline_error: None,
            bookmarked: true,
        }
    }

    #[test]
    fn projects_real_events_and_undo_direction_without_fabricating_multikills() {
        let mut raw = fixture();
        raw.timeline = Some(json!({"frames":[{"events":[
            {"type":"ITEM_PURCHASED","timestamp":42001,"participantId":1,"itemId":1001},
            {"type":"ITEM_SOLD","timestamp":43000,"participantId":1,"itemId":1001},
            {"type":"ITEM_UNDO","timestamp":44000,"participantId":1,"beforeId":0,"afterId":1001},
            {"type":"ITEM_UNDO","timestamp":45000,"participantId":1,"beforeId":1001,"afterId":0},
            {"type":"ITEM_DESTROYED","timestamp":46000,"participantId":1,"itemId":1001},
            {"type":"CHAMPION_KILL","timestamp":47000,"killerId":1},
            {"type":"ELITE_MONSTER_KILL","timestamp":48000,"killerId":1,"monsterType":"DRAGON"},
            {"type":"BUILDING_KILL","timestamp":49000,"killerId":1},
            {"type":"CHAMPION_KILL","timestamp":50000,"killerId":0},
            {"type":"CHAMPION_KILL","timestamp":101000,"killerId":1},
            {"type":"CHAMPION_KILL","killerId":1},
            {"type":"CHAMPION_KILL","timestamp":-1,"killerId":1},
            {"type":"OTHER","timestamp":0,"participantId":1}
        ]}]}));
        let result = review_game(raw).unwrap();
        assert_eq!(result.game_id, "1");
        assert_eq!(result.queue_name, "CLASSIC");
        assert_eq!(result.game.participants[0].triple_kills, 1);
        assert_eq!(
            (
                result.game.participants[0].team,
                result.game.participants[0].placement
            ),
            (3, Some(2))
        );
        let wire = serde_json::to_value(result).unwrap();
        assert!(wire.get("detail").is_none());
        let events = wire["timeline"].as_array().unwrap();
        assert_eq!(events.len(), 8);
        assert_eq!(events[0]["at"], 42);
        assert_eq!(events[0]["id"], "HN1_1:0:0");
        assert_eq!(events[2]["label"], "undoSale");
        assert_eq!(events[2]["restoredItemId"], 1001);
        assert_eq!(events[3]["label"], "undoPurchase");
        assert_eq!(events[3]["restoredItemId"], 0);
        assert_eq!(events[6]["monster"], "DRAGON");
        assert_eq!(events[7]["kind"], "objective");
    }

    #[test]
    fn distinguishes_missing_timeline_from_a_valid_empty_timeline() {
        assert!(review_game(fixture()).unwrap().timeline.is_none());
        let mut raw = fixture();
        raw.timeline = Some(json!({"frames":[]}));
        assert!(review_game(raw.clone())
            .unwrap()
            .timeline
            .unwrap()
            .is_empty());
        raw.timeline = Some(json!({}));
        assert!(review_game(raw).unwrap().timeline.is_none());
    }
}
