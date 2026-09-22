use super::*;
use serde_json::{json, Value};

impl Replays {
    pub async fn seek(&self, game: &StoredReview, at: f64) -> Result<(), String> {
        let _launch = self.launch.try_lock().map_err(|_| "replay.clientBusy")?;
        let duration = game.detail["gameDuration"]
            .as_f64()
            .ok_or("game.invalidData")?;
        if !at.is_finite() || at < 0.0 || at > duration {
            return Err("replay.invalidTime".into());
        }
        let path = files::path(&self.root()?, &game.id)?
            .canonicalize()
            .map_err(|_| "replay.missing")?;
        let client = reqwest::Client::builder()
            .no_proxy()
            .danger_accept_invalid_certs(true)
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(3))
            .build()
            .map_err(|_| "replay.apiUnavailable")?;
        let state: Value = client
            .get("https://127.0.0.1:2999/replay/game")
            .send()
            .await
            .map_err(|_| "replay.apiUnavailable")?
            .error_for_status()
            .map_err(|_| "replay.apiUnavailable")?
            .json()
            .await
            .map_err(|_| "replay.apiUnavailable")?;
        let pid = state["processID"]
            .as_u64()
            .and_then(|v| u32::try_from(v).ok())
            .ok_or("replay.apiUnavailable")?;
        // The API identifies its process, not its game. Verify that process's actual ROFL argument.
        let valid = tokio::task::spawn_blocking(move || {
            let mut system = sysinfo::System::new();
            let pid = sysinfo::Pid::from_u32(pid);
            system.refresh_processes_specifics(
                sysinfo::ProcessesToUpdate::Some(&[pid]),
                true,
                sysinfo::ProcessRefreshKind::nothing().with_cmd(sysinfo::UpdateKind::Always),
            );
            system.process(pid).is_some_and(|process| {
                process.name().eq_ignore_ascii_case("League of Legends.exe")
                    && process.cmd().iter().any(|arg| {
                        let candidate = std::path::Path::new(arg);
                        candidate
                            .extension()
                            .is_some_and(|e| e.eq_ignore_ascii_case("rofl"))
                            && candidate.canonicalize().ok().as_ref() == Some(&path)
                    })
            })
        })
        .await
        .map_err(|_| "replay.seekWrongGame")?;
        if !valid {
            return Err("replay.seekWrongGame".into());
        }
        let playback: Value = client
            .get("https://127.0.0.1:2999/replay/playback")
            .send()
            .await
            .map_err(|_| "replay.apiUnavailable")?
            .error_for_status()
            .map_err(|_| "replay.apiUnavailable")?
            .json()
            .await
            .map_err(|_| "replay.apiUnavailable")?;
        if !playback["length"]
            .as_f64()
            .is_some_and(|length| length.is_finite() && length > 0.0 && at <= length)
        {
            return Err("replay.invalidTime".into());
        }
        client
            .post("https://127.0.0.1:2999/replay/playback")
            .json(&json!({"time":at}))
            .send()
            .await
            .map_err(|_| "replay.apiUnavailable")?
            .error_for_status()
            .map_err(|_| "replay.apiUnavailable")?;
        Ok(())
    }
}
