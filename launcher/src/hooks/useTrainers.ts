import { useCallback, useRef, useState } from "react";
import { searchTrainers } from "@/api/daemon";
import type { TrainerInfo } from "@/types";

const SEARCH_TIMEOUT_MS = 30_000;

interface TrainersState {
  trainers: TrainerInfo[];
  searching: boolean;
  error: string | null;
}

interface UseTrainersReturn extends TrainersState {
  search: (gameName: string) => Promise<void>;
  cancelSearch: () => void;
  clear: () => void;
}

/** Manages trainer search state via the daemon SharkDeck API. */
export function useTrainers(): UseTrainersReturn {
  const [state, setState] = useState<TrainersState>({
    trainers: [],
    searching: false,
    error: null,
  });

  const abortRef = useRef<AbortController | null>(null);

  const cancelSearch = useCallback(() => {
    if (abortRef.current) {
      abortRef.current.abort();
      abortRef.current = null;
    }
    setState((prev) => ({
      ...prev,
      searching: false,
      error: prev.searching ? "Search cancelled" : prev.error,
    }));
  }, []);

  const search = useCallback(async (gameName: string) => {
    // Cancel any in-flight search
    if (abortRef.current) {
      abortRef.current.abort();
    }

    setState({ trainers: [], searching: true, error: null });

    const controller = new AbortController();
    abortRef.current = controller;

    // Timeout: auto-abort after 30s
    const timeout = setTimeout(() => controller.abort(), SEARCH_TIMEOUT_MS);

    try {
      const resp = await Promise.race([
        searchTrainers(gameName),
        new Promise<never>((_, reject) => {
          controller.signal.addEventListener("abort", () =>
            reject(new DOMException("Search timed out", "AbortError")),
          );
        }),
      ]);

      if (controller.signal.aborted) return;

      if (resp.ok && resp.data) {
        setState({
          trainers: resp.data.trainers,
          searching: false,
          error: null,
        });
      } else {
        setState({
          trainers: [],
          searching: false,
          error: resp.error ?? "search failed",
        });
      }
    } catch (e) {
      if (controller.signal.aborted) {
        setState({
          trainers: [],
          searching: false,
          error: "Search timed out — try a different name",
        });
        return;
      }
      setState({
        trainers: [],
        searching: false,
        error: "Cannot reach daemon",
      });
    } finally {
      clearTimeout(timeout);
      if (abortRef.current === controller) {
        abortRef.current = null;
      }
    }
  }, []);

  const clear = useCallback(() => {
    if (abortRef.current) {
      abortRef.current.abort();
      abortRef.current = null;
    }
    setState({ trainers: [], searching: false, error: null });
  }, []);

  return { ...state, search, cancelSearch, clear };
}
