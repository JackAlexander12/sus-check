# Sus Check

Windows overlay tool for Rust-the-game, written in Rust with Tauri.

Current state:
- standalone desktop overlay process
- draggable custom title bar
- minimize and hide/show hotkeys
- first-pass SteamID64 lookup UI
- Steam account validation and visible playtime summary via Steam Web API

## Goals

Sus Check is intended as a local research aid after a death in Rust-the-game. The long-term goal is to surface compact signals such as Steam bans, account age, playtime patterns, and BattleMetrics-derived session context without injecting into the game or touching game memory.

## Safety Constraints

- No DLL injection
- No process memory reads or writes
- No render hooks
- No kernel or anti-cheat evasive techniques
- Overlay is a separate topmost OS window

## Stack

- Rust
- Tauri 2
- React / Vite for the current overlay UI
- `windows` crate planned for deeper Windows-native integrations
- Steam Web API for current development-phase profile lookup

## Current Development Slice

The current panel supports:
- entering a 17-digit SteamID64
- validating that the Steam account exists
- fetching visible library/playtime data
- showing Rust hours, non-Rust hours, visible game count, and top other games

This is intentionally framed as heuristic context only, not a cheating verdict.

## Environment

Create a local `.env` file in the repo root:

```env
STEAM_WEB_API_KEY=replace_me
BATTLEMETRICS_API_TOKEN=replace_me
BATTLEMETRICS_SERVER_IDS=1234567,7654321
```

The real `.env` file is gitignored. Use `.env.example` as the template.

## Local Run

From a native Windows shell:

```powershell
npm install
npm run tauri dev
```

## Hotkeys

- `Alt+Shift+P` hide or restore overlay
- `Alt+Shift+M` minimize or restore overlay

## Notes

- Steam profile/library visibility still limits what data can be shown.
- BattleMetrics Steam64ID lookup requires owner/admin access to the servers in `BATTLEMETRICS_SERVER_IDS`; if that scope or token is wrong, tracked session hours will stay unavailable.
- Requiring each end user to supply a Steam key is only acceptable for development. The intended production path is a hosted API provider.
