//! Local installation discovery follows X's running-process/metadata/shallow-drive approach.
//! See third-party/LeagueReplayStudio-NOTICE.txt. No login, credentials or network are needed.
use super::files::patch;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub struct Installation {
    pub root: PathBuf,
    pub executable: PathBuf,
    pub version: String,
    pub tencent: bool,
    pub locale: String,
    running: bool,
}

fn read_text(path: &Path) -> Option<String> {
    (fs::metadata(path).ok()?.len() <= 512 * 1024)
        .then(|| fs::read_to_string(path).ok())
        .flatten()
}

fn client_root(executable: &Path) -> Option<&Path> {
    let parent = executable.parent()?;
    if parent.file_name()?.eq_ignore_ascii_case("LeagueClient") {
        parent.parent()
    } else {
        Some(parent)
    }
}

pub fn inspect(product: &Path) -> Option<Installation> {
    let root = product.join("Game").canonicalize().ok()?;
    let executable = root.join("League of Legends.exe").canonicalize().ok()?;
    if executable.parent()? != root
        || !executable.is_file()
        || !(product.join("LeagueClient.exe").is_file()
            || product.join("LeagueClient/LeagueClient.exe").is_file())
    {
        return None;
    }
    // Tencent installations currently ship code-metadata rather than compat-version-metadata.
    let code: Option<Value> =
        read_text(&root.join("code-metadata.json")).and_then(|s| serde_json::from_str(&s).ok());
    let compatibility: Option<Value> = read_text(&root.join("compat-version-metadata.json"))
        .and_then(|s| serde_json::from_str(&s).ok());
    let version = code
        .as_ref()
        .and_then(|v| v["version"].as_str())
        .or_else(|| compatibility.as_ref().and_then(|v| v["version"].as_str()))?
        .to_owned();
    patch(&version)?;
    let tencent = version.to_ascii_lowercase().contains("tencent") || product.join("TCLS").is_dir();
    if !tencent && !product.join("LeagueClient.exe").is_file() {
        return None;
    }
    let mut locales: Vec<_> = fs::read_dir(root.join("DATA/FINAL/Localized"))
        .ok()?
        .flatten()
        .take(128)
        .filter_map(|entry| {
            let name = entry.file_name();
            let name = name.to_str()?;
            let locale = name.strip_prefix("Global.")?.strip_suffix(".wad.client")?;
            (!locale.is_empty()
                && locale.len() <= 16
                && locale.bytes().all(|b| b.is_ascii_alphabetic() || b == b'_'))
            .then(|| locale.to_owned())
        })
        .collect();
    locales.sort();
    let preferred = if tencent { "zh_CN" } else { "en_US" };
    let locale = locales
        .iter()
        .find(|s| s.eq_ignore_ascii_case(preferred))
        .or(locales.first())?
        .clone();
    Some(Installation {
        root,
        executable,
        version,
        tencent,
        locale,
        running: false,
    })
}

fn consider(found: &mut BTreeMap<PathBuf, Installation>, product: &Path) {
    if let Some(install) = inspect(product) {
        found.entry(install.executable.clone()).or_insert(install);
    }
}

fn consider_container(found: &mut BTreeMap<PathBuf, Installation>, container: &Path) {
    for suffix in [
        "Riot Games/League of Legends",
        "League of Legends",
        "Games/League of Legends",
        "WeGameApps/英雄联盟",
        "WeGameApps/Games/英雄联盟",
        "Tencent/WeGameApps/英雄联盟",
        "英雄联盟",
    ] {
        consider(found, &container.join(suffix));
    }
}

pub fn discover() -> Vec<Installation> {
    let mut found = BTreeMap::new();
    let mut system = sysinfo::System::new();
    system.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::All,
        true,
        sysinfo::ProcessRefreshKind::nothing().with_exe(sysinfo::UpdateKind::Always),
    );
    for process in system.processes().values() {
        if process.name().eq_ignore_ascii_case("LeagueClient.exe")
            || process.name().eq_ignore_ascii_case("LeagueClientUx.exe")
        {
            if let Some(root) = process.exe().and_then(|p| client_root(p)) {
                if let Some(mut install) = inspect(root) {
                    install.running = true;
                    found.insert(install.executable.clone(), install);
                }
            }
        }
    }
    if let Some(data) = std::env::var_os("ProgramData") {
        let riot = PathBuf::from(data).join("Riot Games");
        if let Some(Value::Object(entries)) = read_text(&riot.join("RiotClientInstalls.json"))
            .and_then(|s| serde_json::from_str::<Value>(&s).ok())
            .and_then(|v| v.get("associated_client").cloned())
        {
            for product in entries.keys().take(64) {
                consider(&mut found, Path::new(product));
            }
        }
        if let Ok(products) = fs::read_dir(riot.join("Metadata")) {
            for product in products.flatten().take(128) {
                if !product
                    .file_name()
                    .to_string_lossy()
                    .starts_with("league_of_legends.")
                {
                    continue;
                }
                let Ok(settings) = fs::read_dir(product.path()) else {
                    continue;
                };
                for settings in settings.flatten().take(32) {
                    if !settings
                        .file_name()
                        .to_string_lossy()
                        .ends_with(".product_settings.yaml")
                    {
                        continue;
                    }
                    if let Some(contents) = read_text(&settings.path()) {
                        if let Some(value) = contents
                            .lines()
                            .find_map(|line| line.trim().strip_prefix("product_install_full_path:"))
                        {
                            consider(
                                &mut found,
                                Path::new(value.trim().trim_matches(['\'', '"'])),
                            );
                        }
                    }
                }
            }
        }
    }
    #[cfg(windows)]
    for drive in super::windows::fixed_drives() {
        consider_container(&mut found, &drive);
        if let Ok(children) = fs::read_dir(&drive) {
            for child in children.flatten().take(512) {
                if child.file_type().is_ok_and(|kind| kind.is_dir()) {
                    consider_container(&mut found, &child.path());
                }
            }
        }
    }
    found.into_values().collect()
}

pub fn select(
    installs: Vec<Installation>,
    tencent: bool,
    version: &str,
    preferred: Option<&Path>,
) -> Result<Installation, String> {
    let patch = patch(version).ok_or("replay.versionMismatch")?;
    let family: Vec<_> = installs
        .into_iter()
        .filter(|i| i.tencent == tencent)
        .collect();
    if family.is_empty() {
        return Err("replay.installMissing".into());
    }
    let mut compatible: Vec<_> = family
        .into_iter()
        .filter(|i| super::files::patch(&i.version) == Some(patch))
        .collect();
    if compatible.is_empty() {
        return Err("replay.versionMismatch".into());
    }
    if let Some(index) = preferred.and_then(|root| compatible.iter().position(|i| i.root == root)) {
        return Ok(compatible.remove(index));
    }
    if compatible.iter().filter(|i| i.running).count() == 1 {
        let index = compatible.iter().position(|i| i.running).unwrap();
        return Ok(compatible.remove(index));
    }
    if compatible.len() != 1 {
        return Err("replay.installAmbiguous".into());
    }
    Ok(compatible.remove(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn offline_discovery_rejects_incomplete_wrong_family_and_incompatible_installs() {
        let base =
            std::env::temp_dir().join(format!("league-replay-installs-{}", uuid::Uuid::new_v4()));
        let product = base.join("WeGameApps/英雄联盟");
        let game = product.join("Game");
        fs::create_dir_all(game.join("DATA/FINAL/Localized")).unwrap();
        fs::write(game.join("League of Legends.exe"), []).unwrap();
        fs::write(
            game.join("DATA/FINAL/Localized/Global.zh_CN.wad.client"),
            [],
        )
        .unwrap();
        fs::write(
            game.join("code-metadata.json"),
            r#"{"version":"16.18.8191953+branch.releases-16-18.code.publictencent"}"#,
        )
        .unwrap();
        assert!(inspect(&product).is_none());
        fs::create_dir_all(product.join("LeagueClient")).unwrap();
        fs::write(product.join("LeagueClient/LeagueClient.exe"), []).unwrap();
        let mut found = BTreeMap::new();
        consider_container(&mut found, &base);
        assert_eq!(found.len(), 1);
        let installed =
            select(found.into_values().collect(), true, "16.18.819.1953", None).unwrap();
        assert_eq!(installed.locale, "zh_CN");
        assert_eq!(installed.root, game.canonicalize().unwrap());
        assert_eq!(
            select(
                vec![inspect(&product).unwrap()],
                false,
                "16.18.819.1953",
                None
            )
            .unwrap_err(),
            "replay.installMissing"
        );
        assert_eq!(
            select(vec![inspect(&product).unwrap()], true, "16.17.1", None).unwrap_err(),
            "replay.versionMismatch"
        );
        assert_eq!(
            select(
                vec![inspect(&product).unwrap(), inspect(&product).unwrap()],
                true,
                "16.18.1",
                None
            )
            .unwrap_err(),
            "replay.installAmbiguous"
        );
        // The fixture is a unique directory created under the system temp root.
        let resolved = base.canonicalize().unwrap();
        assert!(resolved.starts_with(std::env::temp_dir().canonicalize().unwrap()));
        fs::remove_dir_all(resolved).unwrap();
    }
}
