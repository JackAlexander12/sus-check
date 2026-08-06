use chrono::{DateTime, Duration as ChronoDuration, Utc};
use dotenvy::dotenv;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
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
const CROSSHAIR_X_APP_ID: u32 = 1_366_800;
const VAC_BAN_REVIEW_WEIGHT: u32 = 65;
const GAME_BAN_REVIEW_WEIGHT: u32 = 25;
const REPEATED_BAN_PATTERN_WEIGHT: i32 = 60;
const PLAYTIME_COVERAGE_REVIEW_WEIGHT: u32 = 20;
const CROSSHAIR_X_CONTEXT_WEIGHT: i32 = 15;
const ESTABLISHED_TRACKED_HISTORY_CREDIT: i32 = 20;
const SUBSTANTIAL_TRACKED_HISTORY_CREDIT: i32 = 10;
const MAX_BATTLEMETRICS_SESSION_HISTORY_PAGES: usize = 20;
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
    decision: DecisionTree,
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
    crosshair_x_hours: Option<f32>,
    total_hours: Option<f32>,
    non_rust_hours: Option<f32>,
    concentration_ratio: Option<f32>,
    battlemetrics_session_hours: Option<f32>,
    battlemetrics_status: String,
    battlemetrics_post_ban_hours: Option<f32>,
    battlemetrics_session_history_status: String,
    battlemetrics_recent_session_count: Option<u32>,
    steam_minus_session_hours: Option<f32>,
    session_to_steam_ratio: Option<f32>,
    notes: Vec<String>,
    top_other_games: Vec<GameHours>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DecisionTree {
    outcome: String,
    review_priority: u32,
    evidence_coverage: u32,
    evidence_level: String,
    summary: String,
    next_step: String,
    branches: Vec<DecisionBranch>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DecisionBranch {
    key: String,
    label: String,
    weight: i32,
    status: String,
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

#[derive(Deserialize)]
struct BattleMetricsSessionsResponse {
    data: Vec<BattleMetricsSessionRecord>,
    #[serde(default)]
    links: BattleMetricsPaginationLinks,
}

#[derive(Default, Deserialize)]
struct BattleMetricsPaginationLinks {
    #[serde(default)]
    next: Option<String>,
}

#[derive(Deserialize)]
struct BattleMetricsSessionRecord {
    id: String,
    attributes: BattleMetricsSessionAttributes,
    #[serde(default)]
    relationships: BattleMetricsSessionRelationships,
}

#[derive(Default, Deserialize)]
struct BattleMetricsSessionRelationships {
    #[serde(default)]
    server: Option<BattleMetricsRelationship>,
}

#[derive(Deserialize)]
struct BattleMetricsRelationship {
    #[serde(default)]
    data: Option<BattleMetricsResourceIdentifier>,
}

#[derive(Deserialize)]
struct BattleMetricsResourceIdentifier {
    id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BattleMetricsSessionAttributes {
    #[serde(default)]
    first_time: Option<String>,
    #[serde(default)]
    last_time: Option<String>,
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

    let battlemetrics_lookup = fetch_battlemetrics_session_hours(
        &client,
        &steam_id,
        (ban_record.vac_banned
            || ban_record.number_of_vac_bans > 0
            || ban_record.number_of_game_bans > 0)
            .then_some(ban_record.days_since_last_ban),
    );

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
    let crosshair_x_hours = owned_games
        .games
        .iter()
        .find(|game| game.appid == CROSSHAIR_X_APP_ID)
        .map(|game| minutes_to_hours(game.playtime_forever));
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
    let battlemetrics_post_ban_hours = battlemetrics_lookup.post_ban_hours;
    let battlemetrics_session_history_status = battlemetrics_lookup.session_history_status;
    let battlemetrics_recent_session_count = battlemetrics_lookup.recent_session_count;
    let steam_minus_session_hours = rust_hours.zip(battlemetrics_session_hours).map(|(steam, session)| {
        (steam - session).max(0.0)
    });
    let session_to_steam_ratio = rust_hours.zip(battlemetrics_session_hours).and_then(|(steam, session)| {
        (steam > 0.0).then_some((session / steam).clamp(0.0, 1.0))
    });

    let mut notes = Vec::new();
    if !owned_games_visible {
        notes.push(
            "Owned games are not visible from this profile or your key lacks access to that data."
                .to_string(),
        );
    }

    if !owned_games_visible {
        notes.push(
            "Steam-recorded Rust hours cannot be compared because the owned-games response is unavailable."
                .to_string(),
        );
    } else if battlemetrics_session_hours.is_none() {
        notes.push(format!(
            "No tracked BattleMetrics session hours were returned for this lookup ({battlemetrics_status})."
        ));
    }

    let vac_bans = ban_record.number_of_vac_bans.max(u32::from(ban_record.vac_banned));
    let game_bans = ban_record.number_of_game_bans;
    let total_bans = vac_bans + game_bans;

    let profile = ProfileSnapshot {
        steam_id: player.steamid,
        persona_name: player.personaname,
        profile_url: player.profileurl,
        avatar_url: player.avatarfull,
        community_visibility_state: player.communityvisibilitystate,
        ban_context: BanContext {
            vac_bans,
            game_bans,
            days_since_last_ban: (total_bans > 0).then_some(ban_record.days_since_last_ban),
            economy_ban: ban_record.economy_ban,
            community_banned: ban_record.community_banned,
        },
        playtime_context: PlaytimeContext {
            owned_games_visible,
            total_games: owned_games_visible.then_some(owned_games.game_count),
            rust_hours,
            crosshair_x_hours,
            total_hours,
            non_rust_hours,
            concentration_ratio,
            battlemetrics_session_hours,
            battlemetrics_status,
            battlemetrics_post_ban_hours,
            battlemetrics_session_history_status,
            battlemetrics_recent_session_count,
            steam_minus_session_hours,
            session_to_steam_ratio,
            notes,
            top_other_games,
        },
    };
    let decision = build_decision_tree(&profile);

    SteamLookupResult {
        profile,
        decision,
    }
}

struct BattleMetricsLookup {
    session_hours: Option<f32>,
    status: String,
    post_ban_hours: Option<f32>,
    session_history_status: String,
    recent_session_count: Option<u32>,
}

struct BattleMetricsSessionHistory {
    post_ban_hours: Option<f32>,
    status: String,
    session_count: Option<u32>,
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
    days_since_last_ban: Option<u32>,
) -> BattleMetricsLookup {
    let Some((token, server_ids)) = battlemetrics_config() else {
        return BattleMetricsLookup {
            session_hours: None,
            status: "missing config".to_string(),
            post_ban_hours: None,
            session_history_status: "missing config".to_string(),
            recent_session_count: None,
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
                post_ban_hours: None,
                session_history_status: "player lookup unavailable".to_string(),
                recent_session_count: None,
            };
        }
    };

    let payload = match response.json::<BattleMetricsPlayersResponse>() {
        Ok(payload) => payload,
        Err(error) => {
            return BattleMetricsLookup {
                session_hours: None,
                status: format!("parse failed: {error}"),
                post_ban_hours: None,
                session_history_status: "player lookup unavailable".to_string(),
                recent_session_count: None,
            };
        }
    };

    if payload.data.is_empty() {
        return BattleMetricsLookup {
            session_hours: None,
            status: "no matching player records".to_string(),
            post_ban_hours: None,
            session_history_status: "no matching player records".to_string(),
            recent_session_count: None,
        };
    }

    let player_ids = payload
        .data
        .iter()
        .map(|record| record.id.clone())
        .collect::<Vec<_>>();

    let total_seconds = payload
        .data
        .iter()
        .filter_map(|record| record.attributes.time_played)
        .sum::<f64>();

    if total_seconds <= 0.0 {
        return BattleMetricsLookup {
            session_hours: None,
            status: format!("matched {} records without timePlayed", payload.data.len()),
            post_ban_hours: None,
            session_history_status: "aggregate playtime unavailable".to_string(),
            recent_session_count: None,
        };
    }

    let session_history = fetch_post_ban_session_history(
        client,
        &token,
        &server_ids,
        &player_ids,
        days_since_last_ban,
    );

    BattleMetricsLookup {
        session_hours: Some((((total_seconds / 3600.0) * 10.0).round() / 10.0) as f32),
        status: format!("matched {} records", payload.data.len()),
        post_ban_hours: session_history.post_ban_hours,
        session_history_status: session_history.status,
        recent_session_count: session_history.session_count,
    }
}

fn fetch_post_ban_session_history(
    client: &Client,
    token: &str,
    server_ids: &[String],
    player_ids: &[String],
    days_since_last_ban: Option<u32>,
) -> BattleMetricsSessionHistory {
    let Some(days_since_last_ban) = days_since_last_ban else {
        return BattleMetricsSessionHistory {
            post_ban_hours: None,
            status: "not requested: no Steam ban history".to_string(),
            session_count: None,
        };
    };

    let now = DateTime::<Utc>::from(SystemTime::now());
    let cutoff = now - ChronoDuration::days(i64::from(days_since_last_ban));
    let mut seen_session_ids = HashSet::new();
    let mut post_ban_seconds = 0.0;
    let mut post_ban_sessions = 0_u32;
    let mut saw_timestamped_session = false;

    for player_id in player_ids {
        let mut next_url = Some(format!(
            "https://api.battlemetrics.com/players/{player_id}/relationships/sessions"
        ));
        let mut page_number = 0;

        while let Some(url) = next_url.take() {
            if page_number >= MAX_BATTLEMETRICS_SESSION_HISTORY_PAGES {
                return BattleMetricsSessionHistory {
                    post_ban_hours: None,
                    status: format!(
                        "partial: session history exceeded {} pages per matched player",
                        MAX_BATTLEMETRICS_SESSION_HISTORY_PAGES
                    ),
                    session_count: None,
                };
            }
            page_number += 1;

            let mut request = client
                .get(&url)
                .bearer_auth(token)
                .query(&[("page[size]", "100"), ("include", "server")]);
            for server_id in server_ids {
                request = request.query(&[("filter[servers]", server_id.as_str())]);
            }

            let response = match request.send().and_then(|response| response.error_for_status()) {
                Ok(response) => response,
                Err(error) => {
                    return BattleMetricsSessionHistory {
                        post_ban_hours: None,
                        status: format!("session-history request failed: {error}"),
                        session_count: None,
                    };
                }
            };
            let payload = match response.json::<BattleMetricsSessionsResponse>() {
                Ok(payload) => payload,
                Err(error) => {
                    return BattleMetricsSessionHistory {
                        post_ban_hours: None,
                        status: format!("session-history parse failed: {error}"),
                        session_count: None,
                    };
                }
            };

            for session in payload.data {
                let belongs_to_configured_server = session
                    .relationships
                    .server
                    .as_ref()
                    .and_then(|relationship| relationship.data.as_ref())
                    .is_some_and(|server| server_ids.iter().any(|server_id| server_id == &server.id));
                if !belongs_to_configured_server {
                    continue;
                }
                if !seen_session_ids.insert(session.id) {
                    continue;
                }
                let Some(start) = session
                    .attributes
                    .first_time
                    .as_deref()
                    .and_then(parse_battlemetrics_timestamp)
                else {
                    continue;
                };
                let end = session
                    .attributes
                    .last_time
                    .as_deref()
                    .and_then(parse_battlemetrics_timestamp)
                    .unwrap_or(now);
                if end <= cutoff {
                    continue;
                }

                let elapsed_seconds = (end - start).num_seconds().max(0) as f64;
                let recorded_seconds = session.attributes.time_played.unwrap_or(elapsed_seconds);
                if recorded_seconds <= 0.0 || elapsed_seconds <= 0.0 {
                    continue;
                }

                let post_ban_start = start.max(cutoff);
                let post_ban_elapsed_seconds = (end - post_ban_start).num_seconds().max(0) as f64;
                post_ban_seconds += recorded_seconds * (post_ban_elapsed_seconds / elapsed_seconds);
                post_ban_sessions += 1;
                saw_timestamped_session = true;
            }

            next_url = payload.links.next.map(|link| {
                if link.starts_with("http") {
                    link
                } else {
                    format!("https://api.battlemetrics.com{link}")
                }
            });
        }
    }

    if !saw_timestamped_session {
        return BattleMetricsSessionHistory {
            post_ban_hours: None,
            status: "no dated sessions were returned after the latest Steam ban".to_string(),
            session_count: Some(0),
        };
    }

    BattleMetricsSessionHistory {
        post_ban_hours: Some((((post_ban_seconds / 3600.0) * 10.0).round() / 10.0) as f32),
        status: format!("{} dated sessions after the latest Steam ban", post_ban_sessions),
        session_count: Some(post_ban_sessions),
    }
}

fn parse_battlemetrics_timestamp(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|timestamp| timestamp.with_timezone(&Utc))
}

fn build_decision_tree(profile: &ProfileSnapshot) -> DecisionTree {
    let bans = &profile.ban_context;
    let playtime = &profile.playtime_context;
    let mut branches = Vec::new();
    let days_since_last_ban = bans.days_since_last_ban.unwrap_or(0);

    if bans.vac_bans > 0 {
        let weight = decayed_ban_weight(VAC_BAN_REVIEW_WEIGHT, bans.vac_bans, days_since_last_ban);
        branches.push(DecisionBranch {
            key: "vac_bans".to_string(),
            label: "VAC ban history".to_string(),
            weight: weight as i32,
            status: "factual Steam record".to_string(),
            summary: format!(
                "Steam reports {} VAC ban(s); the latest reported ban was {} days ago.",
                bans.vac_bans, days_since_last_ban
            ),
        });
    }

    if bans.game_bans > 0 {
        let weight = decayed_ban_weight(GAME_BAN_REVIEW_WEIGHT, bans.game_bans, days_since_last_ban);
        branches.push(DecisionBranch {
            key: "game_bans".to_string(),
            label: "game ban history".to_string(),
            weight: weight as i32,
            status: "factual Steam record".to_string(),
            summary: format!(
                "Steam reports {} game ban(s); the latest reported ban was {} days ago.",
                bans.game_bans, days_since_last_ban
            ),
        });
    }

    let total_bans = bans.vac_bans + bans.game_bans;
    if total_bans >= 2 {
        branches.push(DecisionBranch {
            key: "repeated_ban_pattern".to_string(),
            label: "repeated ban pattern".to_string(),
            weight: (REPEATED_BAN_PATTERN_WEIGHT + ((total_bans - 2).min(4) as i32 * 5)).min(80),
            status: "factual Steam record".to_string(),
            summary: format!(
                "Steam reports {total_bans} total bans. Multiple bans are treated as a persistent review pattern, even when the latest ban is old."
            ),
        });
    }

    let mut has_tracked_server_history = false;
    let mut playtime_unavailable = false;
    if !playtime.owned_games_visible {
        playtime_unavailable = true;
        branches.push(DecisionBranch {
            key: "steam_library".to_string(),
            label: "Steam library unavailable".to_string(),
            weight: 0,
            status: "missing evidence".to_string(),
            summary: "Steam did not return a visible owned-games library.".to_string(),
        });
    } else if let Some(steam_hours) = playtime.rust_hours {
        if let Some(session_hours) = playtime.battlemetrics_session_hours {
            has_tracked_server_history = true;
            let gap_hours = playtime.steam_minus_session_hours.unwrap_or(0.0);
            let ratio = playtime.session_to_steam_ratio.unwrap_or(0.0);
            let needs_coverage_review = steam_hours >= 500.0 && ratio <= 0.45 && gap_hours >= 250.0;
            branches.push(DecisionBranch {
                key: "tracked_server_hours".to_string(),
                label: if needs_coverage_review {
                    "playtime coverage gap"
                } else {
                    "tracked-server history"
                }
                .to_string(),
                weight: if needs_coverage_review {
                    PLAYTIME_COVERAGE_REVIEW_WEIGHT as i32
                } else {
                    0
                },
                status: "tracked-server lower bound".to_string(),
                summary: format!(
                    "Steam reports {steam_hours:.1} Rust hours; configured BattleMetrics servers report {session_hours:.1} session hours."
                ),
            });

            if steam_hours >= 10_000.0
                && gap_hours >= 5_000.0
                && ratio <= 0.25
                && playtime.crosshair_x_hours.unwrap_or(0.0) >= 1_000.0
            {
                branches.push(DecisionBranch {
                    key: "crosshair_x_context".to_string(),
                    label: "crosshair-overlay inconsistency".to_string(),
                    weight: CROSSHAIR_X_CONTEXT_WEIGHT,
                    status: "supplemental contextual evidence".to_string(),
                    summary: format!(
                        "Steam records {:.1} hours in Crosshair X alongside very high Rust hours and sparse tracked-server time. Crosshair overlays have legitimate uses; this supplements the inconsistency review and does not prove cheating or account purchase.",
                        playtime.crosshair_x_hours.unwrap_or(0.0)
                    ),
                });
            }

            if let Some(post_ban_hours) = playtime.battlemetrics_post_ban_hours {
                if post_ban_hours >= 3000.0 {
                    branches.push(DecisionBranch {
                        key: "post_ban_tracked_history".to_string(),
                        label: "established post-ban tracked history".to_string(),
                        weight: -ESTABLISHED_TRACKED_HISTORY_CREDIT,
                        status: "counter-evidence".to_string(),
                        summary: format!(
                            "Dated BattleMetrics sessions report {post_ban_hours:.1} tracked hours after the latest Steam ban, which reduces the priority of historical ban context."
                        ),
                    });
                } else if post_ban_hours >= 1000.0 {
                    branches.push(DecisionBranch {
                        key: "post_ban_tracked_history".to_string(),
                        label: "substantial post-ban tracked history".to_string(),
                        weight: -SUBSTANTIAL_TRACKED_HISTORY_CREDIT,
                        status: "counter-evidence".to_string(),
                        summary: format!(
                            "Dated BattleMetrics sessions report {post_ban_hours:.1} tracked hours after the latest Steam ban, which moderately reduces the priority of historical ban context."
                        ),
                    });
                }
            }
        } else {
            playtime_unavailable = true;
            branches.push(DecisionBranch {
                key: "tracked_server_hours".to_string(),
                label: "unverified Steam hours".to_string(),
                weight: 0,
                status: "missing evidence".to_string(),
                summary: format!(
                    "Steam reports {steam_hours:.1} Rust hours, but no session hours were returned from the configured BattleMetrics servers."
                ),
            });
        }
    } else {
        playtime_unavailable = true;
        branches.push(DecisionBranch {
            key: "steam_rust_hours".to_string(),
            label: "no visible Rust hours".to_string(),
            weight: 0,
            status: "missing evidence".to_string(),
            summary: "Rust was not present in the visible Steam owned-games response.".to_string(),
        });
    }

    let review_priority = branches
        .iter()
        .map(|branch| branch.weight)
        .sum::<i32>()
        .clamp(0, 100) as u32;
    let has_coverage_gap = branches
        .iter()
        .any(|branch| branch.key == "tracked_server_hours" && branch.weight > 0);
    let outcome = if review_priority >= 75 {
        "manual account review"
    } else if review_priority >= 35 {
        "review supporting evidence"
    } else if has_coverage_gap {
        "review server coverage"
    } else if playtime_unavailable {
        "insufficient playtime evidence"
    } else {
        "context available"
    };
    let evidence_level = if has_tracked_server_history {
        "Steam record + tracked-server lower bound"
    } else {
        "Steam profile record"
    };
    let evidence_coverage = evidence_coverage(playtime, total_bans > 0);
    let summary = branches
        .iter()
        .map(|branch| branch.summary.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let next_step = if total_bans >= 2 {
        "Multiple Steam bans remain a persistent review pattern. Use dated server sessions and current behaviour for context, but do not treat the history alone as proof of current cheating."
    } else if playtime.battlemetrics_post_ban_hours.is_some() {
        "Dated BattleMetrics sessions provide tracked activity after the latest Steam ban. Treat that as meaningful counter-evidence while keeping the historical ban visible."
    } else if has_tracked_server_history
        && days_since_last_ban >= 1095
        && (bans.vac_bans > 0 || bans.game_bans > 0)
    {
        "Keep the historical ban visible, but do not infer current behaviour from it alone. No dated post-ban BattleMetrics sessions were available for this lookup."
    } else if bans.vac_bans > 0 || bans.game_bans > 0 {
        "Review the account history manually. Ban records are factual, but they do not establish current or Rust-specific behaviour."
    } else if has_coverage_gap {
        "Check whether the configured servers cover the player’s normal activity. A gap does not prove idle or fabricated Steam hours."
    } else if playtime_unavailable {
        "Do not infer real playtime or experience until tracked-server session data is available."
    } else {
        "Use tracked-session hours as confirmed activity on the configured servers only; they are not a global total."
    };

    DecisionTree {
        outcome: outcome.to_string(),
        review_priority,
        evidence_coverage,
        evidence_level: evidence_level.to_string(),
        summary,
        next_step: next_step.to_string(),
        branches,
    }
}

fn evidence_coverage(playtime: &PlaytimeContext, has_ban_history: bool) -> u32 {
    let possible_points = if has_ban_history { 100 } else { 80 };
    let mut observed_points = 30; // Steam account and ban record resolved.

    if playtime.owned_games_visible {
        observed_points += 20;
    }
    if playtime.battlemetrics_session_hours.is_some() {
        observed_points += 30;
    }
    if has_ban_history && playtime.battlemetrics_recent_session_count.is_some() {
        observed_points += 20;
    }

    observed_points * 100 / possible_points
}

fn decayed_ban_weight(base_weight: u32, ban_count: u32, days_since_last_ban: u32) -> u32 {
    let repeated_ban_weight = (base_weight + (ban_count.saturating_sub(1) * 5)).min(85);

    match days_since_last_ban {
        0..=180 => repeated_ban_weight,
        181..=365 => repeated_ban_weight * 2 / 3,
        366..=1094 => repeated_ban_weight / 3,
        _ => repeated_ban_weight / 8,
    }
}

#[cfg(test)]
mod decision_tree_tests {
    use super::*;

    #[test]
    fn old_bans_receive_a_small_fraction_of_the_recent_weight() {
        assert_eq!(decayed_ban_weight(VAC_BAN_REVIEW_WEIGHT, 1, 30), 65);
        assert_eq!(decayed_ban_weight(VAC_BAN_REVIEW_WEIGHT, 1, 1095), 8);
        assert_eq!(decayed_ban_weight(GAME_BAN_REVIEW_WEIGHT, 1, 1095), 3);
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
