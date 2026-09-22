use crate::{
    client::regions,
    domain::{review::StoredReview, validate_game_id},
};
use rofl_inspect::{InspectOptions, ReplayInspection};
use std::path::{Path, PathBuf};

pub fn path(root: &Path, id: &str) -> Result<PathBuf, String> {
    let (platform, game) = id.rsplit_once('_').ok_or("game.invalidId")?;
    validate_game_id(game)?;
    if platform.is_empty()
        || platform.len() > 32
        || !platform
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err("game.invalidId".into());
    }
    Ok(root.join(format!("{id}.rofl")))
}
pub fn check_size(size: u64) -> Result<(), String> {
    if size > 512 * 1024 * 1024 {
        Err("replay.tooLarge".into())
    } else {
        Ok(())
    }
}
pub fn patch(version: &str) -> Option<(u32, u32)> {
    let mut parts = version.split('.');
    Some((parts.next()?.parse().ok()?, parts.next()?.parse().ok()?))
}
pub fn inspect(path: &Path, game: &StoredReview) -> Result<ReplayInspection, String> {
    let mut file = std::fs::File::open(path).map_err(|_| "replay.missing")?;
    check_size(file.metadata().map_err(|_| "replay.storageError")?.len())?;
    let result = rofl_inspect::inspect(&mut file, &InspectOptions::default())
        .map_err(|_| "replay.invalidFile")?;
    let expected = game.detail["gameId"].as_u64().ok_or("game.invalidId")?;
    if result.legacy_payload.is_some_and(|p| p.game_id != expected) {
        return Err("game.identityMismatch".into());
    }
    if let Some(value) = result.raw_metadata.get("gameId") {
        if value.as_u64().or_else(|| value.as_str()?.parse().ok()) != Some(expected) {
            return Err("game.identityMismatch".into());
        }
    }
    if let Some(value) = result
        .raw_metadata
        .get("platformId")
        .or_else(|| result.raw_metadata.get("platform"))
    {
        let platform = value.as_str().ok_or("game.identityMismatch")?;
        let (_, sub) = regions::endpoint(&game.server, true)?;
        if ![
            game.account.platform.as_str(),
            sub.as_str(),
            game.server.as_str(),
        ]
        .iter()
        .any(|p| p.eq_ignore_ascii_case(platform))
        {
            return Err("game.identityMismatch".into());
        }
    }
    if patch(&result.game_version).is_none()
        || patch(&result.game_version) != game.detail["gameVersion"].as_str().and_then(patch)
    {
        return Err("replay.versionMismatch".into());
    }
    Ok(result)
}
pub struct PartialFile(pub PathBuf);
impl Drop for PartialFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Account;
    use serde_json::json;
    #[test]
    fn downloaded_container_must_match_requested_game_region_and_patch() {
        // Minimal ROFL2 container uses the format described by the vendored inspector's fixtures.
        let version = b"16.18.1";
        let metadata=json!({"gameId":42,"platformId":"HN1","gameLength":120000,"lastGameChunkId":2,"lastKeyFrameId":1,"statsJson":"[]"}).to_string();
        let mut bytes = vec![0u8; 15];
        bytes[..6].copy_from_slice(b"RIOT\x02\x00");
        bytes[14] = version.len() as u8;
        bytes.extend_from_slice(version);
        bytes.extend_from_slice(&[1, 2, 3]);
        bytes.extend_from_slice(metadata.as_bytes());
        bytes.extend_from_slice(&(metadata.len() as u32).to_le_bytes());
        let temp = PartialFile(
            std::env::temp_dir().join(format!("league-replay-test-{}.rofl", uuid::Uuid::new_v4())),
        );
        std::fs::write(&temp.0, &bytes).unwrap();
        let mut game = StoredReview {
            id: "HN1_42".into(),
            server: "TENCENT_HN1".into(),
            account: Account {
                id: "HN1:a".into(),
                puuid: "a".into(),
                platform: "HN1".into(),
                summoner_id: "1".into(),
                riot_id: "A#TEST".into(),
            },
            detail: json!({"gameId":42,"gameVersion":"16.18.2"}),
            timeline: None,
            source: "sgp".into(),
            timeline_error: None,
            bookmarked: false,
        };
        assert!(inspect(&temp.0, &game).is_ok());
        game.detail["gameId"] = json!(43);
        assert_eq!(
            inspect(&temp.0, &game).unwrap_err(),
            "game.identityMismatch"
        );
        game.detail["gameId"] = json!(42);
        game.server = "TENCENT_HN10".into();
        game.account.platform = "HN10".into();
        assert_eq!(
            inspect(&temp.0, &game).unwrap_err(),
            "game.identityMismatch"
        );
        game.server = "TENCENT_HN1".into();
        game.account.platform = "HN1".into();
        game.detail["gameVersion"] = json!("16.19.1");
        assert_eq!(
            inspect(&temp.0, &game).unwrap_err(),
            "replay.versionMismatch"
        );
        std::fs::write(&temp.0, &bytes[..12]).unwrap();
        assert_eq!(inspect(&temp.0, &game).unwrap_err(), "replay.invalidFile");
    }
    #[test]
    fn file_identity_cannot_escape_managed_root_and_size_is_bounded() {
        for id in ["../HN1_42", "C:\\tmp_42", "HN1_../42", "HN1_0", "HN1_042"] {
            assert!(path(Path::new("replays"), id).is_err());
        }
        assert!(path(Path::new("replays"), "TENCENT_PBE_42").is_ok());
        assert!(check_size(512 * 1024 * 1024 + 1).is_err());
        assert_ne!(patch("16.18.1"), patch("16.19.1"));
        assert_eq!(patch("invalid"), None);
    }
}
