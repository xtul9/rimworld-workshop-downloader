import { spawn, ChildProcess, execSync } from 'child_process';
import * as fs from 'fs/promises';
import * as path from 'path';
import { EventEmitter } from 'events';

export interface ModDownloadInfo {
  modId: string;
  title: string;
  modsPath: string;
}

export interface DownloadedMod {
  modId: string;
  modPath: string;
  folder?: string;
  details?: any;
}

export class Downloader extends EventEmitter {
  private steamCmdPath: string;
  private showWindows: boolean;
  private downloadPath: string;
  private activeDownloads: Set<string> = new Set();
  private steamCmdExecutable: string;

  constructor(steamCmdPath?: string, showWindows: boolean = false) {
    super();
    this.steamCmdPath = steamCmdPath || path.join(process.cwd(), 'steamcmd');
    this.showWindows = showWindows;
    this.downloadPath = path.join(this.steamCmdPath, 'steamapps', 'workshop', 'content', '294100');
    this.steamCmdExecutable = '';
  }

  /**
   * Find SteamCMD from application resources (bundled with app)
   */
  private async findSteamCmdFromResources(): Promise<string | null> {
    const isWindows = process.platform === 'win32';
    const steamCmdExe = isWindows ? 'steamcmd.exe' : 'steamcmd';
    
    // Determine target triple for current platform
    let targetTriple: string;
    if (isWindows) {
      targetTriple = 'x86_64-pc-windows-msvc';
    } else if (process.platform === 'darwin') {
      targetTriple = process.arch === 'arm64' ? 'aarch64-apple-darwin' : 'x86_64-apple-darwin';
    } else {
      targetTriple = 'x86_64-unknown-linux-gnu';
    }
    
    const steamCmdNameWithSuffix = isWindows 
      ? `steamcmd-${targetTriple}.exe`
      : `steamcmd-${targetTriple}`;
    
    // Possible paths where SteamCMD might be located
    // In Tauri, resources are typically in the same directory as the executable
    // or in a resources subdirectory. We check multiple locations.
    const exeDir = process.execPath ? path.dirname(process.execPath) : process.cwd();
    const possiblePaths = [
      // In bundled app, resources might be next to executable
      path.join(exeDir, steamCmdNameWithSuffix),
      // Or in resources subdirectory
      path.join(exeDir, 'resources', steamCmdNameWithSuffix),
      // Or in parent directory (for AppImage, .app bundles, etc.)
      path.join(exeDir, '..', steamCmdNameWithSuffix),
      path.join(exeDir, '..', 'resources', steamCmdNameWithSuffix),
      // Fallback: bin directory in current working directory (for development)
      path.join(process.cwd(), 'bin', 'steamcmd', steamCmdExe),
      // Fallback: old local directory
      path.join(this.steamCmdPath, steamCmdExe)
    ];
    
    for (const possiblePath of possiblePaths) {
      try {
        await fs.access(possiblePath);
        console.log(`[Downloader] Using SteamCMD from resources: ${possiblePath}`);
        return possiblePath;
      } catch {
        continue;
      }
    }
    
    return null;
  }

  /**
   * Find SteamCMD executable - try resources first, then local path, then PATH
   */
  private async findSteamCmdExecutable(): Promise<string> {
    // First, try to find in application resources (bundled SteamCMD)
    const resourceSteamCmd = await this.findSteamCmdFromResources();
    if (resourceSteamCmd) {
      return resourceSteamCmd;
    }
    
    const isWindows = process.platform === 'win32';
    const steamCmdExe = isWindows ? 'steamcmd.exe' : 'steamcmd';
    const localSteamCmdPath = path.join(this.steamCmdPath, steamCmdExe);

    // Try local path
    try {
      await fs.access(localSteamCmdPath);
      console.log(`[Downloader] Using local SteamCMD at: ${localSteamCmdPath}`);
      return localSteamCmdPath;
    } catch (error) {
      console.log(`[Downloader] Local SteamCMD not found at ${localSteamCmdPath}, trying PATH...`);
    }

    // Try to find in PATH
    try {
      if (isWindows) {
        // On Windows, use 'where' command
        const whereOutput = execSync(`where ${steamCmdExe}`, { encoding: 'utf-8', stdio: 'pipe' });
        const globalPath = whereOutput.trim().split('\n')[0].trim();
        if (globalPath) {
          console.log(`[Downloader] Using SteamCMD from PATH: ${globalPath}`);
          return globalPath;
        }
      } else {
        // On Unix-like systems, use 'which' command
        const whichOutput = execSync(`which ${steamCmdExe}`, { encoding: 'utf-8', stdio: 'pipe' });
        const globalPath = whichOutput.trim();
        if (globalPath) {
          console.log(`[Downloader] Using SteamCMD from PATH: ${globalPath}`);
          return globalPath;
        }
      }
    } catch (error) {
      console.log(`[Downloader] SteamCMD not found in PATH`);
    }

    // Not found anywhere
    throw new Error(`SteamCMD not found in resources, at ${localSteamCmdPath}, or in PATH. Please install SteamCMD or rebuild the application.`);
  }

  /**
   * Download mods using SteamCMD
   */
  async downloadMods(mods: ModDownloadInfo[]): Promise<DownloadedMod[]> {
    try {
      // Delete appworkshop file if it exists (as in original code)
      const appworkshopPath = path.join(
        this.steamCmdPath,
        'steamapps',
        'workshop',
        'appworkshop_294100.acf'
      );
      try {
        await fs.unlink(appworkshopPath);
      } catch (error) {
        // File doesn't exist or can't be deleted, ignore
      }

      // Ensure download directory exists
      await fs.mkdir(this.downloadPath, { recursive: true });

      console.log(`Downloading ${mods.length} workshop mods with SteamCMD`);

      // Create steamcmd script
      // Use force_install_dir to ensure mods are downloaded to our local directory
      const scriptLines: string[] = [
        `force_install_dir "${this.steamCmdPath}"`,
        'login anonymous'
      ];
      for (const mod of mods) {
        scriptLines.push(`workshop_download_item 294100 ${mod.modId}`);
      }
      scriptLines.push('quit');

      const scriptContent = scriptLines.join('\n') + '\n';
      // Script path - use absolute path so it works with global SteamCMD
      const scriptPath = path.resolve(path.join(this.steamCmdPath, 'run.txt'));

      await fs.writeFile(scriptPath, scriptContent, 'utf-8');

      // Find SteamCMD executable (local or from PATH)
      const steamCmdPath = await this.findSteamCmdExecutable();
      this.steamCmdExecutable = steamCmdPath;

      // Start watching folder before starting download
      const downloadedMods: DownloadedMod[] = [];
      const downloadPromises: Promise<void>[] = [];

      // Create file watcher for each mod
      for (const mod of mods) {
        const modDownloadPath = path.join(this.downloadPath, mod.modId);
        const promise = this.waitForModDownload(modDownloadPath, mod);
        downloadPromises.push(promise.then((downloadedMod) => {
          if (downloadedMod) {
            downloadedMods.push(downloadedMod);
            this.emit('modDownloaded', downloadedMod);
          }
        }));
      }

      // Always use local steamCmdPath as working directory
      // This ensures mods are downloaded to the same place regardless of SteamCMD location
      const workingDir = this.steamCmdPath;
      
      // Ensure working directory exists
      await fs.mkdir(workingDir, { recursive: true });
      
      console.log(`[Downloader] Starting SteamCMD with working directory: ${workingDir}`);
      console.log(`[Downloader] Download path: ${this.downloadPath}`);
      
      // Start SteamCMD process
      const steamCmdProcess = spawn(steamCmdPath, ['+runscript', scriptPath], {
        cwd: workingDir,
        stdio: this.showWindows ? 'inherit' : 'pipe',
        shell: false
      });

      this.emit('steamStarted');

      // Capture SteamCMD output for debugging
      if (!this.showWindows) {
        steamCmdProcess.stdout?.on('data', (data) => {
          console.log(`[SteamCMD] ${data.toString().trim()}`);
        });
        steamCmdProcess.stderr?.on('data', (data) => {
          console.error(`[SteamCMD] ${data.toString().trim()}`);
        });
      }

      // Wait for SteamCMD to start and login
      await new Promise(resolve => setTimeout(resolve, 3000));

      // Wait for SteamCMD to exit
      await new Promise<void>((resolve, reject) => {
        steamCmdProcess.on('exit', (code) => {
          if (code !== 0 && code !== null) {
            console.error(`[Downloader] SteamCMD exited with code ${code}`);
          } else {
            console.log(`[Downloader] SteamCMD exited successfully`);
          }
          resolve();
        });

        steamCmdProcess.on('error', (error) => {
          console.error(`[Downloader] SteamCMD process error:`, error);
          reject(error);
        });
      });

      // Wait a bit more for file system operations to complete
      await new Promise(resolve => setTimeout(resolve, 2000));

      // Wait for all mod downloads to be detected
      console.log(`[Downloader] Waiting for ${downloadPromises.length} mod(s) to be detected...`);
      await Promise.all(downloadPromises);
      console.log(`[Downloader] Detected ${downloadedMods.length} downloaded mod(s)`);

      this.emit('steamExited');

      console.log(`Finished downloading ${mods.length} mods from workshop. SteamCMD instance closed.`);

      return downloadedMods;
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : String(error);
      console.error(`Error occurred while downloading ${mods.length} mods with SteamCMD:`, errorMessage);
      throw error;
    }
  }

  /**
   * Wait for a mod to be downloaded by watching the download folder
   */
  private async waitForModDownload(
    modDownloadPath: string,
    modInfo: ModDownloadInfo,
    timeout: number = 300000 // 5 minutes timeout
  ): Promise<DownloadedMod | null> {
    return new Promise((resolve) => {
      const startTime = Date.now();
      const checkInterval = 1000; // Check every second

      const checkForMod = async () => {
        try {
          const stats = await fs.stat(modDownloadPath);
          if (stats.isDirectory()) {
            // Mod folder exists, check if it has content
            const files = await fs.readdir(modDownloadPath);
            if (files.length > 0) {
              console.log(`[Downloader] Mod ${modInfo.modId} downloaded successfully to ${modDownloadPath}`);
              const downloadedMod: DownloadedMod = {
                modId: modInfo.modId,
                modPath: modDownloadPath,
                folder: path.basename(modDownloadPath),
                details: {
                  title: modInfo.title,
                  publishedfileid: modInfo.modId
                }
              };
              resolve(downloadedMod);
              return;
            } else {
              console.log(`[Downloader] Mod ${modInfo.modId} folder exists but is empty, waiting...`);
            }
          }
        } catch (error) {
          // Directory doesn't exist yet, continue waiting
          // Only log every 10 seconds to avoid spam
          const elapsed = Date.now() - startTime;
          if (elapsed % 10000 < checkInterval) {
            console.log(`[Downloader] Waiting for mod ${modInfo.modId} at ${modDownloadPath}...`);
          }
        }

        // Check timeout
        if (Date.now() - startTime > timeout) {
          console.warn(`[Downloader] Timeout waiting for mod ${modInfo.modId} to download at ${modDownloadPath}`);
          console.warn(`[Downloader] Checked for ${Math.floor((Date.now() - startTime) / 1000)} seconds`);
          // Check if path exists but is empty
          try {
            const stats = await fs.stat(modDownloadPath);
            if (stats.isDirectory()) {
              const files = await fs.readdir(modDownloadPath);
              console.warn(`[Downloader] Mod folder exists but has ${files.length} files`);
            }
          } catch (e) {
            console.warn(`[Downloader] Mod folder does not exist at ${modDownloadPath}`);
          }
          resolve(null);
          return;
        }

        // Check again after interval
        setTimeout(checkForMod, checkInterval);
      };

      // Start checking
      checkForMod();
    });
  }

  /**
   * Check if a mod is currently being downloaded
   */
  isDownloading(modId: string): boolean {
    return this.activeDownloads.has(modId);
  }

  /**
   * Mark a mod as downloading
   */
  markDownloading(modId: string): void {
    this.activeDownloads.add(modId);
  }

  /**
   * Mark a mod as finished downloading
   */
  markDownloaded(modId: string): void {
    this.activeDownloads.delete(modId);
  }
}

