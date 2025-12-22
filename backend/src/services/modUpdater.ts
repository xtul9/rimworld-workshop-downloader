import * as fs from 'fs/promises';
import * as path from 'path';
import { DownloadedMod } from './downloader.js';
import { queryModId } from './modQuery.js';

export class ModUpdater {
  /**
   * Sanitize folder name to be safe for filesystem
   * Removes invalid characters and trims whitespace
   */
  private sanitizeFolderName(name: string): string {
    // Remove invalid characters for folder names (Windows and Unix)
    // Invalid: < > : " / \ | ? * and control characters
    let sanitized = name
      .replace(/[<>:"/\\|?*\x00-\x1F]/g, '') // Remove invalid chars
      .replace(/\s+/g, ' ') // Replace multiple spaces with single space
      .trim(); // Trim whitespace
    
    // Remove leading/trailing dots and spaces (Windows doesn't allow these)
    sanitized = sanitized.replace(/^[.\s]+|[.\s]+$/g, '');
    
    // If empty after sanitization, use a fallback
    if (!sanitized || sanitized.length === 0) {
      return 'Mod';
    }
    
    // Limit length to avoid filesystem issues
    if (sanitized.length > 200) {
      sanitized = sanitized.substring(0, 200).trim();
    }
    
    return sanitized;
  }

  /**
   * Update/Copy mod from download folder to mods folder
   * @param mod - Downloaded mod to update
   * @param downloadPath - Path where mod was downloaded
   * @param modsPath - Path to mods folder
   * @param existingFolderName - Name of existing folder (if mod already exists), otherwise will use mod title
   * @param createBackup - Whether to create backup before updating
   * @param backupDirectory - Directory where backups should be stored (if createBackup is true)
   */
  async updateMod(mod: DownloadedMod, downloadPath: string, modsPath: string, existingFolderName?: string, createBackup: boolean = false, backupDirectory?: string): Promise<string> {
    // Use existing folder name if provided, otherwise find existing folder with same mod ID, otherwise use mod title
    let folderName: string;
    
    if (existingFolderName) {
      folderName = existingFolderName;
    } else {
      // Try to find existing folder with the same mod ID
      const existingFolder = await this.findExistingModFolder(modsPath, mod.modId);
      if (existingFolder) {
        folderName = path.basename(existingFolder);
        console.log(`[ModUpdater] Found existing folder "${folderName}" for mod ${mod.modId}`);
      } else {
        // Use mod title if available, otherwise fall back to modId
        const modTitle = mod.details?.title || mod.modId;
        folderName = this.sanitizeFolderName(modTitle);
        
        // Check if folder with this name already exists and has different mod ID
        const proposedPath = path.join(modsPath, folderName);
        if (await this.pathExists(proposedPath)) {
          const existingModId = await queryModId(proposedPath);
          if (existingModId && existingModId !== mod.modId) {
            // Folder exists with different mod ID, append modId to avoid conflict
            folderName = `${folderName} (${mod.modId})`;
            console.log(`[ModUpdater] Folder "${this.sanitizeFolderName(modTitle)}" exists with different mod ID, using "${folderName}" instead`);
          }
        }
        
        console.log(`[ModUpdater] No existing folder found for mod ${mod.modId}, will use "${folderName}" as folder name`);
      }
    }
    
    const modDestinationPath = path.join(modsPath, folderName);

    try {
      // Ensure mods folder exists
      await fs.mkdir(modsPath, { recursive: true });

      // Create backup if requested
      if (createBackup && backupDirectory) {
        try {
          // Ensure backup directory exists
          await fs.mkdir(backupDirectory, { recursive: true });
          
          // Backup path uses the same folder name as the mod (no .backup suffix)
          const backupPath = path.join(backupDirectory, folderName);
          
          // Remove old backup if exists
          await fs.rm(backupPath, { recursive: true, force: true });
          
          // Copy current mod to backup directory
          if (await this.pathExists(modDestinationPath)) {
            await fs.cp(modDestinationPath, backupPath, { recursive: true });
            console.log(`[ModUpdater] Created backup for mod ${mod.modId} at ${backupPath}`);
          }
        } catch (error) {
          console.warn(`Failed to create backup for mod ${mod.modId}:`, error);
        }
      }

      // Remove existing mod folder if it exists
      if (await this.pathExists(modDestinationPath)) {
        await fs.rm(modDestinationPath, { recursive: true, force: true });
      }

      // Copy mod from download folder to game mods folder
      const sourcePath = mod.modPath || path.join(downloadPath, mod.modId);
      
      if (!(await this.pathExists(sourcePath))) {
        throw new Error(`Source mod folder not found: ${sourcePath}`);
      }

      await fs.cp(sourcePath, modDestinationPath, { recursive: true });

      console.log(`Mod ${mod.modId} copied to ${modDestinationPath}`);

      return modDestinationPath;
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : String(error);
      throw new Error(`Failed to update mod ${mod.modId}: ${errorMessage}`);
    }
  }

  /**
   * Check if a path exists
   */
  private async pathExists(filePath: string): Promise<boolean> {
    try {
      await fs.access(filePath);
      return true;
    } catch {
      return false;
    }
  }

  /**
   * Find existing mod folder with the given mod ID
   */
  private async findExistingModFolder(modsPath: string, modId: string): Promise<string | null> {
    try {
      const entries = await fs.readdir(modsPath, { withFileTypes: true });
      const directories = entries
        .filter(entry => entry.isDirectory())
        .map(entry => path.join(modsPath, entry.name));
      
      for (const folderPath of directories) {
        const folderModId = await queryModId(folderPath);
        if (folderModId === modId) {
          return folderPath;
        }
      }
    } catch (error) {
      console.error(`[ModUpdater] Error finding existing mod folder for ${modId}:`, error);
    }
    
    return null;
  }
}

