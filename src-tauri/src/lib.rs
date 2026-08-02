use dotenvy::dotenv;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::{env, sync::Mutex, time::Duration};
use tauri::Manager;

struct OverlayState {
    hidden: Mutex<bool>,
    minimized: Mutex<bool>,
}

const RUST_APP_ID: u32 = 252490;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SteamLookupResult {
    steam_id: String,
    persona_name: String,
    profile_url: String,
    avatar_url: String,
    community_visibility_state: u8,
    owned_games_visible: bool,
    total_games: Option<u32>,
    rust_hours: Option<f32>,
    total_hours: Option<f32>,
    non_rust_hours: Option<f32>,
    concentration_ratio: Option<f32>,
    heuristic_level: String,
    heuristic_summary: String,
    notes: Vec<String>,
    top_other_games: Vec<GameHours>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GameHours {
    app_id: u32,
    name: String,
    hours: f32,
}

#[derive(Deserialize)]
struct PlayerSummariesResponse {
    response: PlayerSummariesBody,
}

#[derive(Deserialize)]
struct PlayerSummariesBody {
    players: Vec<SteamPlayer>,
}

#[derive(Deserialize)]
struct SteamPlayer {
    steamid: String,
    personaname: String,
    profileurl: String,
    #[serde(default)]
    avatarfull: String,
    #[serde(default)]
    communityvisibilitystate: u8,
}

#[derive(Deserialize)]
struct OwnedGamesResponse {
    #[serde(default)]
    response: OwnedGamesBody,
}

#[derive(Default, Deserialize)]
struct OwnedGamesBody {
    #[serde(default)]
    game_count: u32,
    #[serde(default)]
    games: Vec<OwnedGame>,
}

#[derive(Deserialize)]
struct OwnedGame {
    appid: u32,
    #[serde(default)]
    name: String,
    #[serde(default)]
    playtime_forever: u32,
}

fn set_hidden_state(app: &tauri::AppHandle, hidden: bool) {
    if let Some(state) = app.try_state::<OverlayState>() {
        if let Ok(mut value) = state.hidden.lock() {
            *value = hidden;
        }
        if let Ok(mut value) = state.minimized.lock() {
            if hidden {
                *value = false;
            }
        }
    }
}

fn set_minimized_state(app: &tauri::AppHandle, minimized: bool) {
    if let Some(state) = app.try_state::<OverlayState>() {
        if let Ok(mut value) = state.minimized.lock() {
            *value = minimized;
        }
        if let Ok(mut value) = state.hidden.lock() {
            if minimized {
                *value = false;
            }
        }
    }
}

fn set_window_hidden(app: &tauri::AppHandle, hidden: bool) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;

    if hidden {
        window.hide().map_err(|err| err.to_string())?;
    } else {
        window.show().map_err(|err| err.to_string())?;
        window.unminimize().map_err(|err| err.to_string())?;
    }

    set_hidden_state(app, hidden);
    Ok(())
}

fn set_window_minimized(app: &tauri::AppHandle, minimized: bool) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;

    if minimized {
        window.minimize().map_err(|err| err.to_string())?;
    } else {
        window.show().map_err(|err| err.to_string())?;
        window.unminimize().map_err(|err| err.to_string())?;
    }

    set_minimized_state(app, minimized);
    Ok(())
}

fn toggle_presence(app: &tauri::AppHandle) -> Result<(), String> {
    let minimized = app
        .try_state::<OverlayState>()
        .and_then(|state| state.minimized.lock().ok().map(|value| *value))
        .unwrap_or(false);
    let hidden = app
        .try_state::<OverlayState>()
        .and_then(|state| state.hidden.lock().ok().map(|value| *value))
        .unwrap_or(false);

    if minimized {
        set_window_minimized(app, false)
    } else if hidden {
        set_window_hidden(app, false)
    } else {
        set_window_hidden(app, true)
    }
}

#[tauri::command]
fn minimize_main_window(app: tauri::AppHandle) -> Result<(), String> {
    set_window_minimized(&app, true)
}

#[tauri::command]
fn begin_window_drag(app: tauri::AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;
    window.start_dragging().map_err(|err| err.to_string())
}

#[tauri::command]
fn lookup_steam_profile(steam_id: String) -> Result<SteamLookupResult, String> {
    let steam_id = normalize_steam_id(&steam_id)?;
    let api_key = env::var("STEAM_WEB_API_KEY")
        .or_else(|_| env::var("STEAM_API_KEY"))
        .map_err(|_| {
            "Missing STEAM_WEB_API_KEY in the app environment. Set it before launching Sus Check."
                .to_string()
        })?;

    let client = Client::builder()
        .timeout(Duration::from_secs(12))
        .build()
        .map_err(|err| err.to_string())?;

    let summary_url = "https://partner.steam-api.com/ISteamUser/GetPlayerSummaries/v2/";
    let summary_response = client
        .get(summary_url)
        .query(&[("key", api_key.as_str()), ("steamids", steam_id.as_str())])
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|err| format!("Steam summaries lookup failed: {err}"))?
        .json::<PlayerSummariesResponse>()
        .map_err(|err| format!("Steam summaries parse failed: {err}"))?;

    let player = summary_response
        .response
        .players
        .into_iter()
        .next()
        .ok_or_else(|| "Steam did not return a player for that SteamID64.".to_string())?;

    let owned_games_url = "https://partner.steam-api.com/IPlayerService/GetOwnedGames/v1/";
    let owned_games_response = client
        .get(owned_games_url)
        .query(&[
            ("key", api_key.as_str()),
            ("steamid", steam_id.as_str()),
            ("include_appinfo", "true"),
            ("include_played_free_games", "true"),
        ])
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|err| format!("Steam owned games lookup failed: {err}"))?
        .json::<OwnedGamesResponse>()
        .map_err(|err| format!("Steam owned games parse failed: {err}"))?;

    Ok(build_lookup_result(player, owned_games_response.response))
}

fn normalize_steam_id(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.len() != 17 || !trimmed.chars().all(|character| character.is_ascii_digit()) {
        return Err("Enter a 17-digit SteamID64.".to_string());
    }
    Ok(trimmed.to_string())
}

fn build_lookup_result(player: SteamPlayer, owned_games: OwnedGamesBody) -> SteamLookupResult {
    let owned_games_visible = !owned_games.games.is_empty();
    let rust_game = owned_games.games.iter().find(|game| game.appid == RUST_APP_ID);
    let rust_hours = rust_game.map(|game| minutes_to_hours(game.playtime_forever));
    let total_minutes: u32 = owned_games.games.iter().map(|game| game.playtime_forever).sum();
    let total_hours = owned_games_visible.then(|| minutes_to_hours(total_minutes));

    let mut top_other_games = owned_games
        .games
        .iter()
        .filter(|game| game.appid != RUST_APP_ID && game.playtime_forever > 0)
        .map(|game| GameHours {
            app_id: game.appid,
            name: if game.name.is_empty() {
                format!("App {}", game.appid)
            } else {
                game.name.clone()
            },
            hours: minutes_to_hours(game.playtime_forever),
        })
        .collect::<Vec<_>>();
    top_other_games.sort_by(|left, right| right.hours.total_cmp(&left.hours));
    top_other_games.truncate(3);

    let non_rust_minutes = total_minutes.saturating_sub(rust_game.map(|game| game.playtime_forever).unwrap_or(0));
    let non_rust_hours = owned_games_visible.then(|| minutes_to_hours(non_rust_minutes));
    let concentration_ratio = if owned_games_visible && total_minutes > 0 {
        rust_game.map(|game| game.playtime_forever as f32 / total_minutes as f32)
    } else {
        None
    };

    let mut notes = Vec::new();
    let (heuristic_level, heuristic_summary) = if !owned_games_visible {
        notes.push(
            "Owned games are not visible from this profile or your key lacks access to that data."
                .to_string(),
        );
        (
            "unknown".to_string(),
            "Public library data is unavailable, so playtime concentration cannot be assessed."
                .to_string(),
        )
    } else {
        if let Some(rust_hours) = rust_hours {
            if rust_hours >= 1500.0 && non_rust_minutes <= 6000 {
                notes.push(
                    "Heavy Rust concentration with very little visible playtime elsewhere can be worth a closer look, but it is still only a heuristic."
                        .to_string(),
                );
                (
                    "warn".to_string(),
                    "Library appears highly Rust-concentrated relative to visible playtime in other games."
                        .to_string(),
                )
            } else if rust_hours < 200.0 {
                (
                    "info".to_string(),
                    "Rust playtime is still relatively low; that can matter for context but is not a cheating signal by itself."
                        .to_string(),
                )
            } else {
                (
                    "neutral".to_string(),
                    "Visible game library looks more mixed, so playtime concentration alone is not especially notable."
                        .to_string(),
                )
            }
        } else {
            notes.push("Rust was not present in the visible owned-games response.".to_string());
            (
                "neutral".to_string(),
                "This visible library does not currently show Rust ownership or playtime."
                    .to_string(),
            )
        }
    };

    SteamLookupResult {
        steam_id: player.steamid,
        persona_name: player.personaname,
        profile_url: player.profileurl,
        avatar_url: player.avatarfull,
        community_visibility_state: player.communityvisibilitystate,
        owned_games_visible,
        total_games: owned_games_visible.then_some(owned_games.game_count),
        rust_hours,
        total_hours,
        non_rust_hours,
        concentration_ratio,
        heuristic_level,
        heuristic_summary,
        notes,
        top_other_games,
    }
}

fn minutes_to_hours(minutes: u32) -> f32 {
    ((minutes as f32) / 60.0 * 10.0).round() / 10.0
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = dotenv();

    tauri::Builder::default()
        .manage(OverlayState {
            hidden: Mutex::new(false),
            minimized: Mutex::new(false),
        })
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_shortcuts(["Alt+Shift+P", "Alt+Shift+M"])
                .expect("failed to register default shortcuts")
                .with_handler(|app, shortcut, event| {
                    #[cfg(desktop)]
                    {
                        use tauri_plugin_global_shortcut::{Code, Modifiers, ShortcutState};

                        if event.state == ShortcutState::Pressed {
                            if let Some(window) = app.get_webview_window("main") {
                                if shortcut.matches(Modifiers::ALT | Modifiers::SHIFT, Code::KeyP)
                                {
                                    let _ = toggle_presence(app);
                                }

                                if shortcut.matches(Modifiers::ALT | Modifiers::SHIFT, Code::KeyM)
                                {
                                    let minimized = app
                                        .try_state::<OverlayState>()
                                        .and_then(|state| {
                                            state.minimized.lock().ok().map(|value| *value)
                                        })
                                        .unwrap_or_else(|| window.is_minimized().unwrap_or(false));
                                    let _ = set_window_minimized(app, !minimized);
                                }
                            }
                        }
                    }
                })
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let window = app.get_webview_window("main").expect("main window");
            window.set_always_on_top(true)?;
            window.set_skip_taskbar(true)?;
            window.set_ignore_cursor_events(false)?;
            set_hidden_state(app.handle(), false);
            set_minimized_state(app.handle(), false);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            minimize_main_window,
            begin_window_drag,
            lookup_steam_profile
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
