use super::*;

impl Store {
    pub fn merge_snapshot(&self, incoming: &Store) -> Result<Store> {
        let mut scratch = self.backup_snapshot()?;
        let tx = scratch.0.transaction().map_err(sql)?;
        let mut subscriptions = BTreeMap::<String, String>::new();
        for table in TABLES.iter().copied().filter(|t| *t != "user_state") {
            stream::rows(&incoming.0, table, |mut row| {
                if table == "sessions" && row[4].is_null() {
                    row[4] = row[3].clone();
                    row[5] = "restored".into();
                }
                if table == "participations" && row[8] == "playing" {
                    row[8] = "pending".into();
                }
                if table == "subscriptions" {
                    let id = row[0].as_str().ok_or("backup.invalid")?.to_string();
                    let account = row[1].as_str().ok_or("backup.invalid")?;
                    let existing: Option<String> = tx
                        .query_row(
                            "SELECT id FROM subscriptions WHERE account_id=?1",
                            [account],
                            |r| r.get(0),
                        )
                        .optional()
                        .map_err(sql)?;
                    if let Some(existing) = existing {
                        subscriptions.insert(id, existing);
                        return Ok(());
                    }
                    row[5] = super::super::id().into();
                    row[7] = 0.into();
                }
                if table == "followed_games" {
                    if let Some(id) = row[0].as_str().and_then(|id| subscriptions.get(id)) {
                        row[0] = id.clone().into();
                    }
                }
                let values: Vec<rusqlite::types::Value> = row
                    .iter()
                    .map(|v| match v {
                        Value::Null => Ok(rusqlite::types::Value::Null),
                        Value::String(v) => Ok(rusqlite::types::Value::Text(v.clone())),
                        Value::Number(v) => v
                            .as_i64()
                            .map(rusqlite::types::Value::Integer)
                            .ok_or("backup.invalid"),
                        _ => Err("backup.invalid"),
                    })
                    .collect::<std::result::Result<_, _>>()?;
                let placeholders = vec!["?"; values.len()].join(",");
                tx.execute(
                    &format!("INSERT OR IGNORE INTO {table} VALUES({placeholders})"),
                    rusqlite::params_from_iter(&values),
                )
                .map_err(sql)?;
                if table == "games" {
                    tx.execute("UPDATE games SET detail=COALESCE(detail,?2),timeline=COALESCE(timeline,?3) WHERE id=?1",params![values[0],values[3],values[4]]).map_err(sql)?;
                }
                Ok(())
            })?;
        }
        let current_ui: String = self
            .0
            .query_row("SELECT data FROM user_state WHERE id=1", [], |r| r.get(0))
            .map_err(sql)?;
        let imported_ui: String = incoming
            .0
            .query_row("SELECT data FROM user_state WHERE id=1", [], |r| r.get(0))
            .map_err(sql)?;
        let mut ui: LiveUiState = serde_json::from_str(&current_ui).map_err(invalid)?;
        let imported: LiveUiState = serde_json::from_str(&imported_ui).map_err(invalid)?;
        for saved in imported.my_accounts {
            if !ui
                .my_accounts
                .iter()
                .any(|s| s.account.id == saved.account.id)
            {
                ui.my_accounts.push(saved);
            }
        }
        for mut note in imported.notes {
            if ui.notes.iter().any(|n| {
                n.match_id == note.match_id
                    && n.body == note.body
                    && n.at == note.at
                    && n.participant_id == note.participant_id
                    && n.tags == note.tags
            }) {
                continue;
            }
            if ui.notes.iter().any(|n| n.id == note.id) {
                note.id = super::super::id();
            }
            ui.notes.push(note);
        }
        for reviewed in imported.reviewed {
            if !ui.reviewed.contains(&reviewed) {
                ui.reviewed.push(reviewed);
            }
        }
        for mark in imported.premades {
            if !ui
                .premades
                .iter()
                .any(|m| m.match_id == mark.match_id && m.account_id == mark.account_id)
            {
                ui.premades.push(mark);
            }
        }
        for mut player in imported.players {
            if ui.players.iter().any(|p| p.id == player.id) {
                continue;
            }
            player
                .account_ids
                .retain(|id| !ui.players.iter().any(|p| p.account_ids.contains(id)));
            ui.players.push(player);
        }
        for link in imported.identity_links {
            if ui.identity_links.iter().any(|l| l.id == link.id) {
                continue;
            }
            ui.identity_links.push(link);
            if super::super::identity::validate(&ui).is_err() {
                ui.identity_links.pop();
            }
        }
        tx.execute(
            "UPDATE user_state SET data=?1 WHERE id=1",
            [serde_json::to_string(&ui).map_err(invalid)?],
        )
        .map_err(sql)?;
        tx.commit().map_err(sql)?;
        validate_store(&mut scratch)?;
        Ok(scratch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn merge_keeps_both_note_versions_and_is_idempotent() {
        let mut a = Store::new(Connection::open_in_memory().unwrap()).unwrap();
        let mut b = Store::new(Connection::open_in_memory().unwrap()).unwrap();
        let mut ui = LiveUiState::default();
        ui.my_accounts.push(crate::domain::SavedAccount {
            server: "TENCENT_HN1".into(),
            account: crate::archive::tests::account("a"),
        });
        ui.notes.push(crate::domain::LiveNote {
            id: "note".into(),
            match_id: "HN1_42".into(),
            body: "first".into(),
            at: None,
            participant_id: None,
            tags: vec![],
            updated_at: "2026-09-21T00:00:00Z".into(),
        });
        a.save_ui(&ui, 0).unwrap();
        ui.notes[0].body = "second".into();
        ui.my_accounts[0].account.riot_id = "old-name#TEST".into();
        ui.my_accounts.push(crate::domain::SavedAccount {
            server: "TENCENT_HN1".into(),
            account: crate::archive::tests::account("b"),
        });
        b.save_ui(&ui, 0).unwrap();
        let merged = a.merge_snapshot(&b).unwrap();
        let twice = merged.merge_snapshot(&b).unwrap();
        let raw: String = twice
            .0
            .query_row("SELECT data FROM user_state WHERE id=1", [], |r| r.get(0))
            .unwrap();
        let ui: LiveUiState = serde_json::from_str(&raw).unwrap();
        assert_eq!(ui.notes.len(), 2);
        assert_eq!(ui.my_accounts.len(), 2);
        assert_eq!(ui.my_accounts[0].account.riot_id, "a#TEST");
    }
}
