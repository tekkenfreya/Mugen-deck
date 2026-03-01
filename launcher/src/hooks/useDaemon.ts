import { useCallback, useEffect, useState } from "react";
import { getHealth } from "@/api/daemon";
import type { HealthData } from "@/types";

interface DaemonState {
  connected: boolean;
  health: HealthData | null;
  error: string | null;
}

/** Polls daemon /health every `intervalMs` (default 3000). */
export function useDaemon(intervalMs = 3000): DaemonState {
  const [state, setState] = useState<DaemonState>({
    connected: false,
    health: null,
    error: null,
  });

  const poll = useCallback(async () => {
    try {
      const resp = await getHealth();
      if (resp.ok && resp.data) {
        setState({ connected: true, health: resp.data, error: null });
      } else {
        setState({
          connected: false,
          health: null,
          error: resp.error ?? "unknown error",
        });
      }
    } catch {
      setState({
        connected: false,
        health: null,
        error: "cannot reach daemon",
      });
    }
  }, []);

  useEffect(() => {
    poll();
    const id = setInterval(poll, intervalMs);
    return () => clearInterval(id);
  }, [poll, intervalMs]);

  return state;
}
