import * as cheerio from "cheerio";
import type { TrainerInfo, SearchResult } from "./types.js";

const FLING_BASE = "https://flingtrainer.com";

/**
 * Searches the Fling trainer database for trainers matching the given game name.
 *
 * Scrapes the Fling website search results and extracts trainer info.
 */
export async function searchTrainers(gameName: string): Promise<SearchResult> {
  const searchUrl = `${FLING_BASE}/?s=${encodeURIComponent(gameName)}`;

  const response = await fetch(searchUrl, {
    headers: {
      "User-Agent":
        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
    },
  });

  if (!response.ok) {
    throw new Error(`Fling search failed: ${response.status} ${response.statusText}`);
  }

  const html = await response.text();
  const trainers = parseSearchResults(html, gameName);

  return {
    query: gameName,
    trainers,
    source: "flingtrainer.com",
  };
}

/**
 * Parses HTML search results from the Fling website.
 *
 * Note: Selectors are abstracted so they can be updated if the site structure changes.
 */
function parseSearchResults(html: string, gameName: string): TrainerInfo[] {
  const $ = cheerio.load(html);
  const trainers: TrainerInfo[] = [];

  // Fling uses article elements for search results
  $("article").each((_i, el) => {
    const titleEl = $(el).find(".entry-title a, h2 a").first();
    const title = titleEl.text().trim();
    const href = titleEl.attr("href");

    if (!title || !href) return;

    // Only include results that look like trainers
    const lowerTitle = title.toLowerCase();
    if (
      !lowerTitle.includes("trainer") &&
      !lowerTitle.includes(gameName.toLowerCase())
    ) {
      return;
    }

    // Extract version from title (e.g., "Game Name v1.2.3 Trainer")
    const versionMatch = title.match(/v[\d.]+/i);
    const version = versionMatch ? versionMatch[0] : "unknown";

    trainers.push({
      name: title,
      gameName,
      version,
      downloadUrl: href,
      source: "flingtrainer.com",
    });
  });

  return trainers;
}

/**
 * Fetches the actual download link from a Fling trainer page.
 *
 * The search results link to a page, not directly to the download.
 */
export async function resolveDownloadUrl(trainerPageUrl: string): Promise<string> {
  const response = await fetch(trainerPageUrl, {
    headers: {
      "User-Agent":
        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
    },
  });

  if (!response.ok) {
    throw new Error(`Failed to load trainer page: ${response.status}`);
  }

  const html = await response.text();
  const $ = cheerio.load(html);

  // Look for download links — common patterns on Fling's site
  const downloadLink = $('a[href*="download"], a:contains("Download"), .download-link a')
    .first()
    .attr("href");

  if (!downloadLink) {
    throw new Error("could not find download link on trainer page");
  }

  // Handle relative URLs
  if (downloadLink.startsWith("/")) {
    return `${FLING_BASE}${downloadLink}`;
  }

  return downloadLink;
}
