import { useDaemon } from "@/hooks/useDaemon";
import type { RunningGame } from "@/types";

declare global {
  interface Window {
    mugen?: {
      quit: () => void;
    };
  }
}

interface StatusBarProps {
  currentGame: RunningGame | null;
}

export function StatusBar({ currentGame }: StatusBarProps) {
  const { connected, health } = useDaemon();

  return (
    <div className="status-bar">
      <div className={`daemon-status ${connected ? "connected" : "disconnected"}`}>
        <span className="dot" />
        {connected ? `Daemon v${health?.version}` : "Disconnected"}
      </div>
      {currentGame && (
        <div className="current-game">
          Playing: {currentGame.name}
        </div>
      )}
      {window.mugen && (
        <button className="exit-btn" onClick={() => window.mugen?.quit()}>
          Exit
        </button>
      )}
    </div>
  );
}
