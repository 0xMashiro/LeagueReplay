use crate::domain::Account;
use reqwest::{Client, StatusCode};
use serde_json::Value;
use std::{ffi::OsString, time::Duration};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};
use zeroize::Zeroizing;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LcuError {
    Offline,
    MultipleClients,
    Discovery,
    CommandLineUnavailable,
    AccessDenied,
    InvalidArguments,
    AppProcessUnavailable,
    Unauthorized,
    Unavailable,
    NotFound,
    InvalidData,
    IdentityChanged,
}

impl LcuError {
    pub fn public_message(self) -> (&'static str, &'static str) {
        match self {
            Self::Offline => ("offline", "等待游戏客户端"),
            Self::MultipleClients => ("ambiguous", "检测到多个游戏客户端，暂不记录"),
            Self::Discovery => ("unavailable", "客户端检测暂时失败，正在重试"),
            Self::CommandLineUnavailable => ("unavailable", "已找到客户端，但暂时无法读取连接信息"),
            Self::AccessDenied => (
                "access-denied",
                "Windows 拒绝读取客户端连接信息，请以管理员身份重新打开 LeagueReplay",
            ),
            Self::InvalidArguments => ("invalid-arguments", "客户端连接参数无法识别，暂不连接"),
            Self::AppProcessUnavailable => ("waiting", "等待游戏客户端主进程就绪"),
            Self::Unauthorized => ("unauthorized", "客户端正在登录或连接凭据已更新"),
            Self::Unavailable => ("unavailable", "客户端暂时没有响应，正在重连"),
            Self::NotFound => ("waiting", "等待客户端提供数据"),
            Self::InvalidData => ("unavailable", "客户端数据无法核对，暂不记录"),
            Self::IdentityChanged => ("switching", "账号正在切换，等待身份稳定"),
        }
    }
}

// Intentionally no Debug/Serialize: the client token never leaves the Rust transport.
pub struct LocalClient {
    port: u16,
    token: Zeroizing<String>,
    pub platform: String,
    pub region: String,
    wire_platform: String,
    pub discovery_source: &'static str,
    riot: Option<(u16, Zeroizing<String>)>,
    http: Client,
}

fn argument<'a>(args: &'a [OsString], key: &str) -> Result<Option<&'a str>, LcuError> {
    let mut values = args
        .iter()
        .filter_map(|arg| arg.to_str()?.strip_prefix(key));
    let first = values.next();
    if values.next().is_some() {
        return Err(LcuError::InvalidArguments);
    }
    Ok(first)
}

fn parse_args(args: &[OsString]) -> Result<(u16, Zeroizing<String>, String, u32), LcuError> {
    let port = argument(args, "--app-port=")?
        .and_then(|v| v.parse::<u16>().ok())
        .filter(|v| *v > 0)
        .ok_or(LcuError::InvalidArguments)?;
    let pid = argument(args, "--app-pid=")?
        .and_then(|v| v.parse::<u32>().ok())
        .filter(|v| *v > 0)
        .ok_or(LcuError::InvalidArguments)?;
    let token = argument(args, "--remoting-auth-token=")?
        .filter(|v| !v.is_empty() && v.len() <= 1024)
        .ok_or(LcuError::InvalidArguments)?;
    let platform = match (
        argument(args, "--rso-platform-id=")?,
        argument(args, "--rso_platform_id=")?,
    ) {
        (Some(_), Some(_)) => return Err(LcuError::InvalidArguments),
        (hyphen, underscore) => hyphen.or(underscore),
    };
    let platform = match platform {
        Some(value) => value.to_uppercase(),
        None => match argument(args, "--region=")?
            .unwrap_or("")
            .to_ascii_uppercase()
            .as_str()
        {
            "NA" => "NA1",
            "EUW" => "EUW1",
            "EUNE" => "EUN1",
            "BR" => "BR1",
            "JP" => "JP1",
            "KR" => "KR",
            "LA1" | "LAN" => "LA1",
            "LA2" | "LAS" => "LA2",
            "OC1" | "OCE" => "OC1",
            "TR" => "TR1",
            "RU" => "RU",
            _ => return Err(LcuError::InvalidArguments),
        }
        .into(),
    };
    if platform.is_empty()
        || platform.len() > 32
        || !platform.bytes().all(|b| b.is_ascii_alphanumeric())
    {
        return Err(LcuError::InvalidArguments);
    }
    Ok((port, Zeroizing::new(token.to_owned()), platform, pid))
}

pub fn discover() -> Result<LocalClient, LcuError> {
    let mut system = System::new();
    let refresh = ProcessRefreshKind::nothing();
    #[cfg(not(windows))]
    let refresh = refresh.with_cmd(sysinfo::UpdateKind::Always);
    system.refresh_processes_specifics(ProcessesToUpdate::All, true, refresh);
    let clients: Vec<_> = system
        .processes()
        .values()
        .filter(|process| process.name().eq_ignore_ascii_case("LeagueClientUx.exe"))
        .collect();
    let process = match clients.as_slice() {
        [] => return Err(LcuError::Offline),
        [process] => process,
        _ => return Err(LcuError::MultipleClients),
    };
    // Like Akari, use Windows' query-only API for the selected UX process.
    // sysinfo's command-line path can return empty for an otherwise readable client.
    #[cfg(windows)]
    let native = super::windows::command_line(process.pid().as_u32())?;
    #[cfg(windows)]
    let (args, discovery_source) = (&native[..], "windows-query");
    #[cfg(not(windows))]
    let (args, discovery_source) = (process.cmd(), "sysinfo");
    let (port, token, platform, pid) = parse_args(args)?;
    let region = argument(args, "--region=")?
        .unwrap_or("")
        .to_ascii_uppercase();
    let wire_platform = platform.clone();
    let platform = if region == "TENCENT" {
        super::regions::platform(&format!("TENCENT_{platform}"))
            .map_err(|_| LcuError::InvalidArguments)?
    } else {
        match platform.as_str() {
            "EUW" => "EUW1".into(),
            "JP" => "JP1".into(),
            "PBE" => "PBE1".into(),
            _ => platform,
        }
    };
    let riot = match (
        argument(args, "--riotclient-app-port=")?
            .and_then(|v| v.parse::<u16>().ok())
            .filter(|v| *v > 0),
        argument(args, "--riotclient-auth-token=")?.filter(|v| !v.is_empty() && v.len() <= 1024),
    ) {
        (Some(port), Some(token)) => Some((port, Zeroizing::new(token.to_owned()))),
        _ => None,
    };
    if !system
        .process(sysinfo::Pid::from_u32(pid))
        .is_some_and(|process| process.name().eq_ignore_ascii_case("LeagueClient.exe"))
    {
        return Err(LcuError::AppProcessUnavailable);
    }
    // LCU uses a self-signed certificate. This client is restricted to a discovered
    // loopback port, bypasses proxies and never follows redirects with credentials.
    let http = Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .danger_accept_invalid_certs(true)
        .connect_timeout(Duration::from_secs(2))
        .timeout(Duration::from_secs(4))
        .build()
        .map_err(|_| LcuError::Unavailable)?;
    Ok(LocalClient {
        port,
        token,
        platform,
        region,
        wire_platform,
        riot,
        discovery_source,
        http,
    })
}

impl LocalClient {
    pub(crate) async fn get(&self, path: &str) -> Result<Value, LcuError> {
        Self::json(
            self.http
                .get(format!("https://127.0.0.1:{}{}", self.port, path))
                .basic_auth("riot", Some(self.token.as_str())),
        )
        .await
    }

    pub(crate) async fn post(&self, path: &str, body: &Value) -> Result<Value, LcuError> {
        Self::json(
            self.http
                .post(format!("https://127.0.0.1:{}{}", self.port, path))
                .basic_auth("riot", Some(self.token.as_str()))
                .json(body),
        )
        .await
    }

    pub(crate) async fn aliases(&self, name: &str, tag: &str) -> Result<Value, LcuError> {
        let (port, token) = self.riot.as_ref().ok_or(LcuError::NotFound)?;
        Self::json(
            self.http
                .get(format!(
                    "https://127.0.0.1:{port}/player-account/aliases/v1/lookup"
                ))
                .basic_auth("riot", Some(token.as_str()))
                .query(&[("gameName", name), ("tagLine", tag)]),
        )
        .await
    }

    async fn json(request: reqwest::RequestBuilder) -> Result<Value, LcuError> {
        let mut response = request.send().await.map_err(|_| LcuError::Unavailable)?;
        match response.status() {
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => return Err(LcuError::Unauthorized),
            StatusCode::NOT_FOUND => return Err(LcuError::NotFound),
            status if !status.is_success() => return Err(LcuError::Unavailable),
            _ => (),
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| LcuError::Unavailable)? {
            if bytes.len() + chunk.len() > 8 * 1024 * 1024 {
                return Err(LcuError::InvalidData);
            }
            bytes.extend_from_slice(&chunk);
        }
        if bytes.is_empty() {
            Ok(Value::Null)
        } else {
            serde_json::from_slice(&bytes).map_err(|_| LcuError::InvalidData)
        }
    }

    pub async fn detail(&self, game_id: &str) -> Result<Value, LcuError> {
        if !game_id.bytes().all(|b| b.is_ascii_digit()) || game_id.is_empty() {
            return Err(LcuError::InvalidData);
        }
        let mut value = self
            .get(&format!("/lol-match-history/v1/games/{game_id}"))
            .await?;
        if !value["platformId"].as_str().is_some_and(|p| {
            p.eq_ignore_ascii_case(&self.wire_platform) || p.eq_ignore_ascii_case(&self.platform)
        }) {
            return Err(LcuError::InvalidData);
        }
        value["platformId"] = Value::String(self.platform.clone());
        Ok(value)
    }

    pub async fn timeline(&self, game_id: &str) -> Result<Value, LcuError> {
        if !game_id.bytes().all(|b| b.is_ascii_digit()) || game_id.is_empty() {
            return Err(LcuError::InvalidData);
        }
        self.get(&format!("/lol-match-history/v1/game-timelines/{game_id}"))
            .await
    }

    #[cfg(test)]
    pub async fn history(&self) -> Result<Value, LcuError> {
        self.get("/lol-match-history/v1/products/lol/current-summoner/matches")
            .await
    }
}

pub fn parse_account(platform: &str, value: &Value) -> Result<Account, LcuError> {
    crate::matches::account_from_profile(platform, value).map_err(|_| LcuError::InvalidData)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn riot_region_aliases_work_without_a_platform_argument() {
        for (region, expected) in [
            ("LAN", "LA1"),
            ("LAS", "LA2"),
            ("OCE", "OC1"),
            ("eune", "EUN1"),
        ] {
            let args = [
                "--app-port=5000".into(),
                "--app-pid=10".into(),
                "--remoting-auth-token=test".into(),
                OsString::from(format!("--region={region}")),
            ];
            assert_eq!(parse_args(&args).unwrap().2, expected);
        }
    }

    #[test]
    fn unsafe_or_ambiguous_connection_arguments_are_rejected() {
        let args: Vec<_> = [
            "--app-port=5000",
            "--app-pid=10",
            "--remoting-auth-token=secret",
            "--rso-platform-id=HN1",
        ]
        .into_iter()
        .map(OsString::from)
        .collect();
        assert!(parse_args(&args).is_ok());
        let mut duplicate = args.clone();
        duplicate.push("--app-port=6000".into());
        assert!(parse_args(&duplicate).is_err());
        let mut alias = args.clone();
        alias.push("--rso_platform_id=HN2".into());
        assert!(parse_args(&alias).is_err());
        let mut underscore = args.clone();
        underscore[3] = "--rso_platform_id=HN1".into();
        assert!(parse_args(&underscore).is_ok());
        let mut injected = args;
        injected[3] = "--rso-platform-id=../../evil".into();
        assert!(parse_args(&injected).is_err());
    }

    #[tokio::test]
    #[ignore = "requires a logged-in local League client; GET only, no archive writes"]
    async fn live_client_read_only_smoke() {
        let client = discover().unwrap_or_else(|error| panic!("discovery: {error:?}"));
        let sample = client.sample().await.expect("current account and phase");
        println!(
            "connected: source={}, platform={}, phase={}, recording={}",
            client.discovery_source,
            client.platform,
            sample.phase,
            sample.is_playing()
        );
        let history = client.history().await.expect("recent history");
        let games = history["games"]["games"]
            .as_array()
            .expect("history games array");
        println!("history: count={}", games.len());
        if let Some(game) = games.first() {
            let game_id = game["gameId"]
                .as_u64()
                .expect("numeric game id")
                .to_string();
            let detail = client.detail(&game_id).await.expect("match detail");
            crate::archive::validate_detail(
                &format!("{}_{game_id}", client.platform),
                &sample.account,
                &detail,
            )
            .expect("game region and current account membership");
            let timeline = crate::matches::lcu_timeline(
                &crate::client::regions::current(&client),
                &game_id,
                client.timeline(&game_id).await.expect("timeline response"),
            )
            .expect("timeline contract");
            println!(
                "detail: identity_verified=true, participants={}, timeline_frames={}",
                detail["participants"].as_array().map_or(0, Vec::len),
                timeline["frames"].as_array().unwrap().len()
            );
            let mut store =
                crate::archive::Store::new(rusqlite::Connection::open_in_memory().unwrap())
                    .unwrap();
            let game = crate::domain::review::StoredReview {
                id: format!("{}_{game_id}", client.platform),
                server: crate::client::regions::current(&client),
                account: sample.account.clone(),
                detail,
                timeline: Some(timeline),
                source: "lcu".into(),
                timeline_error: None,
                bookmarked: false,
            };
            store
                .save_review(&game, crate::domain::now())
                .expect("in-memory archive write");
            let saved = store.review(&game.id, None).unwrap().unwrap();
            let review = crate::matches::review_game(saved).expect("review projection");
            println!(
                "timeline pipeline: archived=true, review_events={}",
                review.timeline.expect("timeline available").len()
            );
        }
        let confirmed = client.sample().await.expect("identity recheck");
        assert!(
            sample.account.id == confirmed.account.id,
            "account switched during smoke test"
        );
    }
}
