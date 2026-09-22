use super::*;
use std::{
    io::{Read, Write},
    path::Path,
};
use zip::{write::SimpleFileOptions, ZipArchive, ZipWriter};

fn error(_: impl std::fmt::Display) -> String {
    "backup.invalid".into()
}
fn text(zip: &mut ZipWriter<std::fs::File>, name: &str, data: &[u8]) -> Result<(), String> {
    zip.start_file(
        name,
        SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated),
    )
    .map_err(error)?;
    zip.write_all(data).map_err(|_| "backup.writeFailed".into())
}
fn replay(zip: &mut ZipWriter<std::fs::File>, name: &str, path: &Path) -> Result<(), String> {
    zip.start_file(
        name,
        SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
    )
    .map_err(error)?;
    std::io::copy(
        &mut std::fs::File::open(path).map_err(|_| "replay.missing")?,
        zip,
    )
    .map_err(|_| "backup.writeFailed")?;
    Ok(())
}
fn write_package(
    path: &Path,
    action: impl FnOnce(&mut ZipWriter<std::fs::File>) -> Result<(), String>,
) -> Result<(), String> {
    let temp = files::PartialFile(path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4())));
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp.0)
        .map_err(|_| "backup.writeFailed")?;
    let mut zip = ZipWriter::new(file);
    action(&mut zip)?;
    zip.finish()
        .map_err(error)?
        .sync_all()
        .map_err(|_| "backup.writeFailed")?;
    std::fs::rename(&temp.0, path).map_err(|_| "backup.writeFailed".into())
}
pub fn read_backup(path: &Path) -> Result<(crate::archive::Store, i64), String> {
    let file = std::fs::File::open(path).map_err(error)?;
    if path
        .extension()
        .is_some_and(|s| s.eq_ignore_ascii_case("jsonl"))
    {
        return crate::archive::Store::read_backup(file);
    }
    let mut zip = ZipArchive::new(file).map_err(error)?;
    read_archive(&mut zip)
}
fn read_archive(
    zip: &mut ZipArchive<std::fs::File>,
) -> Result<(crate::archive::Store, i64), String> {
    validate_entries(zip)?;
    crate::archive::Store::read_backup(zip.by_name("archive.jsonl").map_err(error)?)
}

fn validate_entries(zip: &mut ZipArchive<std::fs::File>) -> Result<(), String> {
    if zip.len() > 10001 {
        return Err("backup.tooLarge".into());
    }
    let mut names = std::collections::HashSet::new();
    let mut total = 0u64;
    let mut archives = 0;
    for index in 0..zip.len() {
        let entry = zip.by_index(index).map_err(error)?;
        if !names.insert(entry.name().to_string()) || entry.is_symlink() {
            return Err("backup.invalid".into());
        }
        if entry.name() == "archive.jsonl" {
            archives += 1;
            let limit = crate::archive::backup::stream::STREAM_LIMIT;
            if entry.size() > limit {
                return Err("backup.tooLarge".into());
            }
            continue;
        }
        let id = entry
            .name()
            .strip_prefix("replays/")
            .and_then(|v| v.strip_suffix(".rofl"))
            .ok_or("backup.invalid")?;
        files::path(Path::new("."), id)?;
        files::check_size(entry.size())?;
        total = total.checked_add(entry.size()).ok_or("backup.tooLarge")?;
        if total > 20 * 1024 * 1024 * 1024 {
            return Err("backup.tooLarge".into());
        }
    }
    if archives != 1 {
        return Err("backup.invalid".into());
    }
    Ok(())
}
impl Replays {
    pub fn export_package(
        &self,
        archive: &SharedArchive,
        path: &Path,
        id: Option<&str>,
    ) -> Result<(), String> {
        let _launch = self.launch.try_lock().map_err(|_| "backup.busy")?;
        let root = self.root()?;
        write_package(path, |zip| {
            if let Some(id) = id {
                // Snapshot only the requested match and notes under the same lock.
                // Sharing must not depend on the size or validity of unrelated games.
                let (game, notes) = {
                    let store = archive.lock().map_err(|_| "library.storageError")?;
                    (
                        store.cached_review(id, None)?.ok_or("library.notFound")?,
                        store.review_notes(id)?,
                    )
                };
                let file = files::path(&root, id)?;
                files::inspect(&file, &game)?;
                text(
                    zip,
                    "match.json",
                    &serde_json::to_vec_pretty(&game).map_err(error)?,
                )?;
                text(
                    zip,
                    "notes.json",
                    &serde_json::to_vec_pretty(&notes).map_err(error)?,
                )?;
                let markdown = format!(
                    "# {id}\n\n{} · {}\n\n{}",
                    game.account.riot_id,
                    game.server,
                    notes
                        .iter()
                        .map(|n| format!(
                            "## {}\n\n{}\n\n{}",
                            n.at.map(|t| format!("{:02}:{:02}", t as u64 / 60, t as u64 % 60))
                                .unwrap_or_else(|| "Match".into()),
                            n.body,
                            n.tags.join(", ")
                        ))
                        .collect::<Vec<_>>()
                        .join("\n\n")
                );
                text(zip, "notes.md", markdown.as_bytes())?;
                replay(zip, &format!("{id}.rofl"), &file)?;
                text(zip,"README.txt",b"LeagueReplay review package\nRead notes.md in any text editor. match.json and notes.json contain this match only.\nPlay the ROFL with a compatible League of Legends game version. No account password is included.\n")?;
            } else {
                let store = archive
                    .lock()
                    .map_err(|_| "library.storageError")?
                    .backup_snapshot()?;
                zip.start_file(
                    "archive.jsonl",
                    SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Deflated),
                )
                .map_err(error)?;
                store.write_backup(zip)?;
                let mut total = 0u64;
                let mut count = 0;
                for job in store.replay_files()? {
                    let path = files::path(&root, &job.id)?;
                    if !path.is_file() {
                        continue;
                    }
                    count += 1;
                    total += std::fs::metadata(&path).map_err(error)?.len();
                    if count > 10000 || total > 20 * 1024 * 1024 * 1024 {
                        return Err("backup.tooLarge".into());
                    }
                    let game = store
                        .cached_review(&job.id, None)?
                        .ok_or("library.notFound")?;
                    files::inspect(&path, &game)?;
                    replay(zip, &format!("replays/{}.rofl", job.id), &path)?;
                }
            }
            Ok(())
        })
    }
    // Called under with_idle and the archive lock; never overwrites an existing managed replay.
    pub fn restore_package_files(
        &self,
        path: &Path,
        expected: &crate::archive::Store,
    ) -> Result<(), String> {
        let mut zip = ZipArchive::new(std::fs::File::open(path).map_err(error)?).map_err(error)?;
        let (snapshot, _) = read_archive(&mut zip)?;
        if snapshot.backup_fingerprint()? != expected.backup_fingerprint()? {
            return Err("backup.changed".into());
        }
        let root = self.root()?;
        let mut staged = Vec::new();
        for index in 0..zip.len() {
            let mut entry = zip.by_index(index).map_err(error)?;
            if entry.name() == "archive.jsonl" {
                continue;
            }
            let id = entry
                .name()
                .strip_prefix("replays/")
                .and_then(|v| v.strip_suffix(".rofl"))
                .ok_or("backup.invalid")?;
            let destination = files::path(&root, id)?;
            let game = snapshot.cached_review(id, None)?.ok_or("backup.invalid")?;
            if destination.exists() {
                files::inspect(&destination, &game)?;
                continue;
            }
            let temp = files::PartialFile(root.join(format!("{}.restore", uuid::Uuid::new_v4())));
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp.0)
                .map_err(|_| "backup.writeFailed")?;
            let bytes = std::io::copy(&mut entry.by_ref().take(512 * 1024 * 1024 + 1), &mut file)
                .map_err(error)?;
            files::check_size(bytes)?;
            file.sync_all().map_err(|_| "backup.writeFailed")?;
            drop(file);
            files::inspect(&temp.0, &game)?;
            staged.push((temp, destination));
        }
        for (temp, destination) in staged {
            std::fs::hard_link(&temp.0, destination).map_err(|_| "backup.writeFailed")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn imported_replay_roundtrips_in_backup_and_share_contains_only_one_match() {
        let root =
            std::env::temp_dir().join(format!("league-replay-package-{}", uuid::Uuid::new_v4()));
        let service = Replays::new(root.join("managed")).unwrap();
        let archive = Arc::new(Mutex::new(
            crate::archive::Store::new(rusqlite::Connection::open_in_memory().unwrap()).unwrap(),
        ));
        let game = StoredReview {
            id: "HN1_42".into(),
            server: "TENCENT_HN1".into(),
            account: crate::domain::Account {
                id: "HN1:a".into(),
                platform: "HN1".into(),
                puuid: "a".into(),
                summoner_id: "1".into(),
                riot_id: "A#TEST".into(),
            },
            detail: json!({"gameId":42,"platformId":"HN1","gameVersion":"16.18.1","gameCreation":1000,"gameDuration":120,"participantIdentities":[{"participantId":1,"player":{"puuid":"a"}}],"participants":[{"participantId":1,"championId":103,"stats":{"win":true}}]}),
            timeline: None,
            source: "archive".into(),
            timeline_error: None,
            bookmarked: false,
        };
        archive.lock().unwrap().save_review(&game, 1000).unwrap();
        let metadata=json!({"gameId":42,"platformId":"HN1","gameLength":120000,"lastGameChunkId":2,"lastKeyFrameId":1,"statsJson":"[]"}).to_string();
        let version = b"16.18.1";
        let mut bytes = vec![0u8; 15];
        bytes[..6].copy_from_slice(b"RIOT\x02\x00");
        bytes[14] = version.len() as u8;
        bytes.extend_from_slice(version);
        bytes.extend_from_slice(&[1, 2, 3]);
        bytes.extend_from_slice(metadata.as_bytes());
        bytes.extend_from_slice(&(metadata.len() as u32).to_le_bytes());
        let external = root.join("external.rofl");
        std::fs::write(&external, &bytes).unwrap();
        service.import(&external, &game, &archive).unwrap();
        assert!(external.exists());
        assert_eq!(service.list(&archive).unwrap()[0].state, "ready");
        // The complete ZIP must round-trip an archive larger than the old JSON cap.
        {
            let mut store = archive.lock().unwrap();
            let tx = store.test_connection().transaction().unwrap();
            let timeline = json!({"frames":[], "padding":"x".repeat(1024 * 1024)}).to_string();
            for n in 1000..1065 {
                tx.execute(
                    "INSERT INTO games(id,platform,game_number,timeline) VALUES (?1,'HN1',?2,?3)",
                    rusqlite::params![format!("HN1_{n}"), n.to_string(), timeline],
                )
                .unwrap();
            }
            tx.commit().unwrap();
        }
        let backup = root.join("backup.zip");
        service.export_package(&archive, &backup, None).unwrap();
        let (raw, _) = read_backup(&backup).unwrap();
        assert_eq!(
            raw.backup_fingerprint().unwrap(),
            archive.lock().unwrap().backup_fingerprint().unwrap()
        );
        let restored = Replays::new(root.join("restored")).unwrap();
        restored
            .with_idle(|| restored.restore_package_files(&backup, &raw))
            .unwrap();
        assert_eq!(
            std::fs::read(restored.root().unwrap().join("HN1_42.rofl")).unwrap(),
            bytes
        );
        let share = root.join("share.zip");
        // A large unrelated archive must not block sharing this one match.
        archive.lock().unwrap().test_connection().execute(
            "INSERT INTO games(id,platform,game_number,timeline) VALUES ('HN1_99','HN1','99',?1)",
            [format!("\"{}\"", "x".repeat(65 * 1024 * 1024))],
        ).unwrap();
        assert_eq!(
            archive
                .lock()
                .unwrap()
                .write_backup(&mut std::io::sink())
                .unwrap_err(),
            "backup.tooLarge"
        );
        service
            .export_package(&archive, &share, Some(&game.id))
            .unwrap();
        let mut zip = ZipArchive::new(std::fs::File::open(share).unwrap()).unwrap();
        assert_eq!(zip.len(), 5);
        assert!(zip.by_name("notes.md").is_ok());
        assert!(zip.by_name("archive.json").is_err());
        drop(zip);
        drop(service);
        drop(restored);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn backup_refuses_extra_paths_before_extracting() {
        let temp = files::PartialFile(std::env::temp_dir().join(format!(
            "league-replay-invalid-{}.zip",
            uuid::Uuid::new_v4()
        )));
        write_package(&temp.0, |zip| {
            text(zip, "archive.json", b"{}")?;
            text(zip, "../outside.txt", b"bad")
        })
        .unwrap();
        assert!(read_backup(&temp.0).is_err());
    }
}
