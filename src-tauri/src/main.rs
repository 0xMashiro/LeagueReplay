#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use rusqlite::Connection;
use std::sync::Mutex;
use tauri::Manager;
mod archive;
mod client;
mod desktop;
mod domain;
mod following;
mod ipc;
mod library;
mod matches;
mod observer;
mod regions;
mod replays;
mod resources;

fn main() {
    if std::env::args().any(|arg| arg == "--diagnose-client") {
        let runtime = tokio::runtime::Runtime::new().expect("无法初始化运行时");
        let status = runtime.block_on(async {
            match client::lcu::discover() {
                Ok(client) => match client.sample().await {
                    Ok(sample) => serde_json::json!({"state":"connected","source":client.discovery_source,"platform":sample.account.platform,"phase":sample.phase,"recording":sample.is_playing()}),
                    Err(error) => {
                        let (state, message) = error.public_message();
                        serde_json::json!({"state":state,"message":message,"stage":"sample","code":format!("{error:?}"),"source":client.discovery_source})
                    }
                },
                Err(error) => {
                    let (state, message) = error.public_message();
                    serde_json::json!({"state":state,"message":message,"stage":"discovery","code":format!("{error:?}")})
                }
            }
        });
        println!("{status}");
        return;
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            desktop::show(app)
        }))
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .app_name("LeagueReplay")
                .arg("--autostart")
                .build(),
        )
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                desktop::close(window, api);
            }
        })
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            desktop::setup(app.handle())?;
            let store = archive::Store::new(Connection::open(dir.join("archive.sqlite"))?)
                .map_err(std::io::Error::other)?;
            let archive = std::sync::Arc::new(Mutex::new(store));
            app.manage(archive.clone());
            let replays =
                replays::Replays::new(dir.join("replays")).map_err(std::io::Error::other)?;
            app.manage(replays.clone());
            tauri::async_runtime::spawn(replays.recover(archive.clone()));
            let following = following::Following::new(archive.clone());
            app.manage(following.clone());
            tauri::async_runtime::spawn(following.run());
            let observer = observer::Observer::new(archive, dir.join("collection.jsonl"));
            app.manage(observer.clone());
            tauri::async_runtime::spawn(observer.run());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            resources::cached_resources,
            resources::update_resources,
            desktop::desktop_settings,
            desktop::save_desktop_settings,
            desktop::set_autostart,
            desktop::desktop_ready,
            desktop::exit_desktop,
            ipc::backup::choose_backup,
            ipc::backup::export_backup_package,
            ipc::backup::share_replay,
            ipc::backup::export_backup,
            ipc::backup::restore_backup,
            ipc::observations::live_workspace,
            ipc::observations::sync_workspace,
            ipc::observations::play_page,
            ipc::observations::save_live_ui,
            ipc::observations::end_live_session,
            ipc::observations::retract_manual_game,
            ipc::observations::move_live_matches,
            ipc::following::fill_following_history,
            ipc::following::following_state,
            ipc::following::follow_player,
            ipc::following::edit_following,
            ipc::following::unfollow_player,
            ipc::following::sync_following,
            ipc::following::following_feed,
            ipc::observations::rename_live_session,
            ipc::observations::merge_live_sessions,
            ipc::observations::split_live_session,
            ipc::library::ensure_review_timeline,
            ipc::library::query_regions,
            ipc::library::search_player,
            ipc::library::search_current_player,
            ipc::library::backfill_games,
            ipc::library::player_history_page,
            ipc::library::open_review_game,
            ipc::library::saved_review_game,
            ipc::library::list_library,
            ipc::library::bookmark_review_game,
            ipc::library::backfill_review_game,
            ipc::replays::seek_replay,
            ipc::replays::replay_directory,
            ipc::replays::choose_replay_directory,
            ipc::replays::import_replay,
            ipc::replays::replay_jobs,
            ipc::replays::download_replay,
            ipc::replays::open_replay,
            ipc::replays::reveal_replay,
            ipc::replays::cancel_replay,
            ipc::replays::remove_replay
        ])
        .run(tauri::generate_context!())
        .expect("无法启动 LeagueReplay");
}
