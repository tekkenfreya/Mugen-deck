import { useDaemon } from "@/hooks/useDaemon";
import type { RunningGame } from "@/types";

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
    </div>
  );
}
