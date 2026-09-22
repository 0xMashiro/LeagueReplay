use super::{files, install};
use crate::{
    client::{
        client_error, connect,
        lcu::{LcuError, LocalClient},
        regions,
    },
    domain::review::StoredReview,
};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

pub async fn open(game: &StoredReview, path: &Path, version: &str) -> Result<(), String> {
    ensure_game_closed().await?;
    let client = match connect().await {
        Ok(client) => Some(client),
        Err(error) if error == "client.Offline" => None,
        Err(error) => return Err(error),
    };
    let mut preferred = None;
    if let Some(client) = &client {
        let config = idle_client(client).await?;
        if regions::current(client) == game.server
            && config
                .as_ref()
                .is_some_and(|v| v["isLoggedIn"] == true && v["isReplaysEnabled"] == true)
        {
            let config = config.as_ref().unwrap();
            // Keep the official playback route when available, especially for Riot/Vanguard.
            if !game.server.starts_with("TENCENT_")
                && files::patch(version) != config["gameVersion"].as_str().and_then(files::patch)
            {
                return Err("replay.versionMismatch".into());
            }
            let before = client.sample().await.map_err(client_error)?;
            let game_id = game.detail["gameId"].as_u64().ok_or("game.invalidId")?;
            let metadata = client
                .get(&format!("/lol-replays/v1/metadata/{game_id}"))
                .await
                .ok();
            let after = client.sample().await.map_err(client_error)?;
            if before.account.id != after.account.id {
                return Err("client.IdentityChanged".into());
            }
            idle_client(client).await?;
            ensure_game_closed().await?;
            if metadata.as_ref().is_some_and(|v| v["state"] == "watch")
                && files::patch(version) == config["gameVersion"].as_str().and_then(files::patch)
            {
                client
                    .post(
                        &format!("/lol-replays/v1/rofls/{game_id}/watch"),
                        &json!({"componentType":"replay-button_match-history"}),
                    )
                    .await
                    .map_err(client_error)?;
                return Ok(());
            }
            if !game.server.starts_with("TENCENT_") {
                client.post(&format!("/lol-replays/v2/metadata/{game_id}/create"),
                    &json!({"gameVersion":game.detail["gameVersion"],"gameType":game.detail["gameType"],"queueId":game.detail["queueId"],"gameEnd":game.detail["gameCreation"].as_u64().unwrap_or(0)+game.detail["gameDuration"].as_u64().unwrap_or(0)*1000})).await.map_err(client_error)?;
                client
                    .post(
                        &format!("/lol-replays/v1/rofls/{game_id}/download"),
                        &json!({"componentType":"replay-button_match-history"}),
                    )
                    .await
                    .map_err(client_error)?;
                return Err("replay.clientPreparing".into());
            }
        }
        // Installation location is usable even when the client is at the sign-in screen.
        if let Ok(location) = client
            .get("/lol-patch/v1/products/league_of_legends/install-location")
            .await
        {
            preferred = location["gameInstallRoot"]
                .as_str()
                .and_then(|v| PathBuf::from(v).canonicalize().ok());
        }
    }
    let server = game.server.clone();
    let replay = path.to_path_buf();
    let version = version.to_owned();
    let mut command = tokio::task::spawn_blocking(move || {
        prepare_direct(&server, &replay, &version, preferred.as_deref())
    })
    .await
    .map_err(|_| "replay.installMissing")??;
    if let Some(client) = &client {
        idle_client(client).await?;
    }
    ensure_game_closed().await?;
    command.spawn().map_err(|_| "replay.openFailed")?;
    Ok(())
}

async fn idle_client(client: &LocalClient) -> Result<Option<Value>, String> {
    let phase = match client.get("/lol-gameflow/v1/gameflow-phase").await {
        Ok(phase) => Some(phase),
        Err(LcuError::Unauthorized | LcuError::NotFound) => None,
        Err(error) => return Err(client_error(error)),
    };
    let config = match client.get("/lol-replays/v1/configuration").await {
        Ok(config) => Some(config),
        Err(LcuError::Unauthorized | LcuError::NotFound) => None,
        Err(error) => return Err(client_error(error)),
    };
    check_idle(phase.as_ref(), config.as_ref())?;
    Ok(config)
}

fn check_idle(phase: Option<&Value>, config: Option<&Value>) -> Result<(), String> {
    if phase.is_some_and(|v| !matches!(v.as_str(), Some("None" | "Lobby" | "EndOfGame")))
        || config.is_some_and(|v| {
            ["isPatching", "isPlayingGame", "isPlayingReplay"]
                .iter()
                .any(|key| v[*key] == true)
        })
    {
        return Err("replay.clientBusy".into());
    }
    if phase.is_none() && config.is_some_and(|v| v["isLoggedIn"] == true) {
        return Err("replay.clientNotReady".into());
    }
    Ok(())
}

// No LCU, account or token dependency. Also used by the read-only live preflight.
pub(super) fn prepare_direct(
    server: &str,
    replay: &Path,
    version: &str,
    preferred: Option<&Path>,
) -> Result<Command, String> {
    let tencent = server.starts_with("TENCENT_");
    regions::platform(server)?;
    let mut candidates = install::discover();
    if let Some(product) = preferred.and_then(Path::parent).and_then(install::inspect) {
        if !candidates
            .iter()
            .any(|i| i.executable == product.executable)
        {
            candidates.push(product);
        }
    }
    let selected = install::select(candidates, tencent, version, preferred)?;
    #[cfg(windows)]
    if !tencent && super::windows::vanguard_active()? {
        return Err("replay.vanguardActive".into());
    }
    #[cfg(not(windows))]
    return Err("replay.unsupportedOs".into());
    #[cfg(windows)]
    launch_command(&selected, server, replay)
}

fn launch_command(
    install: &install::Installation,
    server: &str,
    replay: &Path,
) -> Result<Command, String> {
    let platform = regions::platform(server)?
        .trim_start_matches("TENCENT_")
        .to_owned();
    // Routing follows X's platform registry, independently of the signed-in account.
    let region = if server.starts_with("TENCENT_") {
        "TENCENT"
    } else {
        match platform.as_str() {
            "BR1" => "BR",
            "EUW1" => "EUW",
            "JP1" => "JP",
            "KR" => "KR",
            "LA1" | "LA2" => "LA",
            "NA1" => "NA",
            "OC1" => "OC",
            "TR1" => "TR",
            "RU" => "RU",
            "SG2" => "SG",
            "TW2" => "TW",
            "VN2" => "VN",
            "PH2" => "PH",
            "TH2" => "TH",
            "PBE1" => "PBE",
            _ => return Err("region.unsupported".into()),
        }
    };
    let base = install
        .root
        .parent()
        .ok_or("replay.installMissing")?
        .to_string_lossy();
    let base = if let Some(unc) = base.strip_prefix(r"\\?\UNC\") {
        format!("//{}", unc.replace('\\', "/"))
    } else {
        base.strip_prefix(r"\\?\")
            .unwrap_or(&base)
            .replace('\\', "/")
    };
    let replay = replay.canonicalize().map_err(|_| "replay.missing")?;
    let mut command = Command::new(&install.executable);
    command
        .current_dir(&install.root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .arg(replay)
        .args([
            format!("-GameBaseDir={base}"),
            format!("-Region={region}"),
            format!("-PlatformID={platform}"),
            format!("-Locale={}", install.locale),
            "-SkipBuild".into(),
        ]);
    Ok(command)
}

async fn ensure_game_closed() -> Result<(), String> {
    // Direct Tencent playback does not set LCU's isPlayingReplay flag.
    let running = tokio::task::spawn_blocking(|| {
        let mut system = sysinfo::System::new();
        system.refresh_processes_specifics(
            sysinfo::ProcessesToUpdate::All,
            true,
            sysinfo::ProcessRefreshKind::nothing(),
        );
        system
            .processes()
            .values()
            .any(|p| p.name().eq_ignore_ascii_case("League of Legends.exe"))
    })
    .await
    .map_err(|_| "replay.clientNotReady")?;
    if running {
        Err("replay.clientBusy".into())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signed_out_client_is_allowed_but_queue_patch_and_unknown_active_state_are_not() {
        let signed_out = json!({"isLoggedIn":false,"isReplaysEnabled":false,"isPatching":false});
        assert!(check_idle(None, None).is_ok());
        assert!(check_idle(Some(&json!("None")), Some(&signed_out)).is_ok());
        for phase in [
            "Matchmaking",
            "ReadyCheck",
            "ChampSelect",
            "InProgress",
            "Reconnect",
            "WatchInProgress",
            "Unknown",
        ] {
            assert!(check_idle(Some(&json!(phase)), Some(&signed_out)).is_err());
        }
        assert!(check_idle(None, Some(&json!({"isPatching":true}))).is_err());
        assert!(check_idle(None, Some(&json!({"isLoggedIn":true}))).is_err());
    }
}
