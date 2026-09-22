use crate::ipc::CommandError;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use tauri::Manager;
async fn fetch(client: &reqwest::Client, path: &str) -> Result<Value, String> {
    let mut response = client
        .get(format!("https://ddragon.leagueoflegends.com/{path}"))
        .send()
        .await
        .map_err(|_| "gateway.network")?
        .error_for_status()
        .map_err(|_| "gateway.network")?;
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| "gateway.network")? {
        if bytes.len() + chunk.len() > 16 * 1024 * 1024 {
            return Err("game.invalidData".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| "game.invalidData".into())
}
#[tauri::command]
pub fn cached_resources(app: tauri::AppHandle) -> Result<Option<ResourceData>, CommandError> {
    let path = app
        .path()
        .app_data_dir()
        .map_err(|_| "library.storageError")?
        .join("resources.json");
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|_| "game.invalidData".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err("library.storageError".into()),
    }
}
#[tauri::command]
pub async fn update_resources(app: tauri::AppHandle) -> Result<ResourceData, CommandError> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|_| "gateway.network")?;
    let versions = fetch(&client, "api/versions.json").await?;
    let version = versions[0]
        .as_str()
        .filter(|v| {
            v.len() < 32
                && v.split('.').count() == 3
                && v.bytes().all(|b| b.is_ascii_digit() || b == b'.')
        })
        .ok_or("game.invalidData")?;
    let mut catalog = Value::Null;
    let mut names = BTreeMap::new();
    for (language, locale) in [
        ("zh-CN", "zh_CN"),
        ("en", "en_US"),
        ("ja", "ja_JP"),
        ("ko", "ko_KR"),
    ] {
        let champions_path = format!("cdn/{version}/data/{locale}/champion.json");
        let items_path = format!("cdn/{version}/data/{locale}/item.json");
        let (champions, items) =
            tokio::try_join!(fetch(&client, &champions_path), fetch(&client, &items_path))?;
        let champions = champions["data"].as_object().ok_or("game.invalidData")?;
        let items = items["data"].as_object().ok_or("game.invalidData")?;
        if champions.is_empty() || items.is_empty() {
            return Err("game.invalidData".into());
        }
        if language == "zh-CN" {
            let champions: BTreeMap<_,_> = champions.iter().map(|(id,v)| (id,json!({"key":v["key"].as_str().and_then(|v|v.parse::<u32>().ok()),"name":v["name"],"title":v["name"],"bundled":false}))).collect();
            let items: BTreeMap<_,_> = items.iter().map(|(id,v)| (id,json!({"name":v["name"],"gold":v["gold"]["total"],"finished":v["into"].as_array().is_none_or(|a|a.is_empty()) && v["gold"]["total"].as_u64().unwrap_or(0)>=1500,"bundled":false}))).collect();
            catalog = json!({"version":version,"champions":champions,"items":items});
        } else {
            let champions: BTreeMap<_, _> =
                champions.iter().map(|(id, v)| (id, &v["name"])).collect();
            let items: BTreeMap<_, _> = items.iter().map(|(id, v)| (id, &v["name"])).collect();
            names.insert(language, json!({"champions":champions,"items":items}));
        }
    }
    let result: ResourceData = serde_json::from_value(json!({"catalog":catalog,"names":names}))
        .map_err(|_| "game.invalidData")?;
    let path = app
        .path()
        .app_data_dir()
        .map_err(|_| "library.storageError")?
        .join("resources.json");
    crate::replays::atomic_write(
        &path,
        &serde_json::to_vec(&result).map_err(|_| "game.invalidData")?,
    )?;
    Ok(result)
}
use crate::domain::resources::ResourceData;
