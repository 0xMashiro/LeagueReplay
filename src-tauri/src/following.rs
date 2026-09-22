use crate::{
    archive::SharedArchive,
    domain::{following::FollowingState, now},
    library,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

pub struct Following {
    archive: SharedArchive,
    pub(crate) gate: tokio::sync::Mutex<()>,
    syncing: AtomicBool,
}
pub type SharedFollowing = Arc<Following>;

impl Following {
    pub fn new(archive: SharedArchive) -> SharedFollowing {
        Arc::new(Self {
            archive,
            gate: tokio::sync::Mutex::new(()),
            syncing: AtomicBool::new(false),
        })
    }
    pub fn state(&self) -> Result<FollowingState, String> {
        Ok(FollowingState {
            subscriptions: self
                .archive
                .lock()
                .map_err(|_| "library.storageError")?
                .subscriptions()?,
            syncing: self.syncing.load(Ordering::Relaxed),
        })
    }
    pub async fn run(self: Arc<Self>) {
        loop {
            // No request is made before a player has been explicitly followed.
            let _ = self.sync(None, false).await;
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        }
    }
    pub async fn sync(&self, id: Option<&str>, force: bool) -> Result<(), String> {
        let _guard = self.gate.try_lock().map_err(|_| "following.busy")?;
        self.syncing.store(true, Ordering::Relaxed);
        let result = self.sync_inner(id, force).await;
        self.syncing.store(false, Ordering::Relaxed);
        result
    }
    pub async fn fill_history(&self, id: &str) -> Result<u32, String> {
        let _guard = self.gate.try_lock().map_err(|_| "following.busy")?;
        let (subscription, ticket, (offset, anchor)) = {
            let store = self.archive.lock().map_err(|_| "library.storageError")?;
            let s = store
                .subscriptions()?
                .into_iter()
                .find(|s| s.id == id)
                .ok_or("following.changed")?;
            let ticket = store
                .follow_ticket(id, now(), true)?
                .ok_or("following.changed")?;
            (s, ticket, store.history_cursor(id)?)
        };
        let mut start = offset.saturating_sub(20);
        let mut count = 0;
        for step in 0..5 {
            if start > 10000 {
                return Err("following.historyLimit".into());
            }
            let mut page = library::player_history_page(
                subscription.server.clone(),
                subscription.account.puuid.clone(),
                start,
            )
            .await?;
            // Rewind one page and verify the previous boundary before advancing offset pagination.
            if step == 0
                && start > 0
                && anchor.as_ref().is_some_and(|a| {
                    !page
                        .games
                        .iter()
                        .any(|g| g["gameId"].as_u64().map(|n| n.to_string()).as_ref() == Some(a))
                })
            {
                start = 0;
                page = library::player_history_page(
                    subscription.server.clone(),
                    subscription.account.puuid.clone(),
                    0,
                )
                .await?;
            }
            let done = !page.has_more;
            count += page.games.len() as u32;
            if !self
                .archive
                .lock()
                .map_err(|_| "library.storageError")?
                .finish_history_page(id, &ticket, &page)?
            {
                return Err("following.changed".into());
            }
            if done {
                break;
            }
            start += 20;
        }
        Ok(count)
    }
    async fn sync_inner(&self, id: Option<&str>, force: bool) -> Result<(), String> {
        let subscriptions = self
            .archive
            .lock()
            .map_err(|_| "library.storageError")?
            .subscriptions()?;
        for s in subscriptions
            .into_iter()
            .filter(|s| id.is_none_or(|id| id == s.id))
        {
            let ticket = self
                .archive
                .lock()
                .map_err(|_| "library.storageError")?
                .follow_ticket(&s.id, now(), force)?;
            let Some(ticket) = ticket else { continue };
            let mut pages = Vec::new();
            let mut failure = None;
            let mut limited = false;
            for start in (0..100).step_by(20) {
                match library::player_history_page(s.server.clone(), s.account.puuid.clone(), start)
                    .await
                {
                    Ok(page) => {
                        if page.skipped_games > 0 {
                            pages.push(page);
                            break;
                        }
                        let mut known = false;
                        for game in &page.games {
                            let id = format!(
                                "{}_{}",
                                s.account.platform,
                                game["gameId"].as_u64().ok_or("game.invalidData")?
                            );
                            known |= self
                                .archive
                                .lock()
                                .map_err(|_| "library.storageError")?
                                .known_follow_game(&s.id, &id)?;
                        }
                        let done = known || !page.has_more;
                        limited = !done && start == 80;
                        pages.push(page);
                        if done {
                            break;
                        }
                    }
                    Err(error) => {
                        failure = Some(error);
                        break;
                    }
                }
            }
            let result = self
                .archive
                .lock()
                .map_err(|_| "library.storageError")?
                .finish_follow_sync(&s.id, &ticket, &pages, failure.as_deref(), limited, now());
            if let Err(error) = result {
                self.archive
                    .lock()
                    .map_err(|_| "library.storageError")?
                    .finish_follow_sync(&s.id, &ticket, &[], Some(&error), false, now())?;
            }
            if failure.as_deref().is_some_and(|e| e.starts_with("client.")) {
                break;
            }
        }
        Ok(())
    }
}
