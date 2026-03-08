/** Standard daemon API response envelope. */
export interface ApiResponse<T> {
  ok: boolean;
  data?: T;
  error?: string;
}

/** Validates that a value matches the daemon API response envelope. */
export function isApiResponse(value: unknown): value is ApiResponse<unknown> {
  return (
    typeof value === "object" &&
    value !== null &&
    "ok" in value &&
    typeof (value as ApiResponse<unknown>).ok === "boolean"
  );
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

/** Information about a trainer from the daemon. */
export interface TrainerInfo {
  name: string;
  game_name: string;
  version: string;
  download_url: string;
  file_size?: number;
  checksum?: string;
  source: string;
}

/** Result of a trainer search. */
export interface SearchResult {
  query: string;
  trainers: TrainerInfo[];
  source: string;
}

/** Per-game trainer config (read from daemon). */
export interface TrainerConfig {
  path: string;
  name: string;
  game_name: string;
  version: string;
}

/** SharkDeck subsystem status. */
export type SharkDeckStatus =
  | "idle"
  | "searching"
  | "installing_deps"
  | "downloading"
  | "error";

/** Full status info from the SharkDeck status endpoint. */
export interface SharkDeckStatusInfo {
  status: SharkDeckStatus;
  current_trainer?: {
    name: string;
    game_name: string;
    version: string;
  };
  error?: string;
  progress?: string;
}

/** Result of enabling a trainer. */
export interface EnableResult {
  trainer_path: string;
  launch_options: string;
  needs_restart: boolean;
}

/** Auth token response. */
export interface AuthTokenData {
  token: string;
}
