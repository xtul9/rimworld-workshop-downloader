import { Router } from 'express';
import * as path from 'path';
import * as fs from 'fs/promises';
import { queryModsForUpdates, queryModId } from '../services/modQuery.js';
import { Downloader, ModDownloadInfo } from '../services/downloader.js';
import { ModUpdater } from '../services/modUpdater.js';

/**
 * Find all mod folders in modsPath that have the given modId
 */
async function findAllModFoldersWithId(modsPath: string, modId: string): Promise<string[]> {
  const folders: string[] = [];
  
  try {
    const entries = await fs.readdir(modsPath, { withFileTypes: true });
    const directories = entries
      .filter(entry => entry.isDirectory())
      .map(entry => path.join(modsPath, entry.name));
    
    for (const folderPath of directories) {
      const folderModId = await queryModId(folderPath);
      if (folderModId === modId) {
        folders.push(folderPath);
      }
    }
  } catch (error) {
    console.error(`[UPDATE] Error finding mod folders with ID ${modId}:`, error);
  }
  
  return folders;
}

export const modRouter = Router();

modRouter.get('/greet', (req, res) => {
  const name = (req.query.name as string) || 'World';
  res.json({ 
    message: `Hello, ${name}! You've been greeted from Node.js!` 
  });
});

modRouter.get('/status', (req, res) => {
  res.json({ 
    status: 'running', 
    runtime: 'Node.js',
    timestamp: new Date().toISOString()
  });
});

/**
 * Query mods folder for outdated mods
 */
modRouter.get('/query', async (req, res) => {
  try {
    const modsPath = req.query.modsPath as string;
    const ignoredModsParam = req.query.ignoredMods as string | undefined;
    const ignoredMods = ignoredModsParam ? ignoredModsParam.split(',').filter(id => id.trim().length > 0) : [];
    
    console.log(`[QUERY] Received query request for modsPath: ${modsPath}`);
    if (ignoredMods.length > 0) {
      console.log(`[QUERY] Ignoring ${ignoredMods.length} mod(s): ${ignoredMods.join(', ')}`);
    }
    
    if (!modsPath) {
      console.error('[QUERY] Error: modsPath is missing');
      return res.status(400).json({ error: 'modsPath is required' });
    }

    console.log(`[QUERY] Starting query for mods in: ${modsPath}`);
    const mods = await queryModsForUpdates(modsPath, ignoredMods);
    console.log(`[QUERY] Query completed. Found ${mods.length} mods with updates.`);
    
    // Calculate approximate response size for logging
    try {
      const responseSize = JSON.stringify({ mods }).length;
      const responseSizeMB = (responseSize / (1024 * 1024)).toFixed(2);
      console.log(`[QUERY] Response size: ~${responseSizeMB} MB (${responseSize} bytes)`);
    } catch (sizeError) {
      console.warn(`[QUERY] Could not calculate response size:`, sizeError);
    }
    
    console.log(`[QUERY] Sending response with ${mods.length} mods...`);
    
    try {
      res.json({ mods });
      console.log(`[QUERY] Response sent successfully`);
    } catch (error) {
      console.error(`[QUERY] Error sending response:`, error);
      if (error instanceof Error) {
        console.error(`[QUERY] Error stack:`, error.stack);
      }
      throw error;
    }
  } catch (error) {
    console.error('[QUERY] Error querying mods:', error);
    if (error instanceof Error) {
      console.error('[QUERY] Error stack:', error.stack);
    }
    res.status(500).json({ 
      error: 'Failed to query mods',
      message: error instanceof Error ? error.message : String(error)
    });
  }
});

/**
 * Update mods
 */
modRouter.post('/update', async (req, res) => {
  try {
    const { mods, backupMods, backupDirectory } = req.body;
    
    if (!mods || !Array.isArray(mods) || mods.length === 0) {
      return res.status(400).json({ error: 'mods array is required' });
    }

    const createBackup = backupMods === true;

    // Extract modsPath from first mod (all mods should be from same path)
    const firstMod = mods[0];
    if (!firstMod.modPath) {
      return res.status(400).json({ error: 'mods must have modPath' });
    }

    // Get modsPath from first mod's path (parent directory)
    const modsPath = path.dirname(firstMod.modPath);

    // Prepare mods for download
    const modDownloadInfos: ModDownloadInfo[] = mods.map((mod: any) => ({
      modId: mod.modId,
      title: mod.details?.title || mod.modId,
      modsPath: modsPath
    }));

    // Download mods
    const downloader = new Downloader();
    const downloadedMods = await downloader.downloadMods(modDownloadInfos);

    console.log(`[UPDATE] Downloaded ${downloadedMods.length} mod(s) out of ${modDownloadInfos.length} requested`);

    if (downloadedMods.length === 0) {
      return res.status(500).json({ error: 'Failed to download any mods. Check SteamCMD logs for details.' });
    }

    // Update mods
    const modUpdater = new ModUpdater();
    // Use the same download path as Downloader uses
    const downloadPath = path.join(process.cwd(), 'steamcmd', 'steamapps', 'workshop', 'content', '294100');
    console.log(`[UPDATE] Using download path: ${downloadPath}`);
    const updatedMods: any[] = [];

    for (const downloadedMod of downloadedMods) {
      const originalMod = mods.find((m: any) => m.modId === downloadedMod.modId);
      if (!originalMod) {
        console.warn(`Could not find original mod for downloaded mod ${downloadedMod.modId}`);
        continue;
      }

      try {
        // Use existing folder name from originalMod if available
        // originalMod.folder is the folder name, originalMod.modPath is the full path
        const existingFolderName = originalMod.folder || (originalMod.modPath ? path.basename(originalMod.modPath) : undefined);
        const modPath = await modUpdater.updateMod(downloadedMod, downloadPath, modsPath, existingFolderName, createBackup, backupDirectory);
        console.log(`[UPDATE] Mod ${downloadedMod.modId} copied to ${modPath}`);
        
        // Get remote update time from original mod details (from Steam API)
        const remoteUpdateTime = originalMod.details?.time_updated || Math.floor(Date.now() / 1000);
        const timestamp = remoteUpdateTime.toString();
        
        // Find all folders with the same mod ID and update .lastupdated in all of them
        // This handles cases where the same mod exists in multiple folders (e.g., folder with ID name and folder with mod name)
        const allModFolders = await findAllModFoldersWithId(modsPath, downloadedMod.modId);
        console.log(`[UPDATE] Found ${allModFolders.length} folder(s) with mod ID ${downloadedMod.modId}`);
        
        for (const folderPath of allModFolders) {
          const aboutPath = path.join(folderPath, 'About');
          const lastUpdatedPath = path.join(aboutPath, '.lastupdated');
          try {
            // Ensure About directory exists
            await fs.mkdir(aboutPath, { recursive: true });
            await fs.writeFile(lastUpdatedPath, timestamp, 'utf-8');
            console.log(`[UPDATE] Wrote .lastupdated file for mod ${downloadedMod.modId} in folder ${path.basename(folderPath)} with timestamp ${timestamp} (from Steam API)`);
          } catch (error) {
            console.error(`[UPDATE] Failed to write .lastupdated file for mod ${downloadedMod.modId} in folder ${path.basename(folderPath)}:`, error);
            if (error instanceof Error) {
              console.error(`[UPDATE] Error stack:`, error.stack);
            }
          }
        }

        updatedMods.push({
          ...originalMod,
          updated: true,
          modPath: modPath
        });
      } catch (error) {
        console.error(`Failed to update mod ${downloadedMod.modId}:`, error);
        // Continue with other mods
      }
    }

    res.json(updatedMods);
  } catch (error) {
    console.error('Error updating mods:', error);
    res.status(500).json({ 
      error: 'Failed to update mods',
      message: error instanceof Error ? error.message : String(error)
    });
  }
});

/**
 * Check if backup exists for a mod
 */
modRouter.get('/check-backup', async (req, res) => {
  try {
    const modPath = req.query.modPath as string;
    const backupDirectory = req.query.backupDirectory as string;
    
    if (!modPath) {
      return res.status(400).json({ error: 'modPath is required' });
    }

    if (!backupDirectory) {
      // No backup directory configured, no backup available
      return res.json({ hasBackup: false, backupPath: null });
    }

    // Extract folder name from modPath
    const folderName = path.basename(modPath);
    const backupPath = path.join(backupDirectory, folderName);

    try {
      await fs.access(backupPath);
      
      // Get backup creation date (mtime of the backup folder)
      const stats = await fs.stat(backupPath);
      const backupDate = stats.mtime; // mtime is when the folder was last modified (created)
      
      res.json({ 
        hasBackup: true, 
        backupPath,
        backupDate: backupDate.toISOString() // Return as ISO string for easy parsing
      });
    } catch (error) {
      // Backup doesn't exist
      res.json({ hasBackup: false, backupPath });
    }
  } catch (error) {
    console.error('Error checking backup:', error);
    res.status(500).json({
      error: 'Failed to check backup',
      message: error instanceof Error ? error.message : String(error)
    });
  }
});

/**
 * Restore mod from backup
 */
modRouter.post('/restore-backup', async (req, res) => {
  try {
    const { modPath, backupDirectory } = req.body;
    
    if (!modPath || !backupDirectory) {
      return res.status(400).json({ error: 'modPath and backupDirectory are required' });
    }

    // Normalize paths to avoid issues with different path separators
    const normalizedModPath = path.normalize(modPath);
    const normalizedBackupDirectory = path.normalize(backupDirectory);

    console.log(`[RESTORE] Starting restore: modPath=${normalizedModPath}, backupDirectory=${normalizedBackupDirectory}`);

    // Safety check: ensure backupDirectory is not inside modPath (or vice versa)
    // This prevents accidentally deleting the backup directory
    if (normalizedModPath.startsWith(normalizedBackupDirectory) || normalizedBackupDirectory.startsWith(normalizedModPath)) {
      return res.status(400).json({ 
        error: 'Backup directory cannot be inside mods path or vice versa. They must be separate directories.' 
      });
    }

    // Extract folder name from modPath
    const folderName = path.basename(modPath);
    const backupPath = path.join(normalizedBackupDirectory, folderName);

    // Additional safety check: ensure backupPath is actually inside backupDirectory
    const normalizedBackupPath = path.normalize(backupPath);
    if (!normalizedBackupPath.startsWith(normalizedBackupDirectory)) {
      return res.status(400).json({ error: 'Invalid backup path detected' });
    }

    // Critical safety check: ensure backupPath and modPath are not the same
    // This prevents accidentally deleting the backup when restoring
    if (normalizedBackupPath === normalizedModPath) {
      console.error(`[RESTORE] ERROR: Backup path and mod path are the same! backupPath=${normalizedBackupPath}, modPath=${normalizedModPath}`);
      return res.status(400).json({ 
        error: 'Backup path and mod path cannot be the same. Please ensure backup directory is different from mods directory.' 
      });
    }

    console.log(`[RESTORE] Backup path: ${normalizedBackupPath}, Mod path: ${normalizedModPath}`);

    // Check if backup exists
    try {
      await fs.access(backupPath);
      console.log(`[RESTORE] Backup exists at: ${backupPath}`);
    } catch (error) {
      console.error(`[RESTORE] Backup not found at: ${backupPath}`);
      return res.status(404).json({ error: 'Backup not found' });
    }

    // Remove current mod folder
    if (await fs.access(normalizedModPath).then(() => true).catch(() => false)) {
      await fs.rm(normalizedModPath, { recursive: true, force: true });
      console.log(`[RESTORE] Removed current mod folder: ${normalizedModPath}`);
    }

    // Copy backup to mods folder
    // IMPORTANT: fs.cp copies the SOURCE directory TO the DESTINATION
    // So backupPath (source) will be copied to normalizedModPath (destination)
    // This means the contents of backupPath will be in normalizedModPath
    console.log(`[RESTORE] Copying from backup: ${backupPath} -> ${normalizedModPath}`);
    await fs.cp(backupPath, normalizedModPath, { 
      recursive: true,
      force: true
    });
    console.log(`[RESTORE] Successfully copied backup to mod path`);

    // Verify backup still exists after copy
    try {
      await fs.access(backupPath);
      console.log(`[RESTORE] Backup still exists after restore (good!)`);
    } catch (error) {
      console.error(`[RESTORE] WARNING: Backup was deleted during restore! This should not happen!`);
    }

    // Actually delete the backup\
    try {
      await fs.rm(backupPath, { recursive: true, force: true });
      console.log(`[RESTORE] Deleted backup: ${backupPath}`);
    } catch (error) {
      console.error(`[RESTORE] Failed to delete backup: ${backupPath}:`, error);
    }

    res.json({ message: 'Backup restored successfully', modPath: normalizedModPath });
  } catch (error) {
    console.error('Error restoring backup:', error);
    res.status(500).json({
      error: 'Failed to restore backup',
      message: error instanceof Error ? error.message : String(error)
    });
  }
});

/**
 * Ignore this update - update .lastupdated file with current remote timestamp
 */
modRouter.post('/ignore-update', async (req, res) => {
  try {
    const { mods } = req.body;
    
    if (!mods || !Array.isArray(mods) || mods.length === 0) {
      return res.status(400).json({ error: 'mods array is required' });
    }

    const STEAM_API_BASE = 'http://api.steampowered.com';
    const ignoredMods: any[] = [];

    for (const mod of mods) {
      try {
        let timeUpdated: number;

        // Try to use details.time_updated if available
        if (mod.details && mod.details.time_updated) {
          timeUpdated = mod.details.time_updated;
        } else {
          // Fetch from Steam API
          const url = `${STEAM_API_BASE}/ISteamRemoteStorage/GetPublishedFileDetails/v0001/`;
          const response = await fetch(url, {
            method: 'POST',
            headers: {
              'Content-Type': 'application/x-www-form-urlencoded',
              'User-Agent': 'RimworldWorkshopDownloader/1.0',
              'Accept': 'application/json',
            },
            body: new URLSearchParams({
              itemcount: '1',
              'publishedfileids[0]': mod.modId,
              format: 'json',
            }),
          });

          if (response.ok) {
            const data = await response.json() as {
              response: {
                publishedfiledetails: Array<{ time_updated: number }>;
              };
            };
            const fileDetails = data.response?.publishedfiledetails?.[0];
            if (fileDetails && fileDetails.time_updated) {
              timeUpdated = fileDetails.time_updated;
            } else {
              // Fallback to current time
              timeUpdated = Math.floor(Date.now() / 1000);
            }
          } else {
            // Fallback to current time if API request fails
            timeUpdated = Math.floor(Date.now() / 1000);
          }
        }

        // Find all mod folders with this modId
        const modsPath = mod.modPath ? path.dirname(mod.modPath) : '';
        if (!modsPath) {
          console.warn(`[IGNORE] No modsPath available for mod ${mod.modId}`);
          continue;
        }

        const allModFolders = await findAllModFoldersWithId(modsPath, mod.modId);
        console.log(`[IGNORE] Found ${allModFolders.length} folder(s) with mod ID ${mod.modId}`);

        // Update .lastupdated file for all folders with this modId
        for (const folderPath of allModFolders) {
          const aboutPath = path.join(folderPath, 'About');
          const lastUpdatedPath = path.join(aboutPath, '.lastupdated');
          const timestamp = timeUpdated.toString();

          try {
            await fs.mkdir(aboutPath, { recursive: true });
            await fs.writeFile(lastUpdatedPath, timestamp, 'utf-8');
            console.log(`[IGNORE] Updated .lastupdated file for mod ${mod.modId} in folder ${path.basename(folderPath)} with timestamp ${timestamp}`);
          } catch (error) {
            console.error(`[IGNORE] Failed to write .lastupdated file for mod ${mod.modId} in folder ${path.basename(folderPath)}:`, error);
          }
        }

        ignoredMods.push({
          modId: mod.modId,
          ignored: true
        });
      } catch (error) {
        console.error(`[IGNORE] Failed to ignore update for mod ${mod.modId}:`, error);
        // Continue with other mods
      }
    }

    res.json(ignoredMods);
  } catch (error) {
    console.error('Error ignoring update:', error);
    res.status(500).json({ 
      error: 'Failed to ignore update',
      message: error instanceof Error ? error.message : String(error)
    });
  }
});

