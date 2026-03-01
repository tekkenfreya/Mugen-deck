/** Information about a trainer found on a trainer database site. */
export interface TrainerInfo {
  /** Display name of the trainer. */
  name: string;
  /** Game name the trainer is for. */
  gameName: string;
  /** Trainer version or date string. */
  version: string;
  /** Direct download URL. */
  downloadUrl: string;
  /** Expected file size in bytes, if known. */
  fileSize?: number;
  /** SHA256 checksum for verification. */
  checksum?: string;
  /** Source website. */
  source: string;
}

/** Configuration for Proton-based trainer execution. */
export interface ProtonConfig {
  /** Path to the Proton installation directory. */
  protonPath: string;
  /** Path to the isolated prefix for trainer execution. */
  prefixPath: string;
  /** Path to the trainer executable. */
  trainerPath: string;
  /** Steam app ID of the game (for compatdata). */
  appId: string;
}

/** Status of a trainer download/launch operation. */
export type TrainerStatus =
  | "idle"
  | "searching"
  | "downloading"
  | "verifying"
  | "launching"
  | "running"
  | "stopped"
  | "error";

/** Result of a trainer search. */
export interface SearchResult {
  query: string;
  trainers: TrainerInfo[];
  source: string;
}
