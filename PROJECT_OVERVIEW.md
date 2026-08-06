# Sus Check: Project Overview

Sus Check is a Windows desktop overlay for manually reviewing a Rust player's public Steam profile and optional BattleMetrics session data. It is a separate, always-on-top Tauri window; it does not inject into Rust, read game memory, hook rendering, or attempt anti-cheat evasion.

The app is intended to provide research context after an in-game interaction, not to determine whether a player is cheating.

## What it does today

1. Accepts a 17-digit SteamID64.
2. Looks up the account profile, ban record, and visible owned-game/playtime data through the Steam Web API.
3. Optionally queries BattleMetrics for the configured server IDs, totals returned `timePlayed` values, and—when Steam reports a ban—retrieves dated sessions to calculate tracked hours after that ban.
4. Displays account identity, visible Rust hours, other game hours, ban context, session-hour comparisons, and a transparent weighted review decision.

The UI includes dark, light, and HUD themes, adjustable panel opacity, a custom draggable title bar, and a details view for raw derived values.

## Architecture

```text
React / Vite UI (src/)
        │ Tauri commands
        ▼
Rust backend (src-tauri/src/lib.rs)
        ├── Steam Web API
        │   ├── GetPlayerSummaries
        │   ├── GetPlayerBans
        │   └── GetOwnedGames
        └── BattleMetrics Players API (optional)
            └── Player session-history API (when Steam reports a ban)
```

| Area | Location | Responsibility |
| --- | --- | --- |
| Overlay UI | `src/App.jsx`, `src/App.css` | SteamID input, results, themes, window controls, and display formatting. |
| Client query state | `src/main.jsx` | React Query cache and request lifecycle. |
| Desktop/backend logic | `src-tauri/src/lib.rs` | Tauri commands, global shortcuts, window positioning, API requests, calculations, and session logging. |
| Native app configuration | `src-tauri/tauri.conf.json` | Transparent, undecorated, topmost, fixed-size desktop window. |
| Dependencies | `package.json`, `src-tauri/Cargo.toml` | JavaScript and Rust build/runtime dependencies. |

## Configuration

Copy `.env.example` to `.env` and replace the placeholders:

```env
STEAM_WEB_API_KEY=replace_me
BATTLEMETRICS_API_TOKEN=replace_me
BATTLEMETRICS_SERVER_IDS=1234567,7654321
```

`STEAM_WEB_API_KEY` is required for lookups. `STEAM_API_KEY` is also accepted by the backend as a fallback name.

BattleMetrics configuration is optional. If its token or server IDs are absent, the application still completes Steam lookups and reports the session-data state as `missing config`. BattleMetrics results depend on the token having appropriate owner/admin access to the configured servers.

## Running locally

Use a native Windows shell with Node.js, the Rust toolchain, and the platform prerequisites required by Tauri 2 installed:

```powershell
npm install
npm run tauri dev
```

Useful package commands:

| Command | Purpose |
| --- | --- |
| `npm run dev` | Start only the Vite frontend dev server. |
| `npm run build` | Create the frontend production build in `dist/`. |
| `npm run tauri dev` | Run the complete desktop application in development. |
| `npm run tauri build` | Build the packaged desktop application. |

## Overlay behavior

The app starts as a 384 × 560 transparent, borderless window. At runtime, the backend applies a monitor-relative placement/layout while preserving its minimum size. It is always on top and skipped from the taskbar.

Global shortcuts:

- `Alt+Shift+P` — hide or restore the overlay.
- `Alt+Shift+M` — minimize or restore the overlay.

## Weighted decision tree

The backend uses a weighted, evidence-gated decision tree. Every returned branch includes its label, evidence status, explanation, and contribution to a `0–100` review priority, so the UI does not hide its reasoning behind a single opaque score.

| Branch | Weight | Meaning |
| --- | ---: | --- |
| VAC ban history | 65–85 when recent; sharply decays with age | A factual Steam account-history record. The base contribution falls to one eighth once the most recent ban is at least three years old. |
| Game ban history | 25–45 when recent; sharply decays with age | A factual Steam account-history record; it does not identify the game or reason. |
| Repeated ban pattern | 60–80 | Two or more total Steam bans remain a strong review pattern even when the latest ban is old. |
| Playtime coverage gap | 20 | A review prompt only when high Steam-recorded Rust hours substantially exceed the configured BattleMetrics server-session total. |
| Crosshair-overlay inconsistency | 15 | A supplemental addition only when at least 10,000 Rust hours, 1,000 Crosshair X hours, a 5,000-hour Steam/BattleMetrics gap, and a session-to-Steam ratio at or below 25% are all present. Crosshair overlays are legitimate tools and this does not establish cheating or account purchase. |
| Dated post-ban tracked history | −10 at 1,000 hours; −20 at 3,000 hours | Counter-evidence from configured BattleMetrics servers that reduces the priority of historical ban context. |
| Missing Steam/BattleMetrics data | 0 | Evidence is unavailable; absence of data does not add review weight. |

The review-priority labels are `no action` below 20, `low review` from 20–44, `manual` from 45–74, and `high review` from 75–100. The UI separately reports **evidence coverage**: the proportion of the available Steam profile, visible-library, BattleMetrics aggregate, and—where a ban exists—dated session-history inputs that were successfully returned. It is not a probability that a player is cheating.

BattleMetrics hours are a lower bound for only the configured servers—not a global total—and a gap never proves idle or fabricated Steam hours. For accounts with a Steam ban, the app retrieves up to 20 pages of dated BattleMetrics sessions per matched player to calculate hours after the latest ban. If history is unavailable or exceeds that limit, no post-ban credit is applied. No outcome should be treated as proof of cheating or as the sole basis for moderation action.

## Data handling and privacy

Each successful lookup can be written as JSON Lines containing the submitted SteamID, raw Steam API responses, and the derived assessment. Logging is currently enabled in `src-tauri/src/lib.rs`, but configured as session-only:

- Logs are stored under the operating system temporary directory.
- The active session log directory is removed on normal shutdown.
- Stale Sus Check session logs are removed when the app starts.
- `clear_session_logs` and `shutdown_app` also request cleanup.

Because logs may include player identifiers and API-returned profile data, avoid distributing them and review this behavior before changing logging to persistent storage.

## Current boundaries and limitations

- Input is limited to a numeric, 17-character SteamID64; profile URLs, vanity names, and Steam2/Steam3 IDs are not resolved.
- Steam privacy settings can prevent library and playtime data from being returned.
- BattleMetrics data is limited to the server IDs configured for the app and may return no matching record.
- Granular BattleMetrics session history is subject to API visibility, privacy settings, and retention limits; unavailable or partial history does not produce post-ban counter-evidence.
- API calls are synchronous Rust HTTP requests with a 12-second client timeout.
- There are no automated test scripts defined yet in `package.json` or Cargo metadata.

## Safety boundary

Sus Check is designed as an external desktop utility. Its intended boundary is:

- no DLL injection;
- no game-process memory reads or writes;
- no rendering hooks;
- no kernel drivers or anti-cheat-evasion techniques.
