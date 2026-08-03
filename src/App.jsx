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
      return "private";
    }
    return `${value.toFixed(1)}h`;
  }

  function formatBanLastDays(value) {
    return value != null ? `${value}d` : "none";
  }

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
                    <span>Rust hours</span>
                    <strong>{formatHours(profile.profile.playtimeContext.rustHours)}</strong>
                  </div>
                  <div className="flag-row warn">
                    <span className="glyph">~</span>
                    <span>Other visible game hours</span>
                    <strong>{formatHours(profile.profile.playtimeContext.nonRustHours)}</strong>
                  </div>
                  <div
                    className={`flag-row ${
                      profile.scores.modules.playtime.score >= 45 ? "critical" : "good"
                    }`}
                  >
                    <span className="glyph">{profile.scores.modules.playtime.score >= 45 ? "!" : "+"}</span>
                    <span>Library concentration</span>
                    <strong>
                      {profile.profile.playtimeContext.concentrationRatio != null
                        ? `${Math.round(profile.profile.playtimeContext.concentrationRatio * 100)}% Rust`
                        : "unknown"}
                    </strong>
                  </div>
                </section>

                <section className="activity-card">
                  <div className="card-header">
                    <span>Playtime context</span>
                    <span>
                      {profile.profile.playtimeContext.totalGames != null
                        ? `${profile.profile.playtimeContext.totalGames} games visible`
                        : "visibility limited"}
                    </span>
                  </div>
                  <p className="summary-copy">{profile.scores.modules.playtime.summary}</p>
                  {profile.profile.banContext.economyBan &&
                  profile.profile.banContext.economyBan !== "none" ? (
                    <p className="subtle-copy">Economy ban: {profile.profile.banContext.economyBan}</p>
                  ) : null}
                  {profile.profile.banContext.communityBanned ? (
                    <p className="subtle-copy">Community ban present. Treat as conduct context, not cheating proof.</p>
                  ) : null}
                  {profile.profile.playtimeContext.topOtherGames.length ? (
                    <div className="other-games-list">
                      {profile.profile.playtimeContext.topOtherGames.map((game) => (
                        <div className="other-game-row" key={game.appId}>
                          <span>{game.name}</span>
                          <strong>{formatHours(game.hours)}</strong>
                        </div>
                      ))}
                    </div>
                  ) : (
                    <p className="subtle-copy">No other visible played games were returned.</p>
                  )}
                  {profile.profile.playtimeContext.notes.map((note) => (
                    <p className="subtle-copy" key={note}>
                      {note}
                    </p>
                  ))}
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
