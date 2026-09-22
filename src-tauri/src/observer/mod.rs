mod diagnostics;
mod sample;
#[cfg(test)]
mod tests;
use crate::{
    archive::{self, SharedArchive},
    client::{client_error, lcu},
    domain::*,
};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

pub struct Observer {
    // Sampling and enrichment may overlap; backup restore must exclude both.
    pub(crate) gate: tokio::sync::RwLock<()>,
    pub(crate) store: SharedArchive,
    pub(crate) status: Mutex<ClientStatus>,
    diagnostics: Mutex<diagnostics::Diagnostics>,
}
pub type SharedObserver = Arc<Observer>;
impl Observer {
    pub fn new(store: SharedArchive, log_path: std::path::PathBuf) -> SharedObserver {
        Arc::new(Self {
            gate: tokio::sync::RwLock::new(()),
            store,
            diagnostics: Mutex::new(diagnostics::Diagnostics::new(log_path)),
            status: Mutex::new(ClientStatus::unavailable(
                "starting",
                "正在检测游戏客户端",
                now(),
            )),
        })
    }

    pub async fn run(self: Arc<Self>) {
        let mut enrichment = tokio::task::JoinSet::new();
        loop {
            let client = self.tick().await;
            while enrichment.try_join_next().is_some() {}
            // At most one detail request chain runs in the background. JoinSet aborts
            // it if the observer is stopped; slow requests never delay the next sample.
            if enrichment.is_empty() {
                if let Some(client) = client {
                    let observer = self.clone();
                    enrichment.spawn(async move { observer.complete_pending(client).await });
                }
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }

    async fn tick(&self) -> Option<lcu::LocalClient> {
        let _guard = self.gate.read().await;
        let discovered = tokio::task::spawn_blocking(lcu::discover)
            .await
            .unwrap_or(Err(lcu::LcuError::Discovery));
        let (client, sample) = match discovered {
            Ok(client) => {
                let sample = client.sample().await;
                (Some(client), sample)
            }
            Err(error) => (None, Err(error)),
        };
        self.record_sample(&sample, now());
        sample.ok().and(client)
    }

    fn log(&self, event: &str, data: serde_json::Value) {
        if let Ok(log) = self.diagnostics.lock() {
            log.write(event, data);
        }
    }

    fn record_sample(&self, sample: &Result<Sample, lcu::LcuError>, time: i64) {
        let mut status = match &sample {
            Ok(sample) => ClientStatus {
                state: "connected".into(),
                message: if sample.is_playing() {
                    "正在记录游玩"
                } else {
                    "客户端已连接"
                }
                .into(),
                phase: Some(sample.phase.clone()),
                account: Some(sample.account.clone()),
                active_game_id: sample
                    .observed_game
                    .as_ref()
                    .filter(|_| sample.is_playing())
                    .map(|g| g.id.clone()),
                last_checked_at: time as f64,
            },
            Err(error) => {
                let (state, message) = error.public_message();
                ClientStatus::unavailable(state, message, time)
            }
        };
        let result = self
            .store
            .lock()
            .map_err(|_| "backup.busy".to_string())
            .and_then(|mut s| s.observe(sample.as_ref().ok(), time));
        if let Err(error) = result {
            status.state = "storage-error".into();
            status.message = error;
        }
        if let Ok(mut log) = self.diagnostics.lock() {
            log.sample(serde_json::json!({
                "state":status.state,"phase":status.phase,
                "platform":sample.as_ref().ok().map(|s| &s.account.platform),
                "game":sample.as_ref().ok().and_then(|s| s.observed_game.as_ref()).map(|g| &g.id),
                "error":sample.as_ref().err().map(|e| client_error(*e)),
                "storage_error":if status.state == "storage-error" { Some(&status.message) } else { None }
            }));
        }
        if let Ok(mut current) = self.status.lock() {
            *current = status;
        }
    }

    async fn complete_pending(&self, client: lcu::LocalClient) {
        let _guard = self.gate.read().await;
        let pending = self
            .store
            .lock()
            .map_err(|_| "backup.busy".to_string())
            .and_then(|s| s.pending(&client.platform, now()));
        let pending = match pending {
            Ok(pending) => pending,
            Err(error) => {
                self.log("pending_failed", serde_json::json!({"error":error}));
                return;
            }
        };
        let Some((id, number, account)) = pending else {
            return;
        };
        self.fetch_game(
            &id,
            &account,
            async { client.detail(&number).await.map_err(client_error) },
            async {
                client
                    .timeline(&number)
                    .await
                    .map_err(client_error)
                    .and_then(|v| {
                        crate::matches::lcu_timeline(
                            &crate::client::regions::current(&client),
                            &number,
                            v,
                        )
                    })
            },
        )
        .await;
    }

    async fn fetch_game(
        &self,
        id: &str,
        account: &Account,
        detail: impl std::future::Future<Output = Result<serde_json::Value, String>>,
        timeline: impl std::future::Future<Output = Result<serde_json::Value, String>>,
    ) {
        self.log("fetch_started", serde_json::json!({"game":id}));
        let result = async {
            let detail = detail.await?;
            // Validate before requesting optional timeline; late results use the frozen account.
            archive::validate_detail(id, account, &detail)?;
            let timeline = timeline.await;
            self.store
                .lock()
                .map_err(|_| "backup.busy".to_string())?
                .complete(
                    id,
                    account,
                    &detail,
                    timeline.as_ref().map_err(String::as_str),
                    now(),
                )?;
            self.log("fetch_saved", serde_json::json!({"game":id,"timeline_received":timeline.is_ok(),"error":timeline.err()}));
            Ok::<(), String>(())
        }
        .await;
        if let Err(error) = result {
            self.log(
                "fetch_failed",
                serde_json::json!({"game":id,"error":error,"retry_after_ms":60_000}),
            );
            if let Ok(mut s) = self.store.lock() {
                if let Err(error) = s.failed(id, now(), &error) {
                    self.log(
                        "retry_save_failed",
                        serde_json::json!({"game":id,"error":error}),
                    );
                }
            }
        }
    }
}
