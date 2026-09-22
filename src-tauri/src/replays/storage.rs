use super::*;
use std::io::{Read, Write};
use std::path::Path;

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temp = files::PartialFile(path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4())));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp.0)
        .map_err(|_| "replay.storageError")?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| "replay.storageError")?;
    drop(file);
    std::fs::rename(&temp.0, path).map_err(|_| "replay.storageError".into())
}
pub fn read_root(control: &Path) -> Result<PathBuf, String> {
    match std::fs::read(control.join("directory.json")) {
        Ok(data) => serde_json::from_slice(&data).map_err(|_| "replay.storageError".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(control.to_path_buf()),
        Err(_) => Err("replay.storageError".into()),
    }
}
pub fn read_jobs(control: &Path) -> Result<BTreeMap<String, ReplayJob>, String> {
    let data = match std::fs::read(control.join("queue.json")) {
        Ok(data) => data,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(_) => return Err("replay.storageError".into()),
    };
    let mut jobs: BTreeMap<String, ReplayJob> =
        serde_json::from_slice(&data).map_err(|_| "replay.storageError")?;
    for (id, job) in &mut jobs {
        files::path(control, id)?;
        if id != &job.id {
            return Err("replay.storageError".into());
        }
        if matches!(
            job.state.as_str(),
            "queued" | "downloading" | "validating" | "waiting"
        ) {
            job.state = "waiting".into();
        }
        if job.state == "cancelling" {
            job.state = "cancelled".into();
        }
    }
    Ok(jobs)
}
pub fn write_jobs(control: &Path, jobs: &BTreeMap<String, ReplayJob>) -> Result<(), String> {
    atomic_write(
        &control.join("queue.json"),
        &serde_json::to_vec(jobs).map_err(|_| "replay.storageError")?,
    )
}
fn identical_files(a: &Path, b: &Path) -> std::io::Result<bool> {
    if !std::fs::symlink_metadata(b)?.file_type().is_file() {
        return Ok(false);
    }
    let mut a = std::fs::File::open(a)?;
    let mut b = std::fs::File::open(b)?;
    let mut remaining = a.metadata()?.len();
    if remaining != b.metadata()?.len() {
        return Ok(false);
    }
    let (mut left, mut right) = ([0; 65536], [0; 65536]);
    while remaining > 0 {
        let count = remaining.min(left.len() as u64) as usize;
        a.read_exact(&mut left[..count])?;
        b.read_exact(&mut right[..count])?;
        if left[..count] != right[..count] {
            return Ok(false);
        }
        remaining -= count as u64;
    }
    Ok(true)
}
impl Replays {
    pub async fn recover(self: Arc<Self>, archive: SharedArchive) {
        loop {
            let waiting: Vec<String> = self
                .jobs
                .lock()
                .map(|jobs| {
                    jobs.values()
                        .filter(|j| j.state == "waiting")
                        .map(|j| j.id.clone())
                        .collect()
                })
                .unwrap_or_default();
            if !waiting.is_empty() && connect().await.is_ok() {
                for id in waiting {
                    let game = crate::library::saved_review_game(id.clone(), None, &archive);
                    let result = game.and_then(|game| self.start(game, archive.clone()));
                    if let Err(error) = result {
                        if error == "backup.busy" {
                            continue;
                        }
                        if let Ok(mut jobs) = self.jobs.lock() {
                            if let Some(job) = jobs.get_mut(&id) {
                                job.state = "error".into();
                                job.error = Some(error);
                            }
                            let _ = self.persist(&jobs);
                        }
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(15)).await;
        }
    }
    // Existing files are copied, never moved or silently replaced. Changing back remains possible.
    pub fn change_root(&self, destination: &Path) -> Result<String, String> {
        let _launch = self.launch.try_lock().map_err(|_| "backup.busy")?;
        let jobs = self.jobs.lock().map_err(|_| "backup.busy")?;
        if !self
            .cancellations
            .lock()
            .map_err(|_| "backup.busy")?
            .is_empty()
        {
            return Err("backup.busy".into());
        }
        std::fs::create_dir_all(destination).map_err(|_| "replay.storageError")?;
        let destination = destination
            .canonicalize()
            .map_err(|_| "replay.storageError")?;
        let mut root = self.root.lock().map_err(|_| "replay.storageError")?;
        if root.canonicalize().ok().as_ref() == Some(&destination) {
            return Ok(destination.to_string_lossy().into());
        }
        // Validate destination before copying anything. Only our named managed files are considered.
        let mut copy = Vec::new();
        for entry in std::fs::read_dir(&*root).map_err(|_| "replay.storageError")? {
            let entry = entry.map_err(|_| "replay.storageError")?;
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            let id = name
                .strip_suffix(".rofl")
                .or_else(|| name.strip_suffix(".part"))
                .or_else(|| name.strip_suffix(".resume.json"));
            if id.is_none_or(|id| files::path(&root, id).is_err())
                || !entry
                    .file_type()
                    .map_err(|_| "replay.storageError")?
                    .is_file()
            {
                continue;
            }
            let target = destination.join(name);
            if target.exists() {
                if identical_files(&path, &target).map_err(|_| "replay.storageError")? {
                    continue;
                }
                return Err("replay.destinationExists".into());
            }
            copy.push((path, target));
        }
        for (source, target) in copy {
            let temp =
                files::PartialFile(target.with_extension(format!("{}.tmp", uuid::Uuid::new_v4())));
            std::fs::copy(source, &temp.0).map_err(|_| "replay.storageError")?;
            std::fs::hard_link(&temp.0, target).map_err(|_| "replay.storageError")?;
        }
        atomic_write(
            &self.control.join("directory.json"),
            &serde_json::to_vec(&destination).map_err(|_| "replay.storageError")?,
        )?;
        *root = destination;
        drop(jobs);
        Ok(root.to_string_lossy().into())
    }
    pub fn import(
        &self,
        source: &Path,
        game: &StoredReview,
        archive: &SharedArchive,
    ) -> Result<(), String> {
        let _launch = self.launch.try_lock().map_err(|_| "replay.clientBusy")?;
        let jobs = self.jobs.lock().map_err(|_| "replay.unavailable")?;
        if self
            .cancellations
            .lock()
            .map_err(|_| "replay.unavailable")?
            .contains_key(&game.id)
        {
            return Err("backup.busy".into());
        }
        files::inspect(source, game)?;
        let destination = files::path(&self.root()?, &game.id)?;
        if destination.is_file() {
            files::inspect(&destination, game)?;
        } else {
            let temp = files::PartialFile(
                destination.with_extension(format!("{}.import", uuid::Uuid::new_v4())),
            );
            std::fs::copy(source, &temp.0).map_err(|_| "replay.storageError")?;
            files::inspect(&temp.0, game)?;
            std::fs::hard_link(&temp.0, &destination).map_err(|_| "replay.storageError")?;
        }
        drop(jobs);
        self.finish(game, archive, &destination)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restart_recovers_queue_and_copies_only_managed_files() {
        let root =
            std::env::temp_dir().join(format!("league-replay-storage-{}", uuid::Uuid::new_v4()));
        let service = Replays::new(root.clone()).unwrap();
        std::fs::write(root.join("HN1_42.part"), b"partial").unwrap();
        std::fs::write(root.join("unrelated.txt"), b"keep").unwrap();
        let job = ReplayJob {
            id: "HN1_42".into(),
            state: "downloading".into(),
            received: 7.0,
            total: Some(20.0),
            version: None,
            error: None,
        };
        service
            .persist(&BTreeMap::from([(job.id.clone(), job)]))
            .unwrap();
        drop(service);
        let service = Replays::new(root.clone()).unwrap();
        assert_eq!(service.jobs.lock().unwrap()["HN1_42"].state, "waiting");
        service.cancel("HN1_42").unwrap();
        let destination = root.join("new-location");
        service.change_root(&destination).unwrap();
        assert!(root.join("HN1_42.part").exists());
        assert_eq!(
            std::fs::read(destination.join("HN1_42.part")).unwrap(),
            b"partial"
        );
        assert!(!destination.join("unrelated.txt").exists());
        drop(service);
        let reopened = Replays::new(root.clone()).unwrap();
        assert_eq!(
            reopened.root().unwrap(),
            destination.canonicalize().unwrap()
        );
        assert_eq!(reopened.jobs.lock().unwrap()["HN1_42"].state, "cancelled");
        reopened.change_root(&root).unwrap();
        std::fs::write(destination.join("HN1_42.part"), b"different").unwrap();
        assert_eq!(
            reopened.change_root(&destination).unwrap_err(),
            "replay.destinationExists"
        );
        assert_eq!(std::fs::read(root.join("HN1_42.part")).unwrap(), b"partial");
        drop(reopened);
        std::fs::remove_dir_all(root).unwrap();
    }
}
