import { createHash } from "node:crypto";
import { mkdir, readFile, stat, unlink } from "node:fs/promises";
import { join } from "node:path";

import type { TrainerInfo } from "./types.js";

/**
 * Returns the trainer cache directory.
 * `~/.local/share/mugen/cache/trainers/`
 */
function cacheDir(): string {
  const home = process.env["HOME"] ?? "";
  return join(home, ".local", "share", "mugen", "cache", "trainers");
}

/**
 * Downloads a trainer to the cache directory.
 *
 * Returns the local file path of the downloaded trainer.
 */
export async function downloadTrainer(
  trainer: TrainerInfo,
  resolvedUrl: string,
): Promise<string> {
  const dir = cacheDir();
  await mkdir(dir, { recursive: true });

  // Sanitize filename from trainer name
  const safeName = trainer.name
    .replace(/[^a-zA-Z0-9_\-. ]/g, "")
    .replace(/\s+/g, "_")
    .substring(0, 100);
  const filePath = join(dir, `${safeName}.exe`);

  // Check if already cached
  try {
    const existing = await stat(filePath);
    if (existing.isFile() && existing.size > 0) {
      // Verify checksum if available
      if (trainer.checksum) {
        const valid = await verifyChecksum(filePath, trainer.checksum);
        if (valid) return filePath;
        // Invalid checksum — re-download
        await unlink(filePath);
      } else {
        return filePath;
      }
    }
  } catch {
    // File doesn't exist, proceed with download
  }

  const response = await fetch(resolvedUrl);
  if (!response.ok || !response.body) {
    throw new Error(`download failed: ${response.status} ${response.statusText}`);
  }

  const arrayBuffer = await response.arrayBuffer();
  const { writeFile } = await import("node:fs/promises");
  await writeFile(filePath, Buffer.from(arrayBuffer));

  // Verify checksum if provided
  if (trainer.checksum) {
    const valid = await verifyChecksum(filePath, trainer.checksum);
    if (!valid) {
      await unlink(filePath);
      throw new Error("checksum verification failed");
    }
  }

  return filePath;
}

/**
 * Verifies the SHA256 checksum of a file.
 */
async function verifyChecksum(filePath: string, expected: string): Promise<boolean> {
  const data = await readFile(filePath);
  const hash = createHash("sha256").update(data).digest("hex");
  return hash === expected.toLowerCase();
}
