mod files;
mod storage;
pub(crate) use storage::atomic_write;
mod install;
#[cfg(test)]
mod live_tests;
pub(crate) mod package;
mod playback;
mod resume;
mod seek;
#[cfg(windows)]
mod windows;
pub use crate::domain::replay::ReplayJob;
use crate::{
    archive::SharedArchive,
    client::{connect, sgp::Gateway},
    domain::review::StoredReview,
};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tokio::{io::AsyncWriteExt, sync::Semaphore};

pub struct Replays {
    root: Mutex<PathBuf>,
    control: PathBuf,
    jobs: Mutex<BTreeMap<String, ReplayJob>>,
    cancellations: Mutex<BTreeMap<String, tokio::sync::watch::Sender<bool>>>,
    slots: Semaphore,
    launch: tokio::sync::Mutex<()>,
}
pub type SharedReplays = Arc<Replays>;
async fn interruptible<T>(
    cancelled: &mut tokio::sync::watch::Receiver<bool>,
    future: impl std::future::Future<Output = Result<T, String>>,
) -> Result<T, String> {
    if *cancelled.borrow() {
        return Err("replay.cancelled".into());
    }
    tokio::select! { biased; _ = cancelled.changed() => Err("replay.cancelled".into()), result = future => result }
}

impl Replays {
    pub fn with_idle<T>(&self, action: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
        let _launch = self.launch.try_lock().map_err(|_| "backup.busy")?;
        let mut jobs = self.jobs.lock().map_err(|_| "backup.busy")?;
        if !self
            .cancellations
            .lock()
            .map_err(|_| "backup.busy")?
            .is_empty()
        {
            return Err("backup.busy".into());
        }
        let result = action()?;
        jobs.clear();
        self.persist(&jobs)?;
        Ok(result)
    }
    pub fn new(root: PathBuf) -> Result<SharedReplays, String> {
        std::fs::create_dir_all(&root).map_err(|_| "replay.storageError")?;
        let control = root;
        let root = storage::read_root(&control)?;
        std::fs::create_dir_all(&root).map_err(|_| "replay.storageError")?;
        let jobs = storage::read_jobs(&control)?;
        Ok(Arc::new(Self {
            root: Mutex::new(root),
            control,
            jobs: Mutex::new(jobs),
            cancellations: Mutex::new(BTreeMap::new()),
            slots: Semaphore::new(2),
            launch: tokio::sync::Mutex::new(()),
        }))
    }
    pub fn root(&self) -> Result<PathBuf, String> {
        self.root
            .lock()
            .map(|p| p.clone())
            .map_err(|_| "replay.storageError".into())
    }
    fn persist(&self, jobs: &BTreeMap<String, ReplayJob>) -> Result<(), String> {
        storage::write_jobs(&self.control, jobs)
    }

    pub fn list(&self, archive: &SharedArchive) -> Result<Vec<ReplayJob>, String> {
        let root = self.root()?;
        // Release the database lock before touching job state; completion takes these in reverse.
        let mut saved = archive
            .lock()
            .map_err(|_| "library.storageError")?
            .replay_files()?;
        for job in &mut saved {
            if !files::path(&root, &job.id)?.is_file() {
                job.state = "missing".into();
            }
        }
        let jobs = self.jobs.lock().map_err(|_| "replay.unavailable")?;
        saved.retain(|job| !jobs.contains_key(&job.id));
        saved.extend(jobs.values().cloned().map(|mut job| {
            if job.state == "ready" && !files::path(&root, &job.id).is_ok_and(|p| p.is_file()) {
                job.state = "missing".into();
            }
            job
        }));
        Ok(saved)
    }

    pub fn start(
        self: &Arc<Self>,
        game: StoredReview,
        archive: SharedArchive,
    ) -> Result<(), String> {
        let _launch = self.launch.try_lock().map_err(|_| "backup.busy")?;
        files::path(&self.root()?, &game.id)?;
        let mut jobs = self.jobs.lock().map_err(|_| "replay.unavailable")?;
        if self
            .cancellations
            .lock()
            .map_err(|_| "replay.unavailable")?
            .contains_key(&game.id)
        {
            return Ok(());
        }
        if jobs.get(&game.id).is_some_and(|j| {
            matches!(
                j.state.as_str(),
                "queued" | "downloading" | "validating" | "cancelling"
            )
        }) {
            return Ok(());
        }
        let previous = jobs.insert(
            game.id.clone(),
            ReplayJob {
                id: game.id.clone(),
                state: "queued".into(),
                received: 0.0,
                total: None,
                version: None,
                error: None,
            },
        );
        if let Err(error) = self.persist(&jobs) {
            if let Some(previous) = previous {
                jobs.insert(game.id.clone(), previous);
            } else {
                jobs.remove(&game.id);
            }
            return Err(error);
        }
        let (cancel, mut cancelled) = tokio::sync::watch::channel(false);
        self.cancellations
            .lock()
            .map_err(|_| "replay.unavailable")?
            .insert(game.id.clone(), cancel);
        let service = self.clone();
        tokio::spawn(async move {
            let temp = service
                .root()
                .and_then(|root| files::path(&root, &game.id))
                .map(|p| p.with_extension("part"));
            let result = match temp {
                Ok(temp) => {
                    service
                        .download(&game, &archive, &temp, &mut cancelled)
                        .await
                }
                Err(error) => Err(error),
            };
            if let Ok(mut jobs) = service.jobs.lock() {
                if let Err(error) = result {
                    if let Some(job) = jobs.get_mut(&game.id) {
                        job.state = if error == "replay.cancelled" {
                            "cancelled"
                        } else {
                            "error"
                        }
                        .into();
                        job.error = if error == "replay.cancelled" {
                            None
                        } else {
                            Some(error)
                        };
                    }
                }
                if service.persist(&jobs).is_err() {
                    if let Some(job) = jobs.get_mut(&game.id) {
                        job.error = Some("replay.storageError".into());
                    }
                }
                if let Ok(mut cancellations) = service.cancellations.lock() {
                    cancellations.remove(&game.id);
                }
            }
        });
        Ok(())
    }

    fn progress(
        &self,
        id: &str,
        state: &str,
        received: u64,
        total: Option<u64>,
    ) -> Result<(), String> {
        let mut jobs = self.jobs.lock().map_err(|_| "replay.unavailable")?;
        let job = jobs.get_mut(id).ok_or("replay.unavailable")?;
        if job.state == "cancelling" {
            return Err("replay.cancelled".into());
        }
        job.state = state.into();
        job.received = received as f64;
        job.total = total.map(|v| v as f64);
        Ok(())
    }

    pub fn cancel(&self, id: &str) -> Result<(), String> {
        let mut jobs = self.jobs.lock().map_err(|_| "replay.unavailable")?;
        let job = jobs.get_mut(id).ok_or("replay.missing")?;
        if job.state == "waiting" {
            job.state = "cancelled".into();
            return self.persist(&jobs);
        }
        if !matches!(job.state.as_str(), "queued" | "downloading" | "cancelling") {
            return Err("replay.notCancellable".into());
        }
        self.cancellations
            .lock()
            .map_err(|_| "replay.unavailable")?
            .get(id)
            .ok_or("replay.notCancellable")?
            .send(true)
            .map_err(|_| "replay.notCancellable")?;
        job.state = "cancelling".into();
        self.persist(&jobs)
    }

    pub fn remove(&self, id: &str, archive: &SharedArchive) -> Result<(), String> {
        let _launch = self.launch.try_lock().map_err(|_| "replay.clientBusy")?;
        let mut jobs = self.jobs.lock().map_err(|_| "replay.unavailable")?;
        if self
            .cancellations
            .lock()
            .map_err(|_| "replay.unavailable")?
            .contains_key(id)
        {
            return Err("replay.notCancellable".into());
        }
        if jobs.get(id).is_some_and(|j| {
            matches!(
                j.state.as_str(),
                "queued" | "downloading" | "validating" | "cancelling"
            )
        }) {
            return Err("replay.notCancellable".into());
        }
        let path = files::path(&self.root()?, id)?;
        match std::fs::remove_file(&path) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(_) => return Err("replay.storageError".into()),
        }
        archive
            .lock()
            .map_err(|_| "library.storageError")?
            .forget_replay(id)?;
        jobs.remove(id);
        let _ = std::fs::remove_file(path.with_extension("part"));
        let _ = std::fs::remove_file(path.with_extension("resume.json"));
        self.persist(&jobs)
    }

    async fn download(
        &self,
        game: &StoredReview,
        archive: &SharedArchive,
        temp: &std::path::Path,
        cancelled: &mut tokio::sync::watch::Receiver<bool>,
    ) -> Result<(), String> {
        let _permit = interruptible(cancelled, async {
            self.slots
                .acquire()
                .await
                .map_err(|_| "replay.unavailable".into())
        })
        .await?;
        let destination = files::path(&self.root()?, &game.id)?;
        // A previous process may have committed the file before its database transaction.
        if destination.is_file() {
            self.progress(&game.id, "validating", 0, None)?;
            if files::inspect(&destination, game).is_ok() {
                return self.finish(game, archive, &destination);
            }
            // Retain a damaged local file for recovery instead of making retries fail forever.
            std::fs::rename(
                &destination,
                self.root()?
                    .join(format!("{}.invalid-{}", game.id, uuid::Uuid::new_v4())),
            )
            .map_err(|_| "replay.storageError")?;
            self.progress(&game.id, "queued", 0, None)?;
        }
        let client = interruptible(cancelled, connect()).await?;
        let gateway = interruptible(cancelled, Gateway::connect(&client, &game.server)).await?;
        let game_id = game.detail["gameId"]
            .as_u64()
            .ok_or("game.invalidId")?
            .to_string();
        let meta = destination.with_extension("resume.json");
        let resume = resume::load(temp, &meta);
        let mut response = interruptible(
            cancelled,
            gateway.replay_range(
                &game_id,
                resume.as_ref().map(|r| (r.offset, r.validator.as_str())),
            ),
        )
        .await?;
        // 416 can follow a completed partial or server rotation: retry as a full request.
        if response.status() == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
            response = interruptible(cancelled, gateway.replay_range(&game_id, None)).await?;
        }
        let (mut received, total) = resume::response(&response, resume.as_ref())?;
        files::check_size(total.unwrap_or(0))?;
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .append(received > 0)
            .truncate(received == 0)
            .open(temp)
            .await
            .map_err(|_| "replay.storageError")?;
        // Commit truncation before replacing the validator: a crash must never pair an old
        // partial file with a new server object and then append bytes from that object.
        if received == 0 {
            file.sync_all().await.map_err(|_| "replay.storageError")?;
        }
        resume::save_validator(&response, &meta)?;
        self.progress(&game.id, "downloading", received, total)?;
        while let Some(chunk) = interruptible(cancelled, async {
            response.chunk().await.map_err(|_| "gateway.network".into())
        })
        .await?
        {
            received = received
                .checked_add(chunk.len() as u64)
                .ok_or("replay.tooLarge")?;
            files::check_size(received)?;
            file.write_all(&chunk)
                .await
                .map_err(|_| "replay.storageError")?;
            file.flush().await.map_err(|_| "replay.storageError")?;
            self.progress(&game.id, "downloading", received, total)?;
        }
        file.sync_all().await.map_err(|_| "replay.storageError")?;
        drop(file);
        if total.is_some_and(|size| size != received) {
            return Err("replay.incomplete".into());
        }
        self.progress(&game.id, "validating", received, total)?;
        files::inspect(temp, game)?;
        // Downloads are unique per game in this process. Never replace an existing replay.
        std::fs::hard_link(temp, &destination).map_err(|_| "replay.storageError")?;
        self.finish(game, archive, &destination)?;
        let _ = std::fs::remove_file(temp);
        let _ = std::fs::remove_file(meta);
        Ok(())
    }

    fn finish(
        &self,
        game: &StoredReview,
        archive: &SharedArchive,
        path: &std::path::Path,
    ) -> Result<(), String> {
        let inspected = files::inspect(path, game)?;
        archive
            .lock()
            .map_err(|_| "library.storageError")?
            .save_replay(
                &game.id,
                inspected.layout.file_size,
                &inspected.game_version,
            )?;
        let mut jobs = self.jobs.lock().map_err(|_| "replay.unavailable")?;
        jobs.insert(
            game.id.clone(),
            ReplayJob {
                id: game.id.clone(),
                state: "ready".into(),
                received: inspected.layout.file_size as f64,
                total: Some(inspected.layout.file_size as f64),
                version: Some(inspected.game_version),
                error: None,
            },
        );
        self.persist(&jobs)
    }

    pub async fn open(&self, game: &StoredReview) -> Result<(), String> {
        let _launch = self.launch.try_lock().map_err(|_| "replay.clientBusy")?;
        let path = files::path(&self.root()?, &game.id)?;
        let inspected = files::inspect(&path, game)?;
        playback::open(game, &path, &inspected.game_version).await
    }

    pub fn reveal(&self, id: &str) -> Result<(), String> {
        let path = files::path(&self.root()?, id)?;
        if !path.is_file() {
            return Err("replay.missing".into());
        }
        #[cfg(windows)]
        {
            std::process::Command::new("explorer.exe")
                .arg(format!("/select,{}", path.display()))
                .spawn()
                .map_err(|_| "replay.openFailed")?;
            Ok(())
        }
        #[cfg(not(windows))]
        {
            Err("replay.unsupportedOs".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn queued_tasks_cancel_without_network_and_can_be_retried() {
        let root =
            std::env::temp_dir().join(format!("league-replay-jobs-{}", uuid::Uuid::new_v4()));
        let service = Replays::new(root.clone()).unwrap();
        let _slots = service.slots.acquire_many(2).await.unwrap();
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
                riot_id: "A#TAG".into(),
            },
            detail: serde_json::json!({}),
            timeline: None,
            source: "lcu".into(),
            timeline_error: None,
            bookmarked: false,
        };
        // Failure to persist a queue must not leave an unstartable phantom job.
        std::fs::create_dir(root.join("queue.json")).unwrap();
        assert!(service.start(game.clone(), archive.clone()).is_err());
        assert!(service.jobs.lock().unwrap().is_empty());
        std::fs::remove_dir(root.join("queue.json")).unwrap();
        let operation = service.launch.lock().await;
        assert_eq!(
            service.start(game.clone(), archive.clone()).unwrap_err(),
            "backup.busy"
        );
        drop(operation);
        for _ in 0..2 {
            service.start(game.clone(), archive.clone()).unwrap();
            assert_eq!(service.list(&archive).unwrap()[0].state, "queued");
            assert!(service.with_idle(|| Ok(())).is_err());
            assert!(service.remove(&game.id, &archive).is_err());
            service.cancel(&game.id).unwrap();
            tokio::time::timeout(std::time::Duration::from_secs(2), async {
                while service.list(&archive).unwrap()[0].state != "cancelled" {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            assert!(service.cancellations.lock().unwrap().is_empty());
        }
        assert!(service.cancel(&game.id).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn removing_managed_replay_keeps_match_data_and_never_accepts_external_paths() {
        let root =
            std::env::temp_dir().join(format!("league-replay-files-{}", uuid::Uuid::new_v4()));
        let service = Replays::new(root.clone()).unwrap();
        let mut store =
            crate::archive::Store::new(rusqlite::Connection::open_in_memory().unwrap()).unwrap();
        store
            .test_connection()
            .execute(
                "INSERT INTO games(id,platform,game_number) VALUES ('HN1_42','HN1','42')",
                [],
            )
            .unwrap();
        store.save_replay("HN1_42", 4, "16.18.1").unwrap();
        let archive = Arc::new(Mutex::new(store));
        std::fs::write(root.join("HN1_42.rofl"), b"test").unwrap();
        assert!(service.remove("../elsewhere_42", &archive).is_err());
        assert!(root.join("HN1_42.rofl").exists());
        service.remove("HN1_42", &archive).unwrap();
        assert!(service.list(&archive).unwrap().is_empty());
        assert_eq!(
            archive
                .lock()
                .unwrap()
                .test_connection()
                .query_row("SELECT COUNT(*) FROM games", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );
        service.remove("HN1_42", &archive).unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    #[ignore = "uses the official HN1/HN10 gateway; requires LEAGUEREPLAY_TEST_OUTPUT; does not launch the game"]
    async fn live_replay_download_smoke() {
        let output =
            std::env::var_os("LEAGUEREPLAY_TEST_OUTPUT").expect("explicit output directory");
        let current = crate::library::search_current_player().await.unwrap();
        assert!(
            ["TENCENT_HN1", "TENCENT_HN10"].contains(&current.server.as_str()),
            "live test is authorized for HN1/HN10 only"
        );
        let first = current.games.first().expect("recent game");
        let id = first["gameId"].as_u64().unwrap().to_string();
        let game = crate::library::fetch_game(&current.server, &id, &current.account.puuid)
            .await
            .unwrap();
        let archive = Arc::new(Mutex::new(
            crate::archive::Store::new(rusqlite::Connection::open_in_memory().unwrap()).unwrap(),
        ));
        archive
            .lock()
            .unwrap()
            .save_review(&game, crate::domain::now())
            .unwrap();
        let service = Replays::new(PathBuf::from(output)).unwrap();
        service.start(game.clone(), archive.clone()).unwrap();
        for _ in 0..180 {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            let job = service
                .list(&archive)
                .unwrap()
                .into_iter()
                .find(|j| j.id == game.id)
                .unwrap();
            if job.state == "ready" {
                println!(
                    "replay: validated=true, bytes={}, version={}",
                    job.received,
                    job.version.unwrap()
                );
                return;
            }
            if job.state == "error" {
                panic!("replay download: {}", job.error.unwrap());
            }
        }
        panic!("replay download timed out");
    }
}
