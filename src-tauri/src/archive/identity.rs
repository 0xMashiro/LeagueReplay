use super::*;

pub(super) fn validate(ui: &LiveUiState) -> Result<()> {
    let mut accounts = std::collections::HashSet::new();
    for saved in &ui.my_accounts {
        let account = &saved.account;
        if !accounts.insert(&account.id)
            || crate::regions::platform(&saved.server).as_ref() != Ok(&account.platform)
            || account.id != format!("{}:{}", account.platform, account.puuid)
            || account.puuid.is_empty()
            || account.puuid.len() > 128
            || !account
                .puuid
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
            || account.riot_id.trim().is_empty()
            || account.riot_id.len() > 512
            || account.riot_id.chars().any(char::is_control)
            || account.summoner_id.len() > 128
        {
            return Err("accounts.invalid".into());
        }
    }
    let mut ids = std::collections::HashSet::new();
    let players: std::collections::HashSet<_> = ui.players.iter().map(|p| &p.id).collect();
    if players.len() != ui.players.len() {
        return Err("identity.invalidLink".into());
    }
    for link in &ui.identity_links {
        if !ids.insert(&link.id)
            || !players.contains(&link.player_id)
            || link.account_id.len() > 512
            || !link.account_id.contains(':')
        {
            return Err("identity.invalidLink".into());
        }
        if let Some(game) = &link.match_id {
            if link.from.is_some() || link.to.is_some() || !game.contains('_') {
                return Err("identity.invalidLink".into());
            }
        } else if !matches!((link.from,link.to), (Some(a),Some(b)) if a.is_finite() && b.is_finite() && a >= 0.0 && a <= b)
        {
            return Err("identity.invalidLink".into());
        }
    }
    for (index, a) in ui.identity_links.iter().enumerate() {
        for b in &ui.identity_links[index + 1..] {
            if a.account_id != b.account_id {
                continue;
            }
            let overlaps = match (&a.match_id, &b.match_id) {
                (Some(x), Some(y)) => x == y,
                (None, None) => {
                    a.from.unwrap() <= b.to.unwrap() && b.from.unwrap() <= a.to.unwrap()
                }
                _ => false,
            };
            if overlaps {
                return Err("identity.overlap".into());
            }
        }
    }
    Ok(())
}
