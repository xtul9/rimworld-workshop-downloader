import * as fs from 'fs/promises';
import * as path from 'path';

export interface BaseMod {
  modId: string;
  modPath: string;
  folder?: string;
  details?: any;
  updated?: boolean;
}

export interface WorkshopFileDetails {
  publishedfileid: string;
  result: number;
  creator: string;
  creator_app_id: number;
  consumer_app_id: number;
  filename: string;
  file_size: number;
  file_url: string;
  hcontent_file: string;
  preview_url: string;
  hcontent_preview: string;
  title: string;
  description: string;
  time_created: number;
  time_updated: number;
  visibility: number;
  flags: number;
  workshop_file_url: string;
  workshop_accepted: boolean;
  show_subscribe_all: boolean;
  num_comments_developer: number;
  num_comments_public: number;
  banned: boolean;
  ban_reason: string;
  banner: string;
  can_be_deleted: boolean;
  app_name: string;
  file_type: number;
  can_subscribe: boolean;
  subscriptions: number;
  favorited: number;
  followers: number;
  lifetime_subscriptions: number;
  lifetime_favorited: number;
  lifetime_followers: number;
  lifetime_playtime: string;
  lifetime_playtime_sessions: string;
  views: number;
  num_children: number;
  num_reports: number;
  tags: Array<{ tag: string }>;
}

/**
 * Query mod ID from mod folder by reading PublishedFileId.txt
 */
export async function queryModId(modPath: string): Promise<string | null> {
  const aboutPath = path.join(modPath, 'About');
  
  try {
    const aboutStats = await fs.stat(aboutPath);
    if (!aboutStats.isDirectory()) {
      // Silently skip - not a mod folder
      return null;
    }
  } catch (error) {
    // Silently skip if About folder doesn't exist - not a mod
    if (error && typeof error === 'object' && 'code' in error && (error as any).code === 'ENOENT') {
      return null;
    }
    console.warn(`[QUERYMODID] Error checking About folder for ${path.basename(modPath)}:`, error);
    return null;
  }

  const fileIdPath = path.join(aboutPath, 'PublishedFileId.txt');
  
  try {
    await fs.access(fileIdPath);
  } catch (error) {
    // Silently skip if PublishedFileId.txt doesn't exist - not a workshop mod
    if (error && typeof error === 'object' && 'code' in error && (error as any).code === 'ENOENT') {
      return null;
    }
    console.warn(`[QUERYMODID] Error accessing PublishedFileId.txt for ${path.basename(modPath)}:`, error);
    return null;
  }

  try {
    const fileId = (await fs.readFile(fileIdPath, 'utf-8')).trim();
    if (!fileId || fileId.length === 0) {
      console.warn(`[QUERYMODID] PublishedFileId.txt is empty for ${path.basename(modPath)}`);
      return null;
    }
    return fileId;
  } catch (error) {
    console.warn(`[QUERYMODID] Failed to read PublishedFileId.txt from ${modPath}:`, error);
    return null;
  }
}

/**
 * Get mod's last updated time
 * Checks for .lastupdated file first, then falls back to PublishedFileId.txt creation time
 */
export async function getModLastUpdatedTime(modPath: string): Promise<Date> {
  const aboutPath = path.join(modPath, 'About');
  const lastUpdatedPath = path.join(aboutPath, '.lastupdated');

  // Check for .lastupdated timestamp file
  try {
    const lastUpdatedContent = await fs.readFile(lastUpdatedPath, 'utf-8');
    const trimmed = lastUpdatedContent.trim();
    if (trimmed && trimmed.length > 0) {
      const timestamp = parseInt(trimmed, 10);
      if (!isNaN(timestamp) && timestamp > 0) {
        const date = new Date(timestamp * 1000); // Convert Unix timestamp to Date
        console.log(`[MODQUERY] Read .lastupdated for ${path.basename(modPath)}: timestamp=${timestamp}, date=${date.toISOString()}`);
        return date;
      } else {
        console.warn(`[MODQUERY] Invalid .lastupdated file format at ${lastUpdatedPath} (content: "${trimmed}"). Deleting file.`);
        await fs.unlink(lastUpdatedPath);
      }
    }
  } catch (error) {
    // File doesn't exist, continue to fallback
    if (error && typeof error === 'object' && 'code' in error && (error as any).code !== 'ENOENT') {
      console.warn(`[MODQUERY] Error reading .lastupdated file at ${lastUpdatedPath}:`, error);
    }
  }

  // Fallback: use PublishedFileId.txt creation time
  const fileIdPath = path.join(aboutPath, 'PublishedFileId.txt');
  try {
    const stats = await fs.stat(fileIdPath);
    return stats.birthtime || stats.mtime; // Use birthtime (creation) or mtime (modification) as fallback
  } catch (error) {
    // If PublishedFileId.txt doesn't exist, use mod folder's modification time
    const stats = await fs.stat(modPath);
    return stats.mtime;
  }
}

/**
 * Query batch of mods from Steam Workshop API
 */
export async function queryModBatch(
  modIds: string[],
  retries: number = 0
): Promise<WorkshopFileDetails[] | null> {
  const STEAM_API_BASE = 'http://api.steampowered.com';
  const maxRetries = 3;

  try {
    const url = `${STEAM_API_BASE}/ISteamRemoteStorage/GetPublishedFileDetails/v0001/`;
    
    // Build URLSearchParams for multiple mod IDs (no API key needed, anonymous access)
    const params = new URLSearchParams({
      itemcount: modIds.length.toString(),
      format: 'json',
    });
    
    // Remove duplicates and add to params
    const uniqueIds: string[] = [];
    modIds.forEach((id) => {
      if (!uniqueIds.includes(id)) {
        params.append(`publishedfileids[${uniqueIds.length}]`, id);
        uniqueIds.push(id);
      }
    });
    
    // Update itemcount to reflect unique IDs
    params.set('itemcount', uniqueIds.length.toString());

    const response = await fetch(url, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/x-www-form-urlencoded',
        'User-Agent': 'RimworldWorkshopDownloader/1.0',
        'Accept': 'application/json',
      },
      body: params,
    });

    if (!response.ok) {
      throw new Error(`Steam API error: ${response.statusText}`);
    }

    const data = await response.json() as {
      response: {
        result: number;
        resultcount: number;
        publishedfiledetails: WorkshopFileDetails[];
      };
    };

    if (retries > 0) {
      console.log(`Got batch of ${modIds.length} mods successfully after ${retries} retries.`);
    }

    return data.response?.publishedfiledetails || [];
  } catch (error) {
    if (retries < maxRetries) {
      console.warn(`Failed to query batch of ${modIds.length} mods. Retry ${retries + 1}...`);
      await new Promise(resolve => setTimeout(resolve, 1000 * (retries + 1))); // Exponential backoff
      return queryModBatch(modIds, retries + 1);
    } else {
      console.warn(`Failed to query batch of ${modIds.length} mods after ${maxRetries} retries.`);
      return null;
    }
  }
}

/**
 * Query all mods in mods folder and check for updates
 */
export async function queryModsForUpdates(modsPath: string, ignoredMods: string[] = []): Promise<BaseMod[]> {
  console.log(`[MODQUERY] Starting queryModsForUpdates with modsPath: ${modsPath}`);
  if (ignoredMods.length > 0) {
    console.log(`[MODQUERY] Will ignore ${ignoredMods.length} mod(s): ${ignoredMods.join(', ')}`);
  }
  
  // Check if mods path exists
  try {
    const stats = await fs.stat(modsPath);
    if (!stats.isDirectory()) {
      console.error(`[MODQUERY] Error: Path is not a directory: ${modsPath}`);
      throw new Error(`Mods path is not a directory: ${modsPath}`);
    }
    console.log(`[MODQUERY] Mods path exists and is a directory: ${modsPath}`);
  } catch (error) {
    console.error(`[MODQUERY] Error accessing mods path: ${modsPath}`, error);
    if (error && typeof error === 'object' && 'code' in error && (error as any).code === 'ENOENT') {
      throw new Error(`Mods path does not exist: ${modsPath}`);
    }
    throw error;
  }

  // Get all folders in mods directory
  console.log(`[MODQUERY] Reading directory contents: ${modsPath}`);
  const entries = await fs.readdir(modsPath, { withFileTypes: true });
  console.log(`[MODQUERY] Found ${entries.length} entries in mods directory`);
  const folders = entries
    .filter(entry => entry.isDirectory())
    .map(entry => path.join(modsPath, entry.name));
  console.log(`[MODQUERY] Found ${folders.length} directories (potential mod folders)`);

  if (folders.length === 0) {
    console.error('Tried to query mod folders but found none.');
    return [];
  }

  console.log(`Querying ${folders.length} mod folders for outdated mods...`);

  // Query mod IDs from each folder
  const mods: BaseMod[] = [];
  let validModCount = 0;

  for (const folder of folders) {
    const modId = await queryModId(folder);
    if (modId) {
      const folderName = path.basename(folder);
      console.log(`Got valid mod folder ${folderName} (${modId})`);
      
      mods.push({
        modId,
        modPath: folder,
        folder: folderName,
      });
      validModCount++;
    }
  }

  console.log(`Found ${validModCount}/${folders.length} valid mod folders.`);

  if (mods.length === 0) {
    return [];
  }

  // Query mods in batches of 50
  const batchCount = 50;
  const numBatches = Math.ceil(mods.length / batchCount);
  console.log(`Querying ${mods.length} mods in ${numBatches} batches of ${batchCount}`);

  // Query all batches
  const batchPromises: Promise<void>[] = [];
  for (let i = 0; i < mods.length; i += batchCount) {
    const batch = mods.slice(i, Math.min(i + batchCount, mods.length));
    const modIds = batch.map(m => m.modId);

    const promise = (async () => {
      const details = await queryModBatch(modIds);
      if (details) {
        // Match details to mods
        for (const detail of details) {
          const matchingMods = batch.filter(m => m.modId === detail.publishedfileid);
          for (const mod of matchingMods) {
            mod.details = detail;
          }
        }
      }
    })();

    batchPromises.push(promise);
    
    // Delay between batches to avoid rate limiting
    if (i + batchCount < mods.length) {
      await new Promise(resolve => setTimeout(resolve, 250));
    }
  }

  await Promise.all(batchPromises);

  const modsWithDetails = mods.filter(m => m.details);
  console.log(`Got workshop file details for ${modsWithDetails.length} mods.`);
  
  if (modsWithDetails.length === 0) {
    console.log(`[MODQUERY] No mods have details, returning empty array`);
    return [];
  }

  // Check which mods have updates available
  // Use a Map to avoid duplicates (same modId can appear in multiple folders)
  const modsWithUpdatesMap = new Map<string, BaseMod>();
  let updateCount = 0;

  for (const mod of mods) {
    const details = mod.details;
    const folderName = mod.folder || path.basename(mod.modPath);

    if (!details) {
      console.error(`Couldn't get any file details for mod ${mod.modId} (${folderName}).`);
      continue;
    }

    const id = details.publishedfileid;
    
    // Check for various error conditions
    if (details.result === 9) {
      console.warn(`Tried to query workshop file ${id} (${folderName}) but no file could be found. (Code 9). This could mean the mod has been removed/unlisted`);
      continue;
    }

    if (details.result !== 1) {
      console.warn(`Tried to query workshop file ${id} (${folderName}) but steam returned code ${details.result}`);
    }

    if (details.visibility !== 0) {
      console.warn(`Got workshop file ${id} (${folderName}) but it's a private file.`);
      continue;
    }

    // Check if banned (banned can be boolean or number 1/0)
    const isBanned = details.banned === true || details.banned === 1;
    if (isBanned) {
      console.warn(`Got workshop file ${id} (${folderName}) but it's a banned file.`);
      continue;
    }

    if (details.creator_app_id !== 294100) {
      console.warn(`Got workshop file ${id} (${folderName}) but it's not a rimworld mod! (Huh?)`);
      continue;
    }

    // Compare dates
    const remoteDate = new Date(details.time_updated * 1000); // Steam uses Unix timestamp in seconds
    const lastUpdatedDate = await getModLastUpdatedTime(mod.modPath);

    // Calculate time difference in seconds
    const timeDiffSeconds = (remoteDate.getTime() - lastUpdatedDate.getTime()) / 1000;
    
    // Consider mod as needing update if remote is at least 1 second newer
    // This accounts for potential rounding differences
    const needsUpdate = timeDiffSeconds > 1;

    console.log(`[MODQUERY] Mod ${id} (${folderName}): remote=${remoteDate.toISOString()}, local=${lastUpdatedDate.toISOString()}, diff=${timeDiffSeconds.toFixed(1)}s, needsUpdate=${needsUpdate}`);

    // Skip if mod is in ignored list
    if (ignoredMods.includes(mod.modId)) {
      console.log(`[MODQUERY] Mod ${id} (${folderName}) is in ignored list, skipping.`);
      continue;
    }

    if (needsUpdate) {
      // Only add if we don't already have this modId (avoid duplicates)
      if (!modsWithUpdatesMap.has(mod.modId)) {
        updateCount++;
        console.log(`Mod folder ${folderName} (${details.publishedfileid}) has an update available.`);
        modsWithUpdatesMap.set(mod.modId, mod);
      } else {
        console.log(`Mod ${mod.modId} (${folderName}) already in update list, skipping duplicate.`);
      }
    }
  }

  const modsWithUpdates = Array.from(modsWithUpdatesMap.values());
  console.log(`There are ${modsWithUpdates.length} mods with updates available.`);
  return modsWithUpdates;
}

