# LeagueReplay

[简体中文](README.md) · English · [日本語](README.ja.md) · [한국어](README.ko.md)

A local match tracker and replay review tool for League of Legends. Organize your games across accounts, save other players' matches, and learn with replays, item timelines, and notes.

**Windows x64 · Development preview · MIT**

## Features

- Automatically record games played on your PC; add missed matches and organize play sessions.
- Look up match history by region and Riot ID, and follow players' recent games.
- Download, import, and play ROFL replays; add timestamped notes and player annotations.
- Keep data locally, back up and restore it, or share a single replay with notes.
- Simplified Chinese, English, Japanese, and Korean interfaces.

## Getting started

Requires League of Legends and WebView2. [Download the Windows x64 installer for the v0.2.0 preview](https://github.com/0xMashiro/LeagueReplay/releases/tag/v0.2.0), or [build from source](CONTRIBUTING.md). Preview installers are unsigned; automatic updates are not yet available.

1. Sign in to the game client, launch LeagueReplay, and wait for it to connect.
2. Keep the app running to record your games. To find another player, select a region and enter their full Riot ID.
3. Open a match to download an available replay and add notes. Backup and restore are in settings.

Closing the window minimizes the app to the tray by default; recording and downloads continue. Choose “Exit” in the tray to stop it.

## Region and replay limits

| Signed-in session | Online query scope |
| --- | --- |
| Signed out | Cached local data only |
| Tencent (mainland China) | Current and configured Tencent servers |
| Riot | Current and configured Riot servers |

**Tencent and Riot access is separate. Installing the Chinese client does not grant access to Korean match history.** Cross-region queries within one operator can be attempted, but availability depends on the target service and has not been verified in every region. If access is denied, try signing in to the target region. Empty results do not mean the player is missing.

Queries and downloads require a signed-in client. Replays must still be available from the server, and playback requires a compatible game version. Cached data is available offline. The app handles game replays, not livestream video downloads or live spectating.

To seek from timestamped notes, set `EnableReplayApi=1` under `[General]` in the game's `Config/game.cfg`.

## Contributing and license

Bug reports, code contributions, and translation corrections are welcome. See the [contribution guide](CONTRIBUTING.md) (Chinese) and [security reporting](SECURITY.md) (Chinese).

Source code is licensed under [MIT](LICENSE). See [third-party notices](third-party/) and the [rofl-inspect license](src-tauri/crates/rofl-inspect/LICENSE). Game assets belong to Riot Games and are excluded from the code license. LeagueReplay is not affiliated with Riot Games.
