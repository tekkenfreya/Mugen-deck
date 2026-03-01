import type { ChildProcess } from "node:child_process";

import { searchTrainers, resolveDownloadUrl } from "./fling.js";
import { downloadTrainer } from "./trainer.js";
import { findProton, createIsolatedPrefix, launchTrainer, stopTrainer } from "./proton.js";
import type { ProtonConfig, TrainerInfo, TrainerStatus } from "./types.js";

/** Current SharkDeck state. */
interface SharkDeckState {
  status: TrainerStatus;
  currentTrainer: TrainerInfo | null;
  trainerProcess: ChildProcess | null;
  error: string | null;
}

const state: SharkDeckState = {
  status: "idle",
  currentTrainer: null,
  trainerProcess: null,
  error: null,
};

/**
 * Searches for trainers matching the given game name.
 */
export async function search(gameName: string) {
  state.status = "searching";
  state.error = null;

  try {
    const result = await searchTrainers(gameName);
    state.status = "idle";
    return result;
  } catch (err) {
    state.status = "error";
    state.error = err instanceof Error ? err.message : "search failed";
    throw err;
  }
}

/**
 * Downloads and launches a trainer for the specified game.
 */
export async function launch(trainer: TrainerInfo, appId: string) {
  try {
    // Download
    state.status = "downloading";
    state.currentTrainer = trainer;
    const resolvedUrl = await resolveDownloadUrl(trainer.downloadUrl);
    const trainerPath = await downloadTrainer(trainer, resolvedUrl);

    // Find Proton
    state.status = "launching";
    const protonPath = await findProton();
    const prefixPath = await createIsolatedPrefix(appId);

    const config: ProtonConfig = {
      protonPath,
      prefixPath,
      trainerPath,
      appId,
    };

    // Launch
    const process = await launchTrainer(config);
    state.trainerProcess = process;
    state.status = "running";

    // Monitor process
    process.on("exit", (code) => {
      state.status = "stopped";
      state.trainerProcess = null;
      if (code !== 0 && code !== null) {
        state.error = `trainer exited with code ${code}`;
      }
    });

    process.on("error", (err) => {
      state.status = "error";
      state.error = err.message;
      state.trainerProcess = null;
    });

    return { pid: process.pid, path: trainerPath };
  } catch (err) {
    state.status = "error";
    state.error = err instanceof Error ? err.message : "launch failed";
    throw err;
  }
}

/**
 * Stops the currently running trainer.
 */
export function stop() {
  if (state.trainerProcess) {
    stopTrainer(state.trainerProcess);
    state.trainerProcess = null;
    state.status = "stopped";
  }
}

/**
 * Returns current SharkDeck status.
 */
export function getStatus() {
  return {
    status: state.status,
    currentTrainer: state.currentTrainer
      ? {
          name: state.currentTrainer.name,
          gameName: state.currentTrainer.gameName,
          version: state.currentTrainer.version,
        }
      : null,
    error: state.error,
  };
}
