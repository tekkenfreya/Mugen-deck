import { useCallback, useEffect, useState } from "react";
import { closeApp, getApps, launchApp } from "@/api/daemon";
import type { AppInfo } from "@/types";

interface AppsState {
  apps: AppInfo[];
  loading: boolean;
  error: string | null;
}

interface UseAppsReturn extends AppsState {
  refresh: () => Promise<void>;
  launch: (id: string) => Promise<void>;
  close: (id: string) => Promise<void>;
}

/** Manages the app list from the daemon. */
export function useApps(): UseAppsReturn {
  const [state, setState] = useState<AppsState>({
    apps: [],
    loading: true,
    error: null,
  });

  const refresh = useCallback(async () => {
    try {
      const resp = await getApps();
      if (resp.ok && resp.data) {
        setState({ apps: resp.data, loading: false, error: null });
      } else {
        setState({
          apps: [],
          loading: false,
          error: resp.error ?? "failed to load apps",
        });
      }
    } catch {
      setState({ apps: [], loading: false, error: "cannot reach daemon" });
    }
  }, []);

  const launch = useCallback(
    async (id: string) => {
      await launchApp(id);
      await refresh();
    },
    [refresh],
  );

  const close = useCallback(
    async (id: string) => {
      await closeApp(id);
      await refresh();
    },
    [refresh],
  );

  useEffect(() => {
    refresh();
  }, [refresh]);

  return { ...state, refresh, launch, close };
}
