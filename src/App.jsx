import "./App.css";
import { invoke } from "@tauri-apps/api/core";
import { useQuery } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";

const THEME_OPTIONS = [
  { value: "dark", label: "Dark" },
  { value: "light", label: "Light" },
  { value: "hud", label: "HUD" },
];

function App() {
  const [steamId, setSteamId] = useState("");
  const [activeSteamId, setActiveSteamId] = useState("");
  const [shutdownState, setShutdownState] = useState("idle");
  const [activeTab, setActiveTab] = useState("lookup");
  const [themeMode, setThemeMode] = useState("dark");
  const [overlayOpacity, setOverlayOpacity] = useState(0.97);
  const dragStateRef = useRef(null);

  useEffect(() => {
    function handleWindowMouseMove(event) {
      const dragState = dragStateRef.current;
      if (!dragState || (event.buttons & 1) !== 1) {
        return;
      }

      const deltaX = event.screenX - dragState.cursorX;
      const deltaY = event.screenY - dragState.cursorY;

      void invoke("set_window_position", {
        x: Math.round(dragState.windowX + deltaX),
        y: Math.round(dragState.windowY + deltaY),
      });
    }

    function handleWindowMouseUp() {
      dragStateRef.current = null;
    }

    window.addEventListener("mousemove", handleWindowMouseMove);
    window.addEventListener("mouseup", handleWindowMouseUp);
    window.addEventListener("blur", handleWindowMouseUp);

    return () => {
      window.removeEventListener("mousemove", handleWindowMouseMove);
      window.removeEventListener("mouseup", handleWindowMouseUp);
      window.removeEventListener("blur", handleWindowMouseUp);
    };
  }, []);

  async function minimizeOverlay() {
    await invoke("minimize_main_window");
  }

  async function shutdownApplication(event) {
    event.stopPropagation();
    setShutdownState("stopping");

    try {
      await invoke("shutdown_app");
    } catch {
      setShutdownState("error");
    }
  }

  async function titleBar(event) {
    if (event.button !== 0 || event.target.closest("button")) {
      return;
    }

    event.preventDefault();
    event.stopPropagation();

    try {
      const dragState = await invoke("get_window_drag_state");
      dragStateRef.current = {
        cursorX: dragState.cursorX,
        cursorY: dragState.cursorY,
        windowX: dragState.windowX,
        windowY: dragState.windowY,
      };
    } catch {
      dragStateRef.current = null;
    }
  }

  function endTitleBarDrag() {
    dragStateRef.current = null;
  }

  function handleLookup(event) {
    event.preventDefault();
    const normalizedSteamId = steamId.trim();
    if (!normalizedSteamId) {
      return;
    }

    setActiveSteamId(normalizedSteamId);
  }

  const {
    data: profile,
    error,
    isLoading,
    isFetching,
    isSuccess,
    status,
  } = useQuery({
    queryKey: ["steam-profile", activeSteamId],
    enabled: Boolean(activeSteamId),
    queryFn: () => invoke("lookup_steam_profile", { steamId: activeSteamId }),
  });

  const lookupState = !activeSteamId ? "idle" : status;
  const lookupError = error ? String(error) : "";

  function formatHours(value) {
    if (value == null) {
      return "unknown";
    }
    return `${value.toFixed(1)}h`;
  }

  function formatBanLastDays(value) {
    return value != null ? `${value}d` : "none";
  }

  function formatPercent(value) {
    if (value == null) {
      return "unknown";
    }
    return `${Math.round(value * 100)}%`;
  }

  const detailItems = profile
    ? [
        `Steam Rust hours: ${formatHours(profile.profile.playtimeContext.rustHours)}`,
        `Tracked session hours: ${formatHours(profile.profile.playtimeContext.battlemetricsSessionHours)}`,
        `BattleMetrics status: ${profile.profile.playtimeContext.battlemetricsStatus}`,
        `Steam minus session: ${formatHours(profile.profile.playtimeContext.steamMinusSessionHours)}`,
        `Session / Steam ratio: ${formatPercent(profile.profile.playtimeContext.sessionToSteamRatio)}`,
        `Authenticity confidence: ${profile.profile.playtimeContext.authenticityConfidence}`,
        profile.profile.playtimeContext.totalGames != null
          ? `${profile.profile.playtimeContext.totalGames} Steam games visible`
          : "Steam library visibility limited",
        profile.profile.playtimeContext.battlemetricsSessionHours == null
          ? `BattleMetrics session-hour fetch did not return tracked hours for this lookup (${profile.profile.playtimeContext.battlemetricsStatus}).`
          : "Tracked session hours are available for comparison against Steam Rust hours.",
        ...(profile.profile.banContext.economyBan &&
        profile.profile.banContext.economyBan !== "none"
          ? [`Economy ban: ${profile.profile.banContext.economyBan}`]
          : []),
        ...(profile.profile.banContext.communityBanned
          ? ["Community ban present. Treat as conduct context, not cheating proof."]
          : []),
        ...(profile.profile.playtimeContext.topOtherGames.length
          ? profile.profile.playtimeContext.topOtherGames.map(
              (game) => `Other visible Steam playtime: ${game.name} ${formatHours(game.hours)}`,
            )
          : ["No other visible played games were returned."]),
        ...profile.profile.playtimeContext.notes,
      ]
    : [];
  const rawOutputs = profile
    ? {
        bans: {
          vacBans: profile.profile.banContext.vacBans,
          gameBans: profile.profile.banContext.gameBans,
          daysSinceLastBan: profile.profile.banContext.daysSinceLastBan,
          economyBan: profile.profile.banContext.economyBan,
          communityBanned: profile.profile.banContext.communityBanned,
          score: profile.scores.modules.bans.score,
          label: profile.scores.modules.bans.label,
        },
        playtime: {
          ownedGamesVisible: profile.profile.playtimeContext.ownedGamesVisible,
          totalGames: profile.profile.playtimeContext.totalGames,
          rustHours: profile.profile.playtimeContext.rustHours,
          totalHours: profile.profile.playtimeContext.totalHours,
          nonRustHours: profile.profile.playtimeContext.nonRustHours,
          concentrationRatio: profile.profile.playtimeContext.concentrationRatio,
          battlemetricsSessionHours: profile.profile.playtimeContext.battlemetricsSessionHours,
          battlemetricsStatus: profile.profile.playtimeContext.battlemetricsStatus,
          steamMinusSessionHours: profile.profile.playtimeContext.steamMinusSessionHours,
          sessionToSteamRatio: profile.profile.playtimeContext.sessionToSteamRatio,
          authenticityConfidence: profile.profile.playtimeContext.authenticityConfidence,
          score: profile.scores.modules.playtime.score,
          label: profile.scores.modules.playtime.label,
        },
        overall: {
          score: profile.scores.overall.score,
          label: profile.scores.overall.label,
        },
      }
    : null;

  return (
    <main
      className="overlay-shell"
      data-theme={themeMode}
      style={{ "--overlay-opacity": overlayOpacity }}
    >
      <section className="panel">
        <header
          className="window-bar active"
          onMouseDown={(event) => {
            void titleBar(event);
          }}
          onMouseUp={endTitleBarDrag}
        >
          <div className="window-bar-copy">
            <span className="window-title">steam profile lookup</span>
            <span className="window-subtitle">overlay</span>
          </div>
          <div className="header-actions">
            <span className="status-pill">
              {isFetching ? "LOOKUP" : "READY"}
            </span>
            <button
              type="button"
              className={`header-toggle${activeTab === "details" ? " active" : ""}`}
              aria-label={activeTab === "details" ? "Return to lookup" : "Open details"}
              title={activeTab === "details" ? "back to lookup" : "open details"}
              onMouseDown={(event) => event.stopPropagation()}
              onClick={(event) => {
                event.stopPropagation();
                setActiveTab((current) => (current === "details" ? "lookup" : "details"));
              }}
              disabled={!profile}
            >
              ≣
            </button>
            <button
              type="button"
              className={`header-toggle${activeTab === "settings" ? " active" : ""}`}
              aria-label={activeTab === "settings" ? "Return to lookup" : "Open settings"}
              title={activeTab === "settings" ? "back to lookup" : "open settings"}
              onMouseDown={(event) => event.stopPropagation()}
              onClick={(event) => {
                event.stopPropagation();
                setActiveTab((current) => (current === "settings" ? "lookup" : "settings"));
              }}
            >
              {activeTab === "settings" ? "←" : "⚙"}
            </button>
            <button
              type="button"
              className="header-action"
              aria-label="Shut down Sus Check"
              title="power off"
              onMouseDown={(event) => event.stopPropagation()}
              onClick={(event) => {
                void shutdownApplication(event);
              }}
            >
              {shutdownState === "stopping"
                ? "..."
                : shutdownState === "error"
                  ? "ERR"
                  : "⏻"}
            </button>
            <button
              type="button"
              className="header-minimize"
              aria-label="Minimize overlay"
              title="alt shift m"
              onMouseDown={(event) => event.stopPropagation()}
              onClick={(event) => {
                event.stopPropagation();
                void minimizeOverlay();
              }}
            >
              -
            </button>
          </div>
        </header>

        <header className="panel-header">
          <div>
            <p className="eyebrow">manual steam identity check</p>
            <h1>SUS CHECK</h1>
          </div>
        </header>

        {activeTab === "lookup" ? (
          <>
            <section className="lookup-card">
              <p className="identity-label">SteamID64</p>
              <form className="lookup-form" onSubmit={handleLookup}>
                <input
                  className="lookup-input"
                  value={steamId}
                  onChange={(event) => setSteamId(event.currentTarget.value)}
                  placeholder="7656119..."
                  spellCheck={false}
                />
                <button className="lookup-button" type="submit" disabled={isLoading || isFetching}>
                  {isLoading || isFetching ? "Checking" : "Check"}
                </button>
              </form>
              <p className="lookup-help">
                First pass accepts 17-digit SteamID64 input only. API key required: `STEAM_WEB_API_KEY`.
              </p>
              {lookupError ? <p className="error-copy">{lookupError}</p> : null}
            </section>

            {profile ? (
              <>
                <section className="identity-card">
                  <p className="identity-label">Resolved account</p>
                  <div className="identity-body">
                    <img className="avatar-ring" src={profile.profile.avatarUrl} alt="" />
                    <div>
                      <p className="identity-name">{profile.profile.personaName}</p>
                      <p className="identity-meta">{profile.profile.steamId}</p>
                      <a className="profile-link" href={profile.profile.profileUrl} target="_blank" rel="noreferrer">
                        open steam profile
                      </a>
                    </div>
                  </div>
                </section>

                <section className="flags">
                  <div
                    className={`flag-row ${
                      profile.scores.modules.bans.label === "high risk"
                        ? "critical"
                        : profile.scores.modules.bans.label === "clean"
                          ? "good"
                          : "warn"
                    }`}
                  >
                    <span className="glyph">
                      {profile.scores.modules.bans.label === "high risk"
                        ? "!"
                        : profile.scores.modules.bans.label === "clean"
                          ? "+"
                          : "~"}
                    </span>
                    <span>Ban risk</span>
                    <strong>
                      {profile.scores.modules.bans.label} | VAC {profile.profile.banContext.vacBans} | Game{" "}
                      {profile.profile.banContext.gameBans} | Last{" "}
                      {formatBanLastDays(profile.profile.banContext.daysSinceLastBan)}
                    </strong>
                  </div>
                  <div className="flag-row good">
                    <span className="glyph">+</span>
                    <span>Account lookup</span>
                    <strong>valid Steam account</strong>
                  </div>
                  <div className={`flag-row ${profile.profile.playtimeContext.ownedGamesVisible ? "warn" : "critical"}`}>
                    <span className="glyph">{profile.profile.playtimeContext.ownedGamesVisible ? "~" : "!"}</span>
                    <span>Library visibility</span>
                    <strong>
                      {profile.profile.playtimeContext.ownedGamesVisible
                        ? "public enough to read"
                        : "private / unavailable"}
                    </strong>
                  </div>
                  <div className="flag-row warn">
                    <span className="glyph">~</span>
                    <span>Steam Rust hours</span>
                    <strong>{formatHours(profile.profile.playtimeContext.rustHours)}</strong>
                  </div>
                  <div className="flag-row warn">
                    <span className="glyph">~</span>
                    <span>Tracked session hours</span>
                    <strong>{formatHours(profile.profile.playtimeContext.battlemetricsSessionHours)}</strong>
                  </div>
                  <div
                    className={`flag-row ${
                      profile.scores.modules.playtime.score >= 45 ? "critical" : "good"
                    }`}
                  >
                    <span className="glyph">{profile.scores.modules.playtime.score >= 45 ? "!" : "+"}</span>
                    <span>Session / Steam ratio</span>
                    <strong>
                      {formatPercent(profile.profile.playtimeContext.sessionToSteamRatio)}
                    </strong>
                  </div>
                </section>

                <section className="activity-card">
                  <div className="card-header">
                    <span>Playtime authenticity</span>
                    <span>
                      confidence {profile.profile.playtimeContext.authenticityConfidence}
                    </span>
                  </div>
                  <p className="summary-copy">{profile.scores.modules.playtime.summary}</p>
                  <div className="metric-grid">
                    <div className="metric-tile">
                      <span className="metric-label">Steam Rust</span>
                      <strong>{formatHours(profile.profile.playtimeContext.rustHours)}</strong>
                    </div>
                    <div className="metric-tile">
                      <span className="metric-label">Tracked sessions</span>
                      <strong>{formatHours(profile.profile.playtimeContext.battlemetricsSessionHours)}</strong>
                    </div>
                    <div className="metric-tile">
                      <span className="metric-label">Steam minus session</span>
                      <strong>{formatHours(profile.profile.playtimeContext.steamMinusSessionHours)}</strong>
                    </div>
                    <div className="metric-tile">
                      <span className="metric-label">Session / Steam</span>
                      <strong>{formatPercent(profile.profile.playtimeContext.sessionToSteamRatio)}</strong>
                    </div>
                  </div>
                  <p className="subtle-copy">Open details for supporting notes, extra playtime context, and the visible game list.</p>
                </section>
              </>
            ) : null}

            <footer className="scorebar">
              <div>
                <p className="score-label">Overall score</p>
                <p className="score-value">{isSuccess ? `${profile.scores.overall.score}` : "--"}</p>
              </div>
              <div>
                <p className="score-label">Overall label</p>
                <p className="score-value">{isSuccess ? profile.scores.overall.label : "waiting"}</p>
              </div>
            </footer>

            <p className="hotkey-hint">
              Alt+Shift+P hides or restores the overlay. Alt+Shift+M minimizes or restores it.
            </p>
          </>
        ) : activeTab === "details" ? (
          <section className="details-panel">
            <div className="card-header">
              <span>Detail notes</span>
              <span>{profile ? `${detailItems.length} bullets` : "no profile"}</span>
            </div>
            {profile ? (
              <div className="details-scroll">
                <ul className="details-list">
                  {detailItems.map((item) => (
                    <li className="details-item" key={item}>
                      {item}
                    </li>
                  ))}
                </ul>
                <div className="raw-output-card">
                  <div className="other-games-header">
                    <span>Raw outputs</span>
                    <span>live payload</span>
                  </div>
                  <pre className="raw-output-block">
                    {JSON.stringify(rawOutputs, null, 2)}
                  </pre>
                </div>
              </div>
            ) : (
              <p className="subtle-copy">Run a lookup first to populate details.</p>
            )}
          </section>
        ) : (
          <section className="settings-panel">
            <div className="display-controls settings-grid">
              <div className="control-group">
                <label className="control-label" htmlFor="theme-mode">
                  Theme
                </label>
                <select
                  id="theme-mode"
                  className="control-select"
                  value={themeMode}
                  onChange={(event) => setThemeMode(event.currentTarget.value)}
                >
                  {THEME_OPTIONS.map((option) => (
                    <option key={option.value} value={option.value}>
                      {option.label}
                    </option>
                  ))}
                </select>
              </div>
              <div className="control-group control-group-wide">
                <label className="control-label" htmlFor="overlay-opacity">
                  Opacity
                </label>
                <div className="opacity-row">
                  <input
                    id="overlay-opacity"
                    className="opacity-slider"
                    type="range"
                    min="0.55"
                    max="1"
                    step="0.01"
                    value={overlayOpacity}
                    onChange={(event) =>
                      setOverlayOpacity(Number(event.currentTarget.value))
                    }
                  />
                  <span className="opacity-value">
                    {Math.round(overlayOpacity * 100)}%
                  </span>
                </div>
              </div>
            </div>
            <p className="lookup-help">
              Appearance settings are local overlay controls. The palette system is token-based so more HUD themes can
              be added cleanly later.
            </p>
          </section>
        )}
      </section>
    </main>
  );
}

export default App;
