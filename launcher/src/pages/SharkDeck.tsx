import { useCallback, useEffect, useState } from "react";
import { useController } from "@/hooks/useController";
import { useTrainers } from "@/hooks/useTrainers";
import { TrainerCard } from "@/components/TrainerCard";
import { getCurrentGame } from "@/api/daemon";
import type { RunningGame } from "@/types";

interface SharkDeckProps {
  onBack: () => void;
}

export function SharkDeck({ onBack }: SharkDeckProps) {
  const [currentGame, setCurrentGame] = useState<RunningGame | null>(null);
  const [focusIndex, setFocusIndex] = useState(0);
  const { trainers, searching, error, search } = useTrainers();

  // Detect current game
  useEffect(() => {
    const poll = async () => {
      try {
        const resp = await getCurrentGame();
        if (resp.ok && resp.data) {
          setCurrentGame(resp.data);
        }
      } catch {
        // ignore
      }
    };
    poll();
    const id = setInterval(poll, 5000);
    return () => clearInterval(id);
  }, []);

  // Auto-search when game is detected
  useEffect(() => {
    if (currentGame) {
      search(currentGame.name);
    }
  }, [currentGame, search]);

  const handleSelect = useCallback((index: number) => {
    const trainer = trainers[index];
    if (!trainer) return;
    // In full implementation, this would trigger download + launch
    console.log("Selected trainer:", trainer.name);
  }, [trainers]);

  useController({
    onBack,
    onNavigate: (dir) => {
      if (dir === "up") setFocusIndex((p) => Math.max(0, p - 1));
      if (dir === "down") setFocusIndex((p) => Math.min(trainers.length - 1, p + 1));
    },
    onConfirm: () => handleSelect(focusIndex),
  });

  return (
    <div className="page">
      <div className="page-content">
        <button className="back-btn" onClick={onBack}>
          Back
        </button>
        <h2 className="page-title">SharkDeck</h2>

        {currentGame ? (
          <div className="sharkdeck-game">
            <span className="dim">Detected game:</span>
            <span className="sharkdeck-game-name">{currentGame.name}</span>
          </div>
        ) : (
          <p className="dim">No game running — launch a game to find trainers</p>
        )}

        {searching && <p className="dim">Searching for trainers...</p>}

        {error && <p className="text-danger">{error}</p>}

        {trainers.length > 0 && (
          <div className="trainer-list">
            {trainers.map((trainer, i) => (
              <TrainerCard
                key={`${trainer.name}-${trainer.version}`}
                trainer={trainer}
                focused={i === focusIndex}
                onSelect={() => handleSelect(i)}
              />
            ))}
          </div>
        )}

        {!searching && !error && trainers.length === 0 && currentGame && (
          <p className="dim">No trainers found for {currentGame.name}</p>
        )}
      </div>
    </div>
  );
}
