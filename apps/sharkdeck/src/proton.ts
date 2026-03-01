import { access, mkdir, readdir } from "node:fs/promises";
import { join } from "node:path";
import { spawn, type ChildProcess } from "node:child_process";

import type { ProtonConfig } from "./types.js";

/**
 * Finds the latest Proton installation.
 *
 * Searches in:
 * 1. `~/.local/share/Steam/compatibilitytools.d/` (custom Proton)
 * 2. `~/.local/share/Steam/steamapps/common/` (official Proton)
 */
export async function findProton(): Promise<string> {
  const home = process.env["HOME"] ?? "";

  // Check custom Proton installations first
  const customDir = join(home, ".local", "share", "Steam", "compatibilitytools.d");
  try {
    const entries = await readdir(customDir);
    const protonDirs = entries
      .filter((e) => e.toLowerCase().includes("proton") || e.toLowerCase().includes("ge"))
      .sort()
      .reverse();

    for (const dir of protonDirs) {
      const protonBin = join(customDir, dir, "proton");
      try {
        await access(protonBin);
        return join(customDir, dir);
      } catch {
        continue;
      }
    }
  } catch {
    // Directory doesn't exist
  }

  // Check official Steam Proton
  const steamCommon = join(home, ".local", "share", "Steam", "steamapps", "common");
  try {
    const entries = await readdir(steamCommon);
    const protonDirs = entries
      .filter((e) => e.startsWith("Proton"))
      .sort()
      .reverse();

    for (const dir of protonDirs) {
      const protonBin = join(steamCommon, dir, "proton");
      try {
        await access(protonBin);
        return join(steamCommon, dir);
      } catch {
        continue;
      }
    }
  } catch {
    // Directory doesn't exist
  }

  throw new Error("no Proton installation found");
}

/**
 * Creates an isolated Proton prefix for trainer execution.
 *
 * Uses `~/.local/share/mugen/cache/prefixes/<appId>/` to isolate
 * trainer execution from the game's own Proton prefix.
 */
export async function createIsolatedPrefix(appId: string): Promise<string> {
  const home = process.env["HOME"] ?? "";
  const prefixPath = join(
    home,
    ".local",
    "share",
    "mugen",
    "cache",
    "prefixes",
    appId,
  );
  await mkdir(prefixPath, { recursive: true });
  return prefixPath;
}

/**
 * Launches a trainer executable via Proton in an isolated prefix.
 *
 * Returns the child process handle for lifecycle management.
 */
export async function launchTrainer(config: ProtonConfig): Promise<ChildProcess> {
  const protonBin = join(config.protonPath, "proton");

  // Verify proton binary exists
  await access(protonBin);

  const env: Record<string, string> = {
    ...process.env as Record<string, string>,
    STEAM_COMPAT_DATA_PATH: config.prefixPath,
    STEAM_COMPAT_CLIENT_INSTALL_PATH: join(
      process.env["HOME"] ?? "",
      ".local",
      "share",
      "Steam",
    ),
    // Block network access for trainer processes
    LD_PRELOAD: "",
  };

  const child = spawn(protonBin, ["run", config.trainerPath], {
    env,
    stdio: "pipe",
    detached: false,
  });

  return child;
}

/**
 * Stops a running trainer process.
 */
export function stopTrainer(process: ChildProcess): void {
  if (!process.killed) {
    process.kill("SIGTERM");

    // Force kill after 5 seconds if still alive
    setTimeout(() => {
      if (!process.killed) {
        process.kill("SIGKILL");
      }
    }, 5000);
  }
}
