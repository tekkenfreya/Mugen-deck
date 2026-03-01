import { useCallback, useState } from "react";

/** Trainer info as displayed in the launcher UI. */
export interface TrainerDisplayInfo {
  name: string;
  gameName: string;
  version: string;
  downloadUrl: string;
  source: string;
}

interface TrainersState {
  trainers: TrainerDisplayInfo[];
  searching: boolean;
  error: string | null;
}

interface UseTrainersReturn extends TrainersState {
  search: (gameName: string) => Promise<void>;
  clear: () => void;
}

/**
 * Manages trainer search state.
 *
 * In Phase 1, this is a placeholder that would communicate with the SharkDeck app
 * via the daemon's app communication API. For now, it demonstrates the UI flow.
 */
export function useTrainers(): UseTrainersReturn {
  const [state, setState] = useState<TrainersState>({
    trainers: [],
    searching: false,
    error: null,
  });

  const search = useCallback(async (gameName: string) => {
    setState({ trainers: [], searching: true, error: null });

    try {
      // In production, this would call the daemon which forwards to SharkDeck
      // For now, stub with a timeout to show loading state
      const resp = await fetch(
        `http://127.0.0.1:7331/apps/sharkdeck/action/search?game=${encodeURIComponent(gameName)}`,
        {
          method: "POST",
          headers: { "Content-Type": "application/json" },
        },
      );

      if (!resp.ok) {
        setState({
          trainers: [],
          searching: false,
          error: "Trainer search not available yet",
        });
        return;
      }

      const data = (await resp.json()) as {
        ok: boolean;
        data?: { trainers: TrainerDisplayInfo[] };
        error?: string;
      };

      if (data.ok && data.data) {
        setState({
          trainers: data.data.trainers,
          searching: false,
          error: null,
        });
      } else {
        setState({
          trainers: [],
          searching: false,
          error: data.error ?? "search failed",
        });
      }
    } catch {
      setState({
        trainers: [],
        searching: false,
        error: "Cannot reach daemon",
      });
    }
  }, []);

  const clear = useCallback(() => {
    setState({ trainers: [], searching: false, error: null });
  }, []);

  return { ...state, search, clear };
}
