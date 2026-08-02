import "./App.css";
import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";

function App() {
  const [steamId, setSteamId] = useState("");
  const [lookupState, setLookupState] = useState("idle");
  const [lookupError, setLookupError] = useState("");
  const [profile, setProfile] = useState(null);

  async function minimizeOverlay() {
    await invoke("minimize_main_window");
  }

  async function startWindowDrag(event) {
    if (event.button !== 0) {
      return;
    }

    event.preventDefault();
    event.stopPropagation();
    await invoke("begin_window_drag");
  }

  async function handleLookup(event) {
    event.preventDefault();
    setLookupState("loading");
    setLookupError("");

    try {
      const result = await invoke("lookup_steam_profile", { steamId });
      setProfile(result);
      setLookupState("success");
    } catch (error) {
      setProfile(null);
      setLookupState("error");
      setLookupError(String(error));
    }
  }

  function formatHours(value) {
    if (value == null) {
      return "private";
    }
    return `${value.toFixed(1)}h`;
  }

  return (
    <main className="overlay-shell">
      <section className="panel">
        <header
          className="window-bar active"
          onMouseDown={(event) => {
            const interactiveTarget = event.target.closest("button");
            if (interactiveTarget) {
              return;
            }
            void startWindowDrag(event);
          }}
        >
          <div className="window-bar-copy">
            <span className="window-title">steam profile lookup</span>
            <span className="window-subtitle">drag window</span>
          </div>
          <div className="header-actions">
            <span className="status-pill">
              {lookupState === "loading" ? "LOOKUP" : "READY"}
            </span>
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
            <button className="lookup-button" type="submit" disabled={lookupState === "loading"}>
              {lookupState === "loading" ? "Checking" : "Check"}
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
                <img className="avatar-ring" src={profile.avatarUrl} alt="" />
                <div>
                  <p className="identity-name">{profile.personaName}</p>
                  <p className="identity-meta">{profile.steamId}</p>
                  <a className="profile-link" href={profile.profileUrl} target="_blank" rel="noreferrer">
                    open steam profile
                  </a>
                </div>
              </div>
            </section>

            <section className="flags">
              <div className="flag-row good">
                <span className="glyph">+</span>
                <span>Account lookup</span>
                <strong>valid Steam account</strong>
              </div>
              <div className={`flag-row ${profile.ownedGamesVisible ? "warn" : "critical"}`}>
                <span className="glyph">{profile.ownedGamesVisible ? "~" : "!"}</span>
                <span>Library visibility</span>
                <strong>{profile.ownedGamesVisible ? "public enough to read" : "private / unavailable"}</strong>
              </div>
              <div className="flag-row warn">
                <span className="glyph">~</span>
                <span>Rust hours</span>
                <strong>{formatHours(profile.rustHours)}</strong>
              </div>
              <div className="flag-row warn">
                <span className="glyph">~</span>
                <span>Other visible game hours</span>
                <strong>{formatHours(profile.nonRustHours)}</strong>
              </div>
              <div className={`flag-row ${profile.heuristicLevel === "warn" ? "critical" : "good"}`}>
                <span className="glyph">{profile.heuristicLevel === "warn" ? "!" : "+"}</span>
                <span>Library concentration</span>
                <strong>
                  {profile.concentrationRatio != null
                    ? `${Math.round(profile.concentrationRatio * 100)}% Rust`
                    : "unknown"}
                </strong>
              </div>
            </section>

            <section className="activity-card">
              <div className="card-header">
                <span>Playtime context</span>
                <span>{profile.totalGames != null ? `${profile.totalGames} games visible` : "visibility limited"}</span>
              </div>
              <p className="summary-copy">{profile.heuristicSummary}</p>
              {profile.topOtherGames.length ? (
                <div className="other-games-list">
                  {profile.topOtherGames.map((game) => (
                    <div className="other-game-row" key={game.appId}>
                      <span>{game.name}</span>
                      <strong>{formatHours(game.hours)}</strong>
                    </div>
                  ))}
                </div>
              ) : (
                <p className="subtle-copy">No other visible played games were returned.</p>
              )}
              {profile.notes.map((note) => (
                <p className="subtle-copy" key={note}>
                  {note}
                </p>
              ))}
            </section>
          </>
        ) : null}

        <footer className="scorebar">
          <div>
            <p className="score-label">Lookup state</p>
            <p className="score-value">{lookupState}</p>
          </div>
          <div>
            <p className="score-label">Steam account</p>
            <p className="score-value">{profile ? "resolved" : "waiting"}</p>
          </div>
        </footer>

        <p className="hotkey-hint">
          Alt+Shift+P hides or restores the overlay. Alt+Shift+M minimizes or restores it.
        </p>
      </section>
    </main>
  );
}

export default App;
