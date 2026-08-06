use dotenvy::dotenv;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::{
    env,
    fs::{OpenOptions, create_dir_all, remove_dir_all},
    io::Write,
    path::PathBuf,
    sync::{Mutex, OnceLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{LogicalPosition, LogicalSize, Manager, PhysicalPosition};

struct OverlayState {
    hidden: Mutex<bool>,
    minimized: Mutex<bool>,
}

const RUST_APP_ID: u32 = 252490;
const ENABLE_LOOKUP_LOGGING: bool = true;
const SESSION_ONLY_LOOKUP_LOGGING: bool = true;
const SESSION_LOG_ROOT_DIR: &str = "sus-check-session-logs";

static SESSION_LOG_DIR: OnceLock<PathBuf> = OnceLock::new();

struct WindowLayoutConfig {
    sidebar_width: f64,
    margin: f64,
    min_width: f64,
    min_height: f64,
    height_ratio: f64,
}

const WINDOW_LAYOUT: WindowLayoutConfig = WindowLayoutConfig {
    sidebar_width: 384.0,
    margin: 24.0,
    min_width: 384.0,
    min_height: 560.0,
    height_ratio: 0.78,
};

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct SteamLookupResult {
    profile: ProfileSnapshot,
    scores: ScoreBundle,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ProfileSnapshot {
    steam_id: String,
    persona_name: String,
    profile_url: String,
    avatar_url: String,
    community_visibility_state: u8,
    ban_context: BanContext,
    playtime_context: PlaytimeContext,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct GameHours {
    app_id: u32,
    name: String,
    hours: f32,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct BanRiskSummary {
    label: String,
    score: u32,
    vac_bans: u32,
    game_bans: u32,
    days_since_last_ban: Option<u32>,
    economy_ban: String,
    community_banned: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct BanContext {
    vac_bans: u32,
    game_bans: u32,
    days_since_last_ban: Option<u32>,
    economy_ban: String,
    community_banned: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PlaytimeContext {
    owned_games_visible: bool,
    total_games: Option<u32>,
    rust_hours: Option<f32>,
    total_hours: Option<f32>,
    non_rust_hours: Option<f32>,
    concentration_ratio: Option<f32>,
    battlemetrics_session_hours: Option<f32>,
    battlemetrics_status: String,
    steam_minus_session_hours: Option<f32>,
    session_to_steam_ratio: Option<f32>,
    authenticity_confidence: String,
    notes: Vec<String>,
    top_other_games: Vec<GameHours>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ScoreBundle {
    overall: ScoreCard,
    modules: ScoreModules,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ScoreModules {
    bans: ScoreCard,
    playtime: ScoreCard,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ScoreCard {
    key: String,
    score: u32,
    label: String,
    weight: u32,
    summary: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct PlayerSummariesResponse {
    response: PlayerSummariesBody,
}

#[derive(Serialize, Deserialize, Clone)]
struct PlayerSummariesBody {
    players: Vec<SteamPlayer>,
}

#[derive(Serialize, Deserialize, Clone)]
struct SteamPlayer {
    steamid: String,
    personaname: String,
    profileurl: String,
    #[serde(default)]
    avatarfull: String,
    #[serde(default)]
    communityvisibilitystate: u8,
}

#[derive(Serialize, Deserialize, Clone)]
struct OwnedGamesResponse {
    #[serde(default)]
    response: OwnedGamesBody,
}

#[derive(Default, Serialize, Deserialize, Clone)]
struct OwnedGamesBody {
    #[serde(default)]
    game_count: u32,
    #[serde(default)]
    games: Vec<OwnedGame>,
}

#[derive(Serialize, Deserialize, Clone)]
struct OwnedGame {
    appid: u32,
    #[serde(default)]
    name: String,
    #[serde(default)]
    playtime_forever: u32,
}

#[derive(Serialize, Deserialize, Clone)]
struct PlayerBansResponse {
    players: Vec<PlayerBanRecord>,
}

#[derive(Deserialize)]
struct BattleMetricsPlayersResponse {
    data: Vec<BattleMetricsPlayerRecord>,
}

#[derive(Deserialize)]
struct BattleMetricsPlayerRecord {
    id: String,
    attributes: BattleMetricsPlayerAttributes,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BattleMetricsPlayerAttributes {
    #[serde(default)]
    time_played: Option<f64>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
struct PlayerBanRecord {
    #[serde(default)]
    community_banned: bool,
    #[serde(default)]
    vac_banned: bool,
    #[serde(default)]
    number_of_vac_bans: u32,
    #[serde(default)]
    days_since_last_ban: u32,
    #[serde(default)]
    number_of_game_bans: u32,
    #[serde(default)]
    economy_ban: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct LookupLogEvent {
    schema_version: u8,
    event_type: String,
    timestamp_ms: u128,
    input: LookupLogInput,
    raw: LookupLogRaw,
    assessment: SteamLookupResult,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct LookupLogInput {
    steam_id_raw: String,
    steam_id_normalized: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct LookupLogRaw {
    player_summaries_response: PlayerSummariesResponse,
    player_bans_response: PlayerBansResponse,
    owned_games_response: OwnedGamesResponse,
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

fn apply_monitor_sized_layout(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    let monitor = window.current_monitor()?;

    if let Some(monitor) = monitor {
        let scale_factor = monitor.scale_factor();
        let monitor_size = monitor.size().to_logical::<f64>(scale_factor);
        let monitor_position = monitor.position().to_logical::<f64>(scale_factor);

        let width = WINDOW_LAYOUT
            .sidebar_width
            .min((monitor_size.width - WINDOW_LAYOUT.margin * 2.0).max(WINDOW_LAYOUT.min_width));
        let available_height =
            (monitor_size.height - WINDOW_LAYOUT.margin * 2.0).max(WINDOW_LAYOUT.min_height);
        let height = (monitor_size.height * WINDOW_LAYOUT.height_ratio)
            .min(available_height)
            .max(WINDOW_LAYOUT.min_height);
        let x = monitor_position.x + monitor_size.width - width - WINDOW_LAYOUT.margin;
        let y = monitor_position.y + WINDOW_LAYOUT.margin;

        window.set_size(LogicalSize::new(width, height))?;
        window.set_min_size(Some(LogicalSize::new(
            WINDOW_LAYOUT.min_width,
            WINDOW_LAYOUT.min_height,
        )))?;
        window.set_position(LogicalPosition::new(x, y))?;
    }

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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WindowDragState {
    cursor_x: f64,
    cursor_y: f64,
    window_x: i32,
    window_y: i32,
}

#[tauri::command]
fn get_window_drag_state(app: tauri::AppHandle) -> Result<WindowDragState, String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;
    let cursor = window.cursor_position().map_err(|err| err.to_string())?;
    let position = window.outer_position().map_err(|err| err.to_string())?;

    Ok(WindowDragState {
        cursor_x: cursor.x,
        cursor_y: cursor.y,
        window_x: position.x,
        window_y: position.y,
    })
}

#[tauri::command]
fn set_window_position(app: tauri::AppHandle, x: i32, y: i32) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;
    window
        .set_position(PhysicalPosition::new(x, y))
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn clear_session_logs() -> Result<bool, String> {
    cleanup_session_logs()?;
    Ok(true)
}

#[tauri::command]
fn shutdown_app(app: tauri::AppHandle) -> Result<bool, String> {
    cleanup_session_logs()?;
    app.exit(0);
    Ok(true)
}

#[tauri::command]
fn lookup_steam_profile(app: tauri::AppHandle, steam_id: String) -> Result<SteamLookupResult, String> {
    let steam_id_raw = steam_id.clone();
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

    let summary_url = "https://api.steampowered.com/ISteamUser/GetPlayerSummaries/v2/";
    let summary_response = client
        .get(summary_url)
        .query(&[("key", api_key.as_str()), ("steamids", steam_id.as_str())])
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|err| format!("Steam summaries lookup failed: {err}"))?
        .json::<PlayerSummariesResponse>()
        .map_err(|err| format!("Steam summaries parse failed: {err}"))?;

    let player = summary_response
        .clone()
        .response
        .players
        .into_iter()
        .next()
        .ok_or_else(|| "Steam did not return a player for that SteamID64.".to_string())?;

    let bans_url = "https://api.steampowered.com/ISteamUser/GetPlayerBans/v1/";
    let bans_response = client
        .get(bans_url)
        .query(&[("key", api_key.as_str()), ("steamids", steam_id.as_str())])
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|err| format!("Steam bans lookup failed: {err}"))?
        .json::<PlayerBansResponse>()
        .map_err(|err| format!("Steam bans parse failed: {err}"))?;

    let ban_record = bans_response
        .clone()
        .players
        .into_iter()
        .next()
        .ok_or_else(|| "Steam did not return ban data for that SteamID64.".to_string())?;

    let owned_games_url = "https://api.steampowered.com/IPlayerService/GetOwnedGames/v1/";
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

    let battlemetrics_lookup = fetch_battlemetrics_session_hours(&client, &steam_id);

    let assessment = build_lookup_result(
        battlemetrics_lookup,
        player,
        ban_record,
        owned_games_response.response.clone(),
    );

    let event = LookupLogEvent {
        schema_version: 1,
        event_type: "steam_lookup".to_string(),
        timestamp_ms: now_timestamp_ms(),
        input: LookupLogInput {
            steam_id_raw,
            steam_id_normalized: steam_id,
        },
        raw: LookupLogRaw {
            player_summaries_response: summary_response,
            player_bans_response: bans_response,
            owned_games_response,
        },
        assessment: assessment.clone(),
    };

    if ENABLE_LOOKUP_LOGGING {
        if let Err(error) = append_lookup_log(&app, &event) {
            eprintln!("sus-check logging failed: {error}");
        }
    }

    Ok(assessment)
}

fn normalize_steam_id(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.len() != 17 || !trimmed.chars().all(|character| character.is_ascii_digit()) {
        return Err("Enter a 17-digit SteamID64.".to_string());
    }
    Ok(trimmed.to_string())
}

fn build_lookup_result(
    battlemetrics_lookup: BattleMetricsLookup,
    player: SteamPlayer,
    ban_record: PlayerBanRecord,
    owned_games: OwnedGamesBody,
) -> SteamLookupResult {
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

    let non_rust_minutes = total_minutes.saturating_sub(rust_game.map(|game| game.playtime_forever).unwrap_or(0));
    let non_rust_hours = owned_games_visible.then(|| minutes_to_hours(non_rust_minutes));
    let concentration_ratio = if owned_games_visible && total_minutes > 0 {
        rust_game.map(|game| game.playtime_forever as f32 / total_minutes as f32)
    } else {
        None
    };
    let battlemetrics_session_hours = battlemetrics_lookup.session_hours;
    let battlemetrics_status = battlemetrics_lookup.status;
    let steam_minus_session_hours = rust_hours.zip(battlemetrics_session_hours).map(|(steam, session)| {
        (steam - session).max(0.0)
    });
    let session_to_steam_ratio = rust_hours.zip(battlemetrics_session_hours).and_then(|(steam, session)| {
        (steam > 0.0).then_some((session / steam).clamp(0.0, 1.0))
    });

    let mut notes = Vec::new();
    let (playtime_label, playtime_summary, authenticity_confidence) = if !owned_games_visible {
        notes.push(
            "Owned games are not visible from this profile or your key lacks access to that data."
                .to_string(),
        );
        (
            "unknown".to_string(),
            "Public library data is unavailable, so playtime concentration cannot be assessed."
                .to_string(),
            "low".to_string(),
        )
    } else {
        if let Some(session_hours) = battlemetrics_session_hours {
            if let (Some(steam_hours), Some(gap_hours), Some(ratio)) =
                (rust_hours, steam_minus_session_hours, session_to_steam_ratio)
            {
                if steam_hours >= 1500.0 && ratio <= 0.25 && gap_hours >= 800.0 {
                    notes.push(
                        "Tracked server-session hours are much lower than Steam Rust hours. That gap can indicate inflated or low-quality hours, but it still needs context."
                            .to_string(),
                    );
                    (
                        "mismatch".to_string(),
                        format!(
                            "Steam Rust hours are far above tracked server-session hours ({steam_hours:.1}h vs {session_hours:.1}h)."
                        ),
                        "high".to_string(),
                    )
                } else if steam_hours >= 500.0 && ratio <= 0.45 && gap_hours >= 250.0 {
                    (
                        "watch".to_string(),
                        format!(
                            "Steam Rust hours are materially higher than tracked server-session hours ({steam_hours:.1}h vs {session_hours:.1}h)."
                        ),
                        "high".to_string(),
                    )
                } else {
                    (
                        "tracked".to_string(),
                        format!(
                            "Tracked server-session hours are reasonably aligned with Steam Rust hours ({session_hours:.1}h tracked / {steam_hours:.1}h Steam)."
                        ),
                        "high".to_string(),
                    )
                }
            } else {
                (
                    "tracked".to_string(),
                    "BattleMetrics session data is present, but the Steam-vs-session comparison is incomplete."
                        .to_string(),
                    "medium".to_string(),
                )
            }
        } else if let Some(rust_hours) = rust_hours {
            if rust_hours >= 1500.0 && non_rust_minutes <= 6000 {
                notes.push(
                    format!(
                        "BattleMetrics session hours are unavailable for this lookup ({battlemetrics_status}), so this remains only a Steam-side concentration heuristic for now."
                    )
                        .to_string(),
                );
                (
                    "pending session data".to_string(),
                    format!(
                        "Steam Rust hours are high, but tracked server-session hours are unavailable for this lookup ({battlemetrics_status})."
                    ),
                    "low".to_string(),
                )
            } else if rust_hours < 200.0 {
                (
                    "early account".to_string(),
                    format!(
                        "Rust playtime is still relatively low; tracked session-hour comparison is unavailable for this lookup ({battlemetrics_status})."
                    ),
                    "low".to_string(),
                )
            } else {
                (
                    "steam only".to_string(),
                    format!(
                        "Steam playtime is available, but tracked server-session hours are unavailable for this lookup ({battlemetrics_status})."
                    ),
                    "low".to_string(),
                )
            }
        } else {
            notes.push("Rust was not present in the visible owned-games response.".to_string());
            (
                "neutral".to_string(),
                "This visible library does not currently show Rust ownership or playtime."
                    .to_string(),
                "low".to_string(),
            )
        }
    };

    let ban_risk = compress_ban_risk(&ban_record);
    let ban_score = ScoreCard {
        key: "bans".to_string(),
        score: ban_risk.score,
        label: ban_risk.label.clone(),
        weight: 45,
        summary: format!(
            "VAC {} | Game {} | Last {}",
            ban_risk.vac_bans,
            ban_risk.game_bans,
            ban_risk
                .days_since_last_ban
                .map(|days| format!("{days}d"))
                .unwrap_or_else(|| "none".to_string())
        ),
    };

    let playtime_score = ScoreCard {
        key: "playtime".to_string(),
        score: playtime_score_value(rust_hours, battlemetrics_session_hours, owned_games_visible),
        label: playtime_label.clone(),
        weight: 20,
        summary: playtime_summary.clone(),
    };

    let overall = combine_scores(&[ban_score.clone(), playtime_score.clone()]);

    let profile = ProfileSnapshot {
        steam_id: player.steamid,
        persona_name: player.personaname,
        profile_url: player.profileurl,
        avatar_url: player.avatarfull,
        community_visibility_state: player.communityvisibilitystate,
        ban_context: BanContext {
            vac_bans: ban_risk.vac_bans,
            game_bans: ban_risk.game_bans,
            days_since_last_ban: ban_risk.days_since_last_ban,
            economy_ban: ban_risk.economy_ban,
            community_banned: ban_risk.community_banned,
        },
        playtime_context: PlaytimeContext {
            owned_games_visible,
            total_games: owned_games_visible.then_some(owned_games.game_count),
            rust_hours,
            total_hours,
            non_rust_hours,
            concentration_ratio,
            battlemetrics_session_hours,
            battlemetrics_status,
            steam_minus_session_hours,
            session_to_steam_ratio,
            authenticity_confidence,
            notes,
            top_other_games,
        },
    };

    SteamLookupResult {
        profile,
        scores: ScoreBundle {
            overall,
            modules: ScoreModules {
                bans: ban_score,
                playtime: playtime_score,
            },
        },
    }
}

struct BattleMetricsLookup {
    session_hours: Option<f32>,
    status: String,
}

fn battlemetrics_config() -> Option<(String, Vec<String>)> {
    let token = env::var("BATTLEMETRICS_API_TOKEN")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())?;
    let server_ids = env::var("BATTLEMETRICS_SERVER_IDS")
        .ok()
        .map(|value| {
            value
                .split(',')
                .map(|item| item.trim().to_string())
                .filter(|item| !item.is_empty())
                .collect::<Vec<_>>()
        })
        .filter(|value| !value.is_empty())?;

    Some((token, server_ids))
}

fn fetch_battlemetrics_session_hours(
    client: &Client,
    steam_id: &str,
) -> BattleMetricsLookup {
    let Some((token, server_ids)) = battlemetrics_config() else {
        return BattleMetricsLookup {
            session_hours: None,
            status: "missing config".to_string(),
        };
    };

    let mut request = client
        .get("https://api.battlemetrics.com/players")
        .bearer_auth(token)
        .query(&[
            ("filter[search]", steam_id),
            ("page[size]", "100"),
        ]);

    for server_id in &server_ids {
        request = request.query(&[("filter[servers]", server_id.as_str())]);
    }

    let response = match request.send().and_then(|response| response.error_for_status()) {
        Ok(response) => response,
        Err(error) => {
            return BattleMetricsLookup {
                session_hours: None,
                status: format!("request failed: {error}"),
            };
        }
    };

    let payload = match response.json::<BattleMetricsPlayersResponse>() {
        Ok(payload) => payload,
        Err(error) => {
            return BattleMetricsLookup {
                session_hours: None,
                status: format!("parse failed: {error}"),
            };
        }
    };

    if payload.data.is_empty() {
        return BattleMetricsLookup {
            session_hours: None,
            status: "no matching player records".to_string(),
        };
    }

    let total_seconds = payload
        .data
        .iter()
        .filter_map(|record| record.attributes.time_played)
        .sum::<f64>();

    if total_seconds <= 0.0 {
        return BattleMetricsLookup {
            session_hours: None,
            status: format!("matched {} records without timePlayed", payload.data.len()),
        };
    }

    BattleMetricsLookup {
        session_hours: Some((((total_seconds / 3600.0) * 10.0).round() / 10.0) as f32),
        status: format!("matched {} records", payload.data.len()),
    }
}

fn playtime_score_value(
    rust_hours: Option<f32>,
    battlemetrics_session_hours: Option<f32>,
    owned_games_visible: bool,
) -> u32 {
    if !owned_games_visible {
        return 0;
    }

    if let (Some(steam_hours), Some(session_hours)) = (rust_hours, battlemetrics_session_hours) {
        if steam_hours <= 0.0 {
            return 0;
        }

        let gap_hours = (steam_hours - session_hours).max(0.0);
        let ratio = (session_hours / steam_hours).clamp(0.0, 1.0);

        let mut score = 0;
        if steam_hours >= 1500.0 && ratio <= 0.25 {
            score += 60;
        } else if steam_hours >= 500.0 && ratio <= 0.45 {
            score += 38;
        } else if steam_hours >= 250.0 && ratio <= 0.6 {
            score += 22;
        }

        if gap_hours >= 1200.0 {
            score += 30;
        } else if gap_hours >= 500.0 {
            score += 18;
        } else if gap_hours >= 200.0 {
            score += 8;
        }

        return score.min(100);
    }

    0
}

fn combine_scores(modules: &[ScoreCard]) -> ScoreCard {
    let total_weight: u32 = modules.iter().map(|module| module.weight).sum();
    let weighted_sum: u32 = modules
        .iter()
        .map(|module| module.score.saturating_mul(module.weight))
        .sum();
    let score = if total_weight == 0 {
        0
    } else {
        weighted_sum / total_weight
    };

    let label = if score >= 75 {
        "high risk"
    } else if score >= 45 {
        "elevated"
    } else if score > 0 {
        "low signal"
    } else {
        "insufficient data"
    };

    ScoreCard {
        key: "overall".to_string(),
        score,
        label: label.to_string(),
        weight: total_weight,
        summary: "Combined from module-level suspicion signals.".to_string(),
    }
}

fn append_lookup_log(app: &tauri::AppHandle, event: &LookupLogEvent) -> Result<(), String> {
    let log_dir = session_log_dir(app)?;
    create_dir_all(&log_dir).map_err(|err| format!("failed to create log dir: {err}"))?;

    let log_path = log_dir.join("steam_lookup_events.jsonl");
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|err| format!("failed to open log file {}: {err}", log_path.display()))?;

    let payload =
        serde_json::to_string(event).map_err(|err| format!("failed to serialize log event: {err}"))?;
    writeln!(file, "{payload}")
        .map_err(|err| format!("failed to append log event {}: {err}", log_path.display()))
}

fn now_timestamp_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn session_log_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    if !ENABLE_LOOKUP_LOGGING {
        return Err("logging is disabled".to_string());
    }

    if let Some(path) = SESSION_LOG_DIR.get() {
        return Ok(path.clone());
    }

    let base_dir = if SESSION_ONLY_LOOKUP_LOGGING {
        env::temp_dir().join(SESSION_LOG_ROOT_DIR)
    } else {
        app.path()
            .app_local_data_dir()
            .map_err(|err| format!("failed to resolve app local data dir: {err}"))?
            .join("logs")
    };

    let path = base_dir.join(format!("session-{}-{}", std::process::id(), now_timestamp_ms()));
    let _ = SESSION_LOG_DIR.set(path.clone());
    Ok(path)
}

fn cleanup_session_logs() -> Result<(), String> {
    if !(ENABLE_LOOKUP_LOGGING && SESSION_ONLY_LOOKUP_LOGGING) {
        return Ok(());
    }

    if let Some(path) = SESSION_LOG_DIR.get() {
        if path.exists() {
            remove_dir_all(path)
                .map_err(|err| format!("failed to remove session log dir {}: {err}", path.display()))?;
        }
    }

    Ok(())
}

fn cleanup_stale_session_log_root() -> Result<(), String> {
    if !(ENABLE_LOOKUP_LOGGING && SESSION_ONLY_LOOKUP_LOGGING) {
        return Ok(());
    }

    let root = env::temp_dir().join(SESSION_LOG_ROOT_DIR);
    if root.exists() {
        remove_dir_all(&root)
            .map_err(|err| format!("failed to remove stale session log root {}: {err}", root.display()))?;
    }

    Ok(())
}

fn compress_ban_risk(ban_record: &PlayerBanRecord) -> BanRiskSummary {
    let vac_bans = ban_record.number_of_vac_bans.max(u32::from(ban_record.vac_banned));
    let game_bans = ban_record.number_of_game_bans;
    let total_bans = vac_bans + game_bans;

    let recency_score = match ban_record.days_since_last_ban {
        0 if total_bans == 0 => 0,
        0..=30 => 30,
        31..=180 => 22,
        181..=365 => 16,
        366..=730 => 10,
        731..=1825 => 5,
        _ => 2,
    };

    let base_score = vac_bans * 45 + game_bans * 14;
    let repeat_bonus = if total_bans >= 4 {
        15
    } else if total_bans >= 2 {
        8
    } else {
        0
    };

    let score = (base_score + recency_score + repeat_bonus).min(100);

    let label = if total_bans == 0 {
        "clean"
    } else if vac_bans > 1 || (vac_bans >= 1 && ban_record.days_since_last_ban <= 180) {
        "high risk"
    } else if total_bans >= 2 {
        "repeat bans"
    } else if ban_record.days_since_last_ban <= 365 {
        "recent ban"
    } else {
        "old ban"
    };

    BanRiskSummary {
        label: label.to_string(),
        score,
        vac_bans,
        game_bans,
        days_since_last_ban: (total_bans > 0).then_some(ban_record.days_since_last_ban),
        economy_ban: ban_record.economy_ban.clone(),
        community_banned: ban_record.community_banned,
    }
}

fn minutes_to_hours(minutes: u32) -> f32 {
    ((minutes as f32) / 60.0 * 10.0).round() / 10.0
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = dotenv();
    if let Err(error) = cleanup_stale_session_log_root() {
        eprintln!("sus-check stale session log cleanup failed: {error}");
    }

    let app = tauri::Builder::default()
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
            apply_monitor_sized_layout(&window)?;
            set_hidden_state(app.handle(), false);
            set_minimized_state(app.handle(), false);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            minimize_main_window,
            begin_window_drag,
            get_window_drag_state,
            set_window_position,
            lookup_steam_profile,
            clear_session_logs,
            shutdown_app
        ]);

    app
        .run(tauri::generate_context!())
        .expect("error while running tauri application");

    if let Err(error) = cleanup_session_logs() {
        eprintln!("sus-check session log cleanup failed: {error}");
    }
}
