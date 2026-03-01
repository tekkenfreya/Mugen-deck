/** Standard daemon API response envelope. */
export interface ApiResponse<T> {
  ok: boolean;
  data?: T;
  error?: string;
}

/** Data returned by GET /health. */
export interface HealthData {
  status: string;
  version: string;
  uptime_secs: number;
}

/** App manifest info from the daemon. */
export interface AppInfo {
  id: string;
  name: string;
  version: string;
  description: string;
  permissions: string[];
  entry: string;
  status: "installed" | "running" | "stopped";
}

/** Steam game from library scan. */
export interface SteamGame {
  app_id: string;
  name: string;
  install_dir: string;
  size_on_disk: number;
  state_flags: number;
}

/** Currently running game. */
export interface RunningGame {
  app_id: string;
  name: string;
  pid: number;
}

/** Update check result. */
export interface UpdateCheck {
  available: unknown[];
}
