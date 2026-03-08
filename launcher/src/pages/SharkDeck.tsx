import { useCallback, useEffect, useRef, useState } from "react";
import { useTrainers } from "@/hooks/useTrainers";
import { TrainerCard } from "@/components/TrainerCard";

import {
  getCurrentGame,
  getGameLibrary,
  enableTrainer,
  disableTrainer,
  getEnabledTrainer,
  getSharkDeckStatus,
  cancelSharkDeck,
} from "@/api/daemon";
import type { EnableResult, RunningGame, SharkDeckStatus, SteamGame, TrainerConfig } from "@/types";

const STORAGE_KEY = "sharkdeck_state";
const LAUNCH_OPTIONS = "/home/deck/.local/share/sharkdeck/trainer-hook.sh %command%";
const SPLASH_DURATION = 4000;

interface PersistedState {
  currentGame: RunningGame | null;
  manualQuery: string;
}

function loadPersistedState(): PersistedState | null {
  try {
    const raw = sessionStorage.getItem(STORAGE_KEY);
    if (!raw) return null;
    return JSON.parse(raw) as PersistedState;
  } catch {
    return null;
  }
}

function savePersistedState(state: PersistedState): void {
  try {
    sessionStorage.setItem(STORAGE_KEY, JSON.stringify(state));
  } catch {
    // non-critical
  }
}

export function SharkDeck() {
  const persisted = useRef(loadPersistedState());
  const searchInputRef = useRef<HTMLInputElement>(null);

  // Splash screen state
  const [splashDone, setSplashDone] = useState(false);
  const [splashFading, setSplashFading] = useState(false);

  const [currentGame, setCurrentGame] = useState<RunningGame | null>(
    persisted.current?.currentGame ?? null,
  );
  const [manualQuery, setManualQuery] = useState(
    persisted.current?.manualQuery ?? "",
  );
  const [trainerStatus, setTrainerStatus] = useState<SharkDeckStatus>("idle");
  const [statusError, setStatusError] = useState<string | null>(null);
  const [statusProgress, setStatusProgress] = useState<string | null>(null);
  const [hasSearched, setHasSearched] = useState(false);

  const [enabledTrainer, setEnabledTrainer] = useState<TrainerConfig | null>(null);
  const [enableResult, setEnableResult] = useState<EnableResult | null>(null);
  const [copied, setCopied] = useState(false);
  const [matchedAppId, setMatchedAppId] = useState<string>("0");

  const { trainers, searching, error, search, cancelSearch } = useTrainers();
  const searchedRef = useRef(false);

  const isBusy =
    trainerStatus === "installing_deps" ||
    trainerStatus === "downloading";

  // Splash timer
  useEffect(() => {
    const fadeTimer = setTimeout(() => setSplashFading(true), SPLASH_DURATION - 600);
    const doneTimer = setTimeout(() => setSplashDone(true), SPLASH_DURATION);
    return () => {
      clearTimeout(fadeTimer);
      clearTimeout(doneTimer);
    };
  }, []);

  // On mount: check daemon status
  useEffect(() => {
    const sync = async () => {
      try {
        const statusResp = await getSharkDeckStatus();
        if (statusResp.ok && statusResp.data) {
          setTrainerStatus(statusResp.data.status);
          if (statusResp.data.error) {
            setStatusError(statusResp.data.error);
          }
          setStatusProgress(statusResp.data.progress ?? null);
        }
      } catch {
        // stay idle
      }
    };
    sync();
  }, []);

  // Persist search context
  useEffect(() => {
    savePersistedState({ currentGame, manualQuery });
  }, [currentGame, manualQuery]);

  // Detect current game + check for enabled trainer
  useEffect(() => {
    const poll = async () => {
      try {
        const resp = await getCurrentGame();
        if (resp.ok && resp.data) {
          setCurrentGame(resp.data);
          setMatchedAppId(resp.data.app_id);
          if (!searchedRef.current) {
            searchedRef.current = true;
            setManualQuery(resp.data.name);
            setHasSearched(true);
            search(resp.data.name);
          }
          const enabledResp = await getEnabledTrainer(resp.data.app_id);
          if (enabledResp.ok && enabledResp.data) {
            setEnabledTrainer(enabledResp.data);
          }
        }
      } catch {
        // ignore
      }
    };
    poll();
    const id = setInterval(poll, 5000);
    return () => clearInterval(id);
  }, [search]);

  // Poll status while busy
  useEffect(() => {
    if (!isBusy) return;

    const poll = async () => {
      try {
        const resp = await getSharkDeckStatus();
        if (resp.ok && resp.data) {
          setTrainerStatus(resp.data.status);
          setStatusProgress(resp.data.progress ?? null);
          if (resp.data.error) {
            setStatusError(resp.data.error);
          }
          if (resp.data.status === "idle" && !resp.data.error) {
            const checkAppId = currentGame?.app_id ?? matchedAppId;
            const enabledResp = await getEnabledTrainer(checkAppId);
            if (enabledResp.ok && enabledResp.data) {
              setEnabledTrainer(enabledResp.data);
              setEnableResult({
                trainer_path: enabledResp.data.path,
                launch_options: LAUNCH_OPTIONS,
                needs_restart: !!currentGame,
              });
            }
          }
        }
      } catch {
        // ignore
      }
    };
    const intervalId = setInterval(poll, 1000);
    return () => clearInterval(intervalId);
  }, [trainerStatus, currentGame]);

  const handleSearch = useCallback(async () => {
    const query = manualQuery.trim();
    if (query.length === 0) return;

    searchInputRef.current?.blur();
    setHasSearched(true);
    search(query);

    try {
      const libResp = await getGameLibrary();
      if (libResp.ok && libResp.data) {
        const lower = query.toLowerCase();
        // Score-based matching: exact > starts with > shorter name > includes
        let bestMatch: SteamGame | null = null;
        let bestScore = -1;
        for (const g of libResp.data) {
          const name = g.name.toLowerCase();
          if (!name.includes(lower) && !lower.includes(name)) continue;
          let score = 0;
          if (name === lower) {
            score = 100; // exact match
          } else if (name.startsWith(lower)) {
            score = 80;
          } else if (lower.startsWith(name)) {
            score = 70;
          } else if (name.includes(lower)) {
            score = 50;
          } else {
            score = 30; // query includes game name
          }
          // Prefer shorter names (more specific match)
          score -= name.length * 0.1;
          if (score > bestScore) {
            bestScore = score;
            bestMatch = g;
          }
        }
        if (bestMatch) {
          setMatchedAppId(bestMatch.app_id);
        }
      }
    } catch {
      // ignore
    }
  }, [manualQuery, search]);

  const handleEnable = useCallback(
    async (index: number) => {
      const trainer = trainers[index];
      if (!trainer) return;

      const appId = currentGame?.app_id ?? matchedAppId;
      setStatusError(null);
      setTrainerStatus("downloading");
      setEnableResult(null);

      try {
        const resp = await enableTrainer(trainer, appId);
        if (!resp.ok) {
          setTrainerStatus("error");
          setStatusError(resp.error ?? "ENABLE FAILED");
        }
      } catch {
        setTrainerStatus("error");
        setStatusError("CANNOT REACH DAEMON");
      }
    },
    [trainers, currentGame],
  );

  const handleDisable = useCallback(async () => {
    const appId = currentGame?.app_id ?? matchedAppId;
    try {
      const resp = await disableTrainer(appId);
      if (resp.ok) {
        setEnabledTrainer(null);
        setEnableResult(null);
      }
    } catch {
      // ignore
    }
  }, [currentGame]);

  const handleCopy = useCallback(() => {
    navigator.clipboard
      .writeText(LAUNCH_OPTIONS)
      .then(() => {
        setCopied(true);
        setTimeout(() => setCopied(false), 2000);
      })
      .catch(() => {
        // ignore
      });
  }, []);

  const handleCancel = useCallback(async () => {
    // Cancel search if active
    if (searching) {
      cancelSearch();
      return;
    }
    // Cancel download/install if active
    if (isBusy) {
      try {
        await cancelSharkDeck();
      } catch {
        // ignore
      }
      setTrainerStatus("idle");
      setStatusError(null);
      setStatusProgress(null);
    }
  }, [searching, isBusy, cancelSearch]);

  const handleExit = useCallback(() => {
    window.close();
  }, []);

  // ─── Splash Screen ──────────────────────────────────────────────
  if (!splashDone) {
    return (
      <div className={`splash ${splashFading ? "splash--fade" : ""}`}>
        <div className="splash-glow" />
        <img
          src="./sharkdeck-logo.png"
          alt="SharkDeck"
          className="splash-logo"
        />
        <div className="splash-tagline">&#x25B2; &#x25B2; &#x25BC; &#x25BC; &#x25C0; &#x25B6; &#x25C0; &#x25B6; B A START</div>
      </div>
    );
  }

  // ─── Main App ────────────────────────────────────────────────────
  return (
    <div className="sd-app">
      <div className="sd-topbar">
        <div className="sd-topbar-logo-wrap">
          <img src="./sharkdeck-logo.png" alt="SharkDeck" className="sd-topbar-logo" />
        </div>
        {currentGame && (
          <div className="sd-target">
            <span className="sd-target-label">TARGET</span>
            <span className="sd-target-name">{currentGame.name.toUpperCase()}</span>
          </div>
        )}
      </div>

      <div className="sd-content">
        {/* Enabled trainer panel */}
        {enabledTrainer && (
          <div className="sd-enabled">
            <div className="sd-enabled-header">
              <span className="sd-badge">ACTIVE</span>
              <span className="sd-enabled-name">{enabledTrainer.name}</span>
            </div>

            {enableResult && (
              <div className="sd-launch-info">
                <p className="sd-dim">
                  {enableResult.needs_restart
                    ? "RESTART THE GAME FOR THE TRAINER TO TAKE EFFECT."
                    : "SET THIS AS YOUR GAME'S LAUNCH OPTIONS IN STEAM:"}
                </p>
                <div className="sd-launch-box">
                  <code>{LAUNCH_OPTIONS}</code>
                  <button className="sd-copy-btn" onClick={handleCopy}>
                    {copied ? "COPIED!" : "COPY"}
                  </button>
                </div>
                <p className="sd-dim sd-small">
                  STEAM &gt; RIGHT-CLICK GAME &gt; PROPERTIES &gt; LAUNCH OPTIONS &gt; PASTE
                </p>
              </div>
            )}

            <div className="sd-actions">
              <button className="sd-btn sd-btn--primary" onClick={handleExit}>
                EXIT
              </button>
              <button className="sd-btn sd-btn--ghost" onClick={handleDisable}>
                DISABLE TRAINER
              </button>
            </div>
          </div>
        )}

        {/* Search bar */}
        {!enabledTrainer && (
          <div className="sd-search">
            <div className="sd-search-field">
              <svg className="sd-search-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <circle cx="11" cy="11" r="8" />
                <path d="m21 21-4.35-4.35" />
              </svg>
              <input
                ref={searchInputRef}
                className="sd-search-input"
                type="text"
                placeholder="SEARCH GAME..."
                value={manualQuery}
                onChange={(e) => setManualQuery(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") handleSearch();
                }}
              />
            </div>
            {searching ? (
              <button className="sd-scan-btn sd-scan-btn--cancel" onClick={handleCancel}>
                CANCEL
              </button>
            ) : (
              <button
                className="sd-scan-btn"
                onClick={handleSearch}
                disabled={manualQuery.trim().length === 0}
              >
                SCAN
              </button>
            )}
          </div>
        )}

        {searching && (
          <div className="sd-scanning">
            <div className="sd-scanning-icon">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" width="28" height="28">
                <circle cx="11" cy="11" r="8" strokeDasharray="50" strokeDashoffset="0">
                  <animateTransform attributeName="transform" type="rotate" from="0 11 11" to="360 11 11" dur="1s" repeatCount="indefinite" />
                </circle>
                <path d="m21 21-4.35-4.35" />
              </svg>
            </div>
            <p className="sd-scanning-text">SCANNING TRAINER DATABASES...</p>
            <p className="sd-dim sd-small">SEARCHING FLING + GAMECOPYWORLD</p>
            <div className="sd-progress" style={{ maxWidth: "300px" }}>
              <div className="sd-progress-bar" />
            </div>
          </div>
        )}

        {error && <p className="sd-error">{error}</p>}
        {statusError && <p className="sd-error">{statusError}</p>}

        {isBusy && (
          <div className="sd-busy">
            <p className="sd-dim">
              {trainerStatus === "installing_deps" && "INSTALLING .NET RUNTIME (ONE-TIME, ~5 MIN)..."}
              {trainerStatus === "downloading" && (statusProgress ?? "DOWNLOADING TRAINER DATA...")}
            </p>
            <div className="sd-progress">
              <div className="sd-progress-bar" />
            </div>
            <button className="sd-btn sd-btn--ghost sd-cancel-btn" onClick={handleCancel}>
              CANCEL
            </button>
          </div>
        )}

        {!searching && !isBusy && !enabledTrainer && trainers.length > 0 && (
          <div className="sd-trainer-list">
            {trainers.map((trainer, i) => (
              <TrainerCard
                key={`${trainer.name}-${trainer.version}`}
                trainer={trainer}
                focused={false}
                status={trainerStatus}
                onSelect={() => handleEnable(i)}
              />
            ))}
          </div>
        )}

        {!searching && !error && trainers.length === 0 && hasSearched && !enabledTrainer && (
          <div className="sd-empty">
            <p>NO TRAINERS FOUND</p>
            <p className="sd-dim sd-small">TRY A DIFFERENT SEARCH TERM</p>
          </div>
        )}
      </div>
    </div>
  );
}
