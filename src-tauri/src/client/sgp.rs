use super::regions;
use crate::client::lcu::LocalClient;
use reqwest::{Client, RequestBuilder, Response, StatusCode};
use serde_json::Value;
use std::time::Duration;
use zeroize::Zeroizing;

pub struct Gateway {
    http: Client,
    server: String,
    entitlements: Zeroizing<String>,
}

impl Gateway {
    pub async fn connect(client: &LocalClient, server: &str) -> Result<Self, String> {
        let source = regions::current(client);
        regions::check_history_access(&source, server)?;
        let entitlements = client
            .get("/entitlements/v1/token")
            .await
            .map_err(super::client_error)?;
        let token = |value: &Value| {
            value
                .as_str()
                .filter(|s| !s.is_empty())
                .map(|s| Zeroizing::new(s.to_owned()))
                .ok_or_else(|| "gateway.tokenNotReady".to_owned())
        };
        Ok(Self {
            http: Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(8))
                .timeout(Duration::from_secs(30))
                .build()
                .map_err(|_| "gateway.network")?,
            server: server.into(),
            entitlements: token(&entitlements["accessToken"])?,
        })
    }

    fn request(&self, path: &str) -> Result<RequestBuilder, String> {
        let (base, sub) = regions::endpoint(&self.server, true)?;
        let url = format!("{base}{}", path.replace("{region}", &sub));
        Ok(self.http.get(url).bearer_auth(self.entitlements.as_str()))
    }

    pub async fn history(&self, puuid: &str, start: u32) -> Result<Value, String> {
        // UUID-like opaque identity has already been validated; URL path segments use encoding.
        let encoded: String = url_segment(puuid);
        read_json(
            self.request(&format!(
                "/match-history-query/v1/products/lol/player/{encoded}/SUMMARY"
            ))?
            .query(&[("startIndex", start), ("count", 20)]),
        )
        .await
    }

    pub async fn game(&self, game_id: &str, timeline: bool) -> Result<Value, String> {
        crate::domain::validate_game_id(game_id)?;
        read_json(self.request(&format!(
            "/match-history-query/v1/products/lol/{{region}}_{game_id}/{}",
            if timeline { "DETAILS" } else { "SUMMARY" }
        ))?)
        .await
    }

    pub async fn replay_range(
        &self,
        game_id: &str,
        range: Option<(u64, &str)>,
    ) -> Result<Response, String> {
        crate::domain::validate_game_id(game_id)?;
        let mut request = self
            .request(&format!(
                "/match-history-query/v3/product/lol/matchId/{{region}}_{game_id}/infoType/replay"
            ))?
            .timeout(Duration::from_secs(300));
        if let Some((offset, validator)) = range {
            request = request
                .header("Range", format!("bytes={offset}-"))
                .header("If-Range", validator);
        }
        let response = request.send().await.map_err(|_| "gateway.network")?;
        match response.status() {
            StatusCode::UNAUTHORIZED => Err("gateway.unauthorized".into()),
            StatusCode::FORBIDDEN => Err("gateway.forbidden".into()),
            StatusCode::NOT_FOUND => Err("gateway.notFound".into()),
            StatusCode::TOO_MANY_REQUESTS => Err("gateway.rateLimited".into()),
            StatusCode::RANGE_NOT_SATISFIABLE => Ok(response),
            status if status.is_success() => Ok(response),
            _ => Err("gateway.unavailable".into()),
        }
    }
}

pub fn url_segment(value: &str) -> String {
    value
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

async fn send(request: RequestBuilder) -> Result<Response, String> {
    let response = request.send().await.map_err(|_| "gateway.network")?;
    match response.status() {
        StatusCode::UNAUTHORIZED => Err("gateway.unauthorized".into()),
        StatusCode::FORBIDDEN => Err("gateway.forbidden".into()),
        StatusCode::NOT_FOUND => Err("gateway.dataNotFound".into()),
        StatusCode::TOO_MANY_REQUESTS => Err("gateway.rateLimited".into()),
        status if status.is_success() => Ok(response),
        _ => Err("gateway.unavailable".into()),
    }
}

async fn read_json(request: RequestBuilder) -> Result<Value, String> {
    let mut response = send(request).await?;
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| "gateway.network")? {
        if bytes.len() + chunk.len() > 16 * 1024 * 1024 {
            return Err("gateway.invalidData".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| "gateway.invalidData".into())
}
