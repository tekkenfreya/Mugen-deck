import type {
  ApiResponse,
  AppInfo,
  HealthData,
  RunningGame,
  SteamGame,
} from "@/types";

const DAEMON_BASE = "http://127.0.0.1:7331";

let sessionToken: string | null = null;

/** Sets the session token for authenticated requests. */
export function setSessionToken(token: string): void {
  sessionToken = token;
}

/** Typed fetch wrapper for the daemon REST API. */
async function daemonFetch<T>(
  path: string,
  options: RequestInit = {},
): Promise<ApiResponse<T>> {
  const headers: Record<string, string> = {
    "Content-Type": "application/json",
    ...((options.headers as Record<string, string>) ?? {}),
  };

  if (sessionToken) {
    headers["Authorization"] = `Bearer ${sessionToken}`;
  }

  const resp = await fetch(`${DAEMON_BASE}${path}`, {
    ...options,
    headers,
  });

  const body = (await resp.json()) as ApiResponse<T>;
  return body;
}

/** GET /health — no auth required. */
export async function getHealth(): Promise<ApiResponse<HealthData>> {
  return daemonFetch<HealthData>("/health");
}

/** GET /apps — list all registered apps. */
export async function getApps(): Promise<ApiResponse<AppInfo[]>> {
  return daemonFetch<AppInfo[]>("/apps");
}

/** POST /apps/:id/launch — launch an app. */
export async function launchApp(
  id: string,
): Promise<ApiResponse<{ id: string; status: string }>> {
  return daemonFetch<{ id: string; status: string }>(`/apps/${id}/launch`, {
    method: "POST",
  });
}

/** POST /apps/:id/close — close a running app. */
export async function closeApp(
  id: string,
): Promise<ApiResponse<{ id: string; status: string }>> {
  return daemonFetch<{ id: string; status: string }>(`/apps/${id}/close`, {
    method: "POST",
  });
}

/** GET /game/current — currently running Steam game. */
export async function getCurrentGame(): Promise<
  ApiResponse<RunningGame | null>
> {
  return daemonFetch<RunningGame | null>("/game/current");
}

/** GET /game/library — all detected Steam games. */
export async function getGameLibrary(): Promise<ApiResponse<SteamGame[]>> {
  return daemonFetch<SteamGame[]>("/game/library");
}
