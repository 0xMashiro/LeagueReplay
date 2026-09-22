use super::*;
use rusqlite::functions::FunctionFlags;
use std::collections::{HashMap, HashSet};

// Functions are scoped to this connection and replaced while its archive mutex is held.
// Capturing annotations once avoids parsing the entire user state for every match.
pub(super) fn configure(db: &Connection, query: &PlayQuery) -> Result<()> {
    let raw: String = db
        .query_row("SELECT data FROM user_state WHERE id=1", [], |r| r.get(0))
        .map_err(sql)?;
    let ui: LiveUiState = serde_json::from_str(&raw).map_err(|_| "ui.unreadable")?;
    let mut notes: HashMap<String, Vec<String>> = HashMap::new();
    for note in &ui.notes {
        notes
            .entry(note.match_id.clone())
            .or_default()
            .push(format!("{} {}", note.body, note.tags.join(" ")));
    }
    let text = query.text.trim().to_lowercase();
    let names = query.champion_names.clone();
    let flags = FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC;
    db.create_scalar_function("play_text_match", 5, flags, move |ctx| {
        if text.is_empty() {
            return Ok(true);
        }
        let game: String = ctx.get(0)?;
        let champion: u32 = ctx.get(4)?;
        let fields = [
            game.clone(),
            ctx.get::<String>(1)?,
            ctx.get::<String>(2)?,
            ctx.get::<String>(3)?,
            names
                .get(&champion)
                .cloned()
                .unwrap_or_else(|| champion.to_string()),
            notes.get(&game).map(|v| v.join(" ")).unwrap_or_default(),
        ];
        Ok(fields.join(" ").to_lowercase().contains(&text))
    })
    .map_err(sql)?;
    let selected = query.player.clone();
    let only_premade = query.only_premade;
    let premades: HashSet<_> = ui
        .premades
        .iter()
        .map(|m| (m.match_id.clone(), m.account_id.clone()))
        .collect();
    db.create_scalar_function("play_player_match",5,flags,move |ctx| {
        let Some(selected) = &selected else { return Ok(true); };
        let detail: Option<String> = ctx.get(0)?;
        let Some(detail) = detail else { return Ok(false); };
        let platform: String = ctx.get(1)?;
        let own: String = ctx.get(2)?;
        let game_id: String = ctx.get(3)?;
        let started: f64 = ctx.get(4)?;
        let value: Value = serde_json::from_str(&detail).map_err(|e| rusqlite::Error::UserFunctionError(Box::new(e)))?;
        let Ok(game) = crate::matches::project(&value) else { return Ok(false); };
        let Some(me) = game.participants.iter().find(|p|p.puuid.as_deref()==Some(&own)) else { return Ok(false); };
        if me.team == 0 { return Ok(false); }
        Ok(game.participants.iter().any(|p| {
            let Some(puuid) = &p.puuid else { return false; };
            if p.id == me.id || p.team != me.team { return false; }
            let account = format!("{platform}:{puuid}");
            let links: Vec<_> = ui.identity_links.iter().filter(|l|l.account_id==account).collect();
            let link = links.iter().find(|l|l.match_id.as_deref()==Some(&game_id))
                .or_else(||links.iter().find(|l|l.match_id.is_none() && matches!((l.from,l.to),(Some(from),Some(to)) if from<=started && started<=to)));
            let player = match link {
                Some(link) => ui.players.iter().find(|p|p.id==link.player_id),
                None => ui.players.iter().find(|p|p.account_ids.contains(&account)),
            };
            player.is_some_and(|p|&p.id==selected) && (!only_premade || premades.contains(&(game_id.clone(),account)))
        }))
    }).map_err(sql)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture() -> Store {
        let mut store = Store::new(Connection::open_in_memory().unwrap()).unwrap();
        for n in 1..=25 {
            let mut detail = crate::archive::tests::detail("a", n);
            detail["gameCreation"] = json!(n * 1000);
            detail["participants"][0]["teamId"] = json!(100);
            detail["participantIdentities"]
                .as_array_mut()
                .unwrap()
                .extend([
                    json!({"participantId":2,"player":{"puuid":"b"}}),
                    json!({"participantId":3,"player":{"puuid":"c"}}),
                ]);
            detail["participants"].as_array_mut().unwrap().extend([
                json!({"participantId":2,"teamId":100,"stats":{}}),
                json!({"participantId":3,"teamId":200,"stats":{}}),
            ]);
            store
                .import_many(
                    &[crate::domain::review::StoredReview {
                        id: format!("HN1_{n}"),
                        server: "TENCENT_HN1".into(),
                        account: crate::archive::tests::account("a"),
                        detail,
                        timeline: None,
                        source: "archive".into(),
                        timeline_error: None,
                        bookmarked: false,
                    }],
                    None,
                    &format!("场次 {n}"),
                )
                .unwrap();
        }
        let ui = json!({"notes":[{"id":"n","matchId":"HN1_1","body":"ÉTUDE 中文笔记","tags":["练习"],"updatedAt":"2026-09-22"}],"reviewed":[],"theme":null,
            "players":[{"id":"friend","name":"Friend","accountIds":["HN1:b"]},{"id":"other","name":"Other","accountIds":[]},{"id":"enemy","name":"Enemy","accountIds":["HN1:c"]}],
            "identityLinks":[{"id":"range","accountId":"HN1:b","playerId":"other","matchId":null,"from":1000,"to":2000},{"id":"override","accountId":"HN1:b","playerId":"friend","matchId":"HN1_1","from":null,"to":null}],
            "premades":[{"matchId":"HN1_1","accountId":"HN1:b"},{"matchId":"HN1_2","accountId":"HN1:b"},{"matchId":"HN1_1","accountId":"HN1:c"}]});
        store
            .save_ui(&serde_json::from_value(ui).unwrap(), 0)
            .unwrap();
        store
    }

    #[test]
    fn keyword_search_is_unicode_aware_and_applies_before_page_selection() {
        let store = fixture();
        for text in ["  étude  ", "中文笔记", "练习", "场次 1 "] {
            let page = store
                .play_page(&PlayQuery {
                    text: text.into(),
                    ..PlayQuery::default()
                })
                .unwrap();
            if text != "场次 1 " {
                assert_eq!(page.total_sessions, 1, "{text}");
                assert_eq!(page.matches[0].id, "HN1_1");
            } else {
                assert!(page.matches.iter().any(|m| m.id == "HN1_1"));
            }
        }
        let query = PlayQuery {
            text: "阿狸".into(),
            champion_names: std::collections::BTreeMap::from([(103, "阿狸".into())]),
            ..PlayQuery::default()
        };
        assert_eq!(store.play_page(&query).unwrap().total_sessions, 25);
        assert_eq!(
            store
                .play_page(&PlayQuery {
                    text: "Ahri".into(),
                    ..query
                })
                .unwrap()
                .total_sessions,
            0
        );
        // Replacing the function for a new query must not retain the old text or names.
        assert_eq!(
            store
                .play_page(&PlayQuery::default())
                .unwrap()
                .total_sessions,
            25
        );
    }

    #[test]
    fn player_filter_honors_match_overrides_ranges_opponents_and_premade_marks() {
        let store = fixture();
        let query = PlayQuery {
            player: Some("friend".into()),
            ..PlayQuery::default()
        };
        assert_eq!(store.play_page(&query).unwrap().total_sessions, 24);
        let marked = store
            .play_page(&PlayQuery {
                only_premade: true,
                ..query
            })
            .unwrap();
        assert_eq!(marked.matches.len(), 1);
        assert_eq!(marked.matches[0].id, "HN1_1");
        let ranged = store
            .play_page(&PlayQuery {
                player: Some("other".into()),
                only_premade: true,
                ..PlayQuery::default()
            })
            .unwrap();
        assert_eq!(ranged.matches.len(), 1);
        assert_eq!(ranged.matches[0].id, "HN1_2");
        let opponent = store
            .play_page(&PlayQuery {
                player: Some("enemy".into()),
                only_premade: true,
                ..PlayQuery::default()
            })
            .unwrap();
        assert!(opponent.matches.is_empty());
        assert_eq!(opponent.summary.games, 25);
    }
}
