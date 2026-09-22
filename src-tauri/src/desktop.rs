use crate::ipc::CommandError;
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};
use tauri_plugin_autostart::ManagerExt;

#[derive(Clone, Serialize, Deserialize, ts_rs::TS)]
#[serde(default, rename_all = "camelCase")]
#[ts(export, export_to = "../../src/generated/desktop/")]
pub struct Preferences {
    pub close_to_tray: bool,
    pub language: String,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            close_to_tray: true,
            language: "zh-CN".into(),
        }
    }
}
pub struct Desktop {
    path: PathBuf,
    preferences: Mutex<Preferences>,
    ready: AtomicBool,
}
#[derive(Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/generated/desktop/")]
pub struct Settings {
    preferences: Preferences,
    autostart: bool,
}

pub fn show(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
fn menu(app: &tauri::AppHandle, language: &str) -> tauri::Result<Menu<tauri::Wry>> {
    let (open, quit) = match language {
        "en" => ("Open LeagueReplay", "Quit LeagueReplay"),
        "ja" => ("LeagueReplay を開く", "LeagueReplay を終了"),
        "ko" => ("LeagueReplay 열기", "LeagueReplay 종료"),
        _ => ("打开 LeagueReplay", "退出 LeagueReplay"),
    };
    Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, "open", open, true, None::<&str>)?,
            &MenuItem::with_id(app, "quit", quit, true, None::<&str>)?,
        ],
    )
}
pub fn setup(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let path = app.path().app_data_dir()?.join("desktop.json");
    let preferences = match std::fs::read(&path) {
        Ok(raw) => serde_json::from_slice(&raw)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Preferences::default(),
        Err(error) => return Err(error.into()),
    };
    // Small, opaque brand mark remains visible on both Windows taskbar themes.
    let mut rgba = vec![0u8; 32 * 32 * 4];
    for y in 4..28 {
        for x in 5..27 {
            if x < 12 || y > 21 || (x > 17 && y < 19 && x - 17 < (19 - y).min(y - 3)) {
                rgba[(y * 32 + x) * 4..(y * 32 + x) * 4 + 4].copy_from_slice(&[201, 172, 112, 255]);
            }
        }
    }
    TrayIconBuilder::with_id("main")
        .icon(tauri::image::Image::new_owned(rgba, 32, 32))
        .tooltip("LeagueReplay")
        .menu(&menu(app, &preferences.language)?)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show(app),
            "quit" => {
                if !app.state::<Desktop>().ready.load(Ordering::Relaxed) {
                    app.exit(0);
                } else {
                    show(app);
                    let _ = app.emit("desktop-request-exit", ());
                }
            }
            _ => (),
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                show(tray.app_handle());
            }
        })
        .build(app)?;
    app.manage(Desktop {
        path,
        preferences: Mutex::new(preferences),
        ready: AtomicBool::new(false),
    });
    let fallback = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(15)).await;
        if !fallback.state::<Desktop>().ready.load(Ordering::Relaxed) {
            show(&fallback);
        }
    });
    Ok(())
}
pub fn close(window: &tauri::Window, api: &tauri::CloseRequestApi) {
    api.prevent_close();
    let hide = window
        .state::<Desktop>()
        .preferences
        .lock()
        .map(|p| p.close_to_tray)
        .unwrap_or(false);
    if hide {
        let _ = window.hide();
    } else {
        let _ = window.emit("desktop-request-exit", ());
    }
}
#[tauri::command]
pub fn desktop_settings(app: tauri::AppHandle) -> Result<Settings, CommandError> {
    Ok(Settings {
        preferences: app
            .state::<Desktop>()
            .preferences
            .lock()
            .map_err(|_| "desktop.settingsError")?
            .clone(),
        autostart: app
            .autolaunch()
            .is_enabled()
            .map_err(|_| "desktop.settingsError")?,
    })
}
#[tauri::command]
pub fn save_desktop_settings(
    app: tauri::AppHandle,
    preferences: Preferences,
) -> Result<(), CommandError> {
    if !["zh-CN", "en", "ja", "ko"].contains(&preferences.language.as_str()) {
        return Err("desktop.settingsError".into());
    }
    let desktop = app.state::<Desktop>();
    let mut current = desktop
        .preferences
        .lock()
        .map_err(|_| "desktop.settingsError")?;
    let temporary = desktop.path.with_extension("tmp");
    use std::io::Write;
    let mut file = std::fs::File::create(&temporary).map_err(|_| "desktop.settingsError")?;
    file.write_all(&serde_json::to_vec(&preferences).map_err(|_| "desktop.settingsError")?)
        .map_err(|_| "desktop.settingsError")?;
    file.sync_all().map_err(|_| "desktop.settingsError")?;
    drop(file);
    std::fs::rename(temporary, &desktop.path).map_err(|_| "desktop.settingsError")?;
    if let Some(tray) = app.tray_by_id("main") {
        tray.set_menu(Some(
            menu(&app, &preferences.language).map_err(|_| "desktop.settingsError")?,
        ))
        .map_err(|_| "desktop.settingsError")?;
    }
    *current = preferences;
    Ok(())
}
#[tauri::command]
pub fn set_autostart(app: tauri::AppHandle, enabled: bool) -> Result<(), CommandError> {
    let manager = app.autolaunch();
    if enabled {
        manager.enable()
    } else {
        manager.disable()
    }
    .map_err(|_| "desktop.settingsError".into())
}
#[tauri::command]
pub fn desktop_ready(app: tauri::AppHandle) {
    app.state::<Desktop>().ready.store(true, Ordering::Relaxed);
    if !std::env::args().any(|arg| arg == "--autostart") {
        show(&app);
    }
}
#[tauri::command]
pub fn exit_desktop(app: tauri::AppHandle) {
    app.exit(0);
}
