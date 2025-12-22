import https from 'https';
import { createWriteStream } from 'fs';
import { execSync } from 'child_process';
import * as fs from 'fs/promises';
import * as path from 'path';

const platform = process.platform;
const arch = process.arch;

const steamCmdUrls = {
  'linux': {
    'x64': 'https://steamcdn-a.akamaihd.net/client/installer/steamcmd_linux.tar.gz',
    'arm64': 'https://steamcdn-a.akamaihd.net/client/installer/steamcmd_linux.tar.gz'
  },
  'win32': {
    'x64': [
      'https://steamcdn-a.akamaihd.net/client/installer/steamcmd.zip',
      'https://steamcdn-a.akamaihd.net/client/installer/steamcmd_win32.zip'
    ],
    'ia32': [
      'https://steamcdn-a.akamaihd.net/client/installer/steamcmd.zip',
      'https://steamcdn-a.akamaihd.net/client/installer/steamcmd_win32.zip'
    ]
  },
  'darwin': {
    'x64': 'https://steamcdn-a.akamaihd.net/client/installer/steamcmd_osx.tar.gz',
    'arm64': 'https://steamcdn-a.akamaihd.net/client/installer/steamcmd_osx.tar.gz'
  }
};

function downloadFile(url, outputPath) {
  return new Promise((resolve, reject) => {
    const file = createWriteStream(outputPath);
    const options = {
      headers: {
        'User-Agent': 'RimworldWorkshopDownloader/1.0'
      }
    };
    
    https.get(url, options, (response) => {
      if (response.statusCode === 302 || response.statusCode === 301 || response.statusCode === 307 || response.statusCode === 308) {
        // Follow redirect
        const redirectUrl = response.headers.location;
        if (!redirectUrl) {
          reject(new Error(`Redirect received but no location header: ${response.statusCode}`));
          return;
        }
        // Handle relative redirects
        const absoluteUrl = redirectUrl.startsWith('http') 
          ? redirectUrl 
          : new URL(redirectUrl, url).toString();
        return downloadFile(absoluteUrl, outputPath).then(resolve).catch(reject);
      }
      if (response.statusCode !== 200) {
        file.close();
        fs.unlink(outputPath).catch(() => {});
        reject(new Error(`Failed to download: ${response.statusCode} ${response.statusMessage || ''}`));
        return;
      }
      response.pipe(file);
      file.on('finish', () => {
        file.close();
        resolve();
      });
    }).on('error', (err) => {
      file.close();
      fs.unlink(outputPath).catch(() => {});
      reject(err);
    });
  });
}

async function extractTarGz(tarGzPath, outputDir) {
  await fs.mkdir(outputDir, { recursive: true });
  execSync(`tar -xzf "${tarGzPath}" -C "${outputDir}"`, { stdio: 'inherit' });
}

async function extractZip(zipPath, outputDir) {
  await fs.mkdir(outputDir, { recursive: true });
  // On Windows, use PowerShell; on Linux/Mac, use unzip if available
  if (platform === 'win32') {
    execSync(`powershell -Command "Expand-Archive -Path '${zipPath}' -DestinationPath '${outputDir}' -Force"`, { stdio: 'inherit' });
  } else {
    execSync(`unzip -o "${zipPath}" -d "${outputDir}"`, { stdio: 'inherit' });
  }
}

async function main() {
  let urlConfig = steamCmdUrls[platform]?.[arch] || steamCmdUrls[platform]?.['x64'];
  if (!urlConfig) {
    throw new Error(`Unsupported platform: ${platform} ${arch}`);
  }

  // Handle both single URL strings and arrays of URLs (for fallback)
  const urls = Array.isArray(urlConfig) ? urlConfig : [urlConfig];
  
  const binDir = path.join(process.cwd(), 'bin', 'steamcmd');
  await fs.mkdir(binDir, { recursive: true });

  const isZip = urls[0].endsWith('.zip');
  const archiveName = isZip ? 'steamcmd.zip' : 'steamcmd.tar.gz';
  const archivePath = path.join(binDir, archiveName);

  // Try each URL until one works
  let downloadSuccess = false;
  let lastError = null;
  
  for (const url of urls) {
    try {
      console.log(`Downloading SteamCMD from ${url}...`);
      await downloadFile(url, archivePath);
      downloadSuccess = true;
      break;
    } catch (error) {
      console.warn(`Failed to download from ${url}: ${error.message}`);
      lastError = error;
      // Clean up failed download
      await fs.unlink(archivePath).catch(() => {});
      continue;
    }
  }

  if (!downloadSuccess) {
    throw new Error(`Failed to download SteamCMD from all URLs. Last error: ${lastError?.message || 'Unknown error'}`);
  }

  console.log(`Extracting SteamCMD...`);
  if (isZip) {
    await extractZip(archivePath, binDir);
  } else {
    await extractTarGz(archivePath, binDir);
  }

  // Find steamcmd executable and move it to bin/steamcmd/
  const steamcmdExe = platform === 'win32' ? 'steamcmd.exe' : 'steamcmd';
  
  // On Linux, steamcmd is in linux32/ or linux64/ subdirectory
  // On Mac, steamcmd is usually in the root
  // On Windows, it's in steamcmd/ subdirectory
  // Try multiple possible locations
  const possibleSourcePaths = [];
  
  if (platform === 'linux') {
    // Linux: check linux32 and linux64 subdirectories first
    possibleSourcePaths.push(
      path.join(binDir, 'linux32', steamcmdExe),
      path.join(binDir, 'linux64', steamcmdExe)
    );
  } else if (platform === 'win32') {
    // Windows: check steamcmd subdirectory
    possibleSourcePaths.push(
      path.join(binDir, 'steamcmd', steamcmdExe)
    );
  }
  
  // Common fallbacks
  possibleSourcePaths.push(
    path.join(binDir, steamcmdExe),  // Direct in binDir
    path.join(binDir, '..', 'steamcmd', steamcmdExe)  // In parent/steamcmd
  );

  let sourcePath = null;
  for (const possiblePath of possibleSourcePaths) {
    try {
      await fs.access(possiblePath);
      sourcePath = possiblePath;
      console.log(`Found SteamCMD at: ${sourcePath}`);
      break;
    } catch {
      continue;
    }
  }

  if (!sourcePath) {
    // List directory contents to help debug
    try {
      const files = await fs.readdir(binDir);
      console.log(`Files in ${binDir}:`, files);
      // Also check subdirectories
      const entries = await fs.readdir(binDir, { withFileTypes: true });
      for (const entry of entries) {
        if (entry.isDirectory()) {
          const subFiles = await fs.readdir(path.join(binDir, entry.name));
          console.log(`Files in ${entry.name}:`, subFiles);
        }
      }
    } catch (err) {
      console.error('Error listing directory:', err);
    }
    throw new Error(`SteamCMD executable not found after extraction. Expected: ${steamcmdExe}`);
  }

  const targetPath = path.join(binDir, steamcmdExe);
  
  // If already in right place, skip
  if (sourcePath !== targetPath) {
    try {
      await fs.rename(sourcePath, targetPath);
      console.log(`Moved SteamCMD from ${sourcePath} to ${targetPath}`);
    } catch (error) {
      // If rename fails, try copy
      console.log(`Rename failed, trying copy...`);
      await fs.copyFile(sourcePath, targetPath);
      console.log(`Copied SteamCMD from ${sourcePath} to ${targetPath}`);
    }
  }

  // Verify target exists before chmod
  try {
    await fs.access(targetPath);
    // Make executable on Unix
    if (platform !== 'win32') {
      await fs.chmod(targetPath, 0o755);
      console.log(`Set executable permissions on ${targetPath}`);
    }
  } catch (error) {
    throw new Error(`Failed to access target SteamCMD at ${targetPath}: ${error}`);
  }

  // Clean up archive
  await fs.unlink(archivePath).catch(() => {});

  // Clean up extracted directory if it exists
  const extractedDirPath = path.join(binDir, 'steamcmd');
  try {
    const stat = await fs.stat(extractedDirPath);
    if (stat.isDirectory() && extractedDirPath !== binDir) {
      // Remove extracted directory
      await fs.rm(extractedDirPath, { recursive: true, force: true });
    }
  } catch {}

  // Rename to match Tauri externalBin naming convention
  // Tauri automatically adds target triple suffix, so we use base name
  // But we need to create platform-specific versions for build
  const finalName = platform === 'win32' ? 'steamcmd.exe' : 'steamcmd';
  const finalPath = path.join(binDir, finalName);
  
  // If target is different from current, rename
  if (targetPath !== finalPath) {
    try {
      await fs.rename(targetPath, finalPath);
    } catch (error) {
      // Already correct name or copy failed, try copy
      try {
        await fs.copyFile(targetPath, finalPath);
      } catch (copyError) {
        // Ignore
      }
    }
  }

  // For Tauri, we need to create copies with target triple suffix for each platform
  // This allows building for multiple platforms from one machine
  const targetTriples = {
    'linux': ['x86_64-unknown-linux-gnu'],
    'win32': ['x86_64-pc-windows-msvc'],
    'darwin': ['x86_64-apple-darwin', 'aarch64-apple-darwin']
  };

  const triples = targetTriples[platform] || [];
  for (const triple of triples) {
    const suffixName = platform === 'win32' 
      ? `steamcmd-${triple}.exe`
      : `steamcmd-${triple}`;
    const suffixPath = path.join(binDir, suffixName);
    
    // Copy to create platform-specific version
    try {
      await fs.copyFile(finalPath, suffixPath);
      if (platform !== 'win32') {
        await fs.chmod(suffixPath, 0o755);
      }
      console.log(`Created ${suffixName} for Tauri bundle`);
    } catch (error) {
      console.warn(`Failed to create ${suffixName}:`, error);
    }
  }

  console.log(`SteamCMD downloaded to ${finalPath}`);
}

main().catch((error) => {
  console.error('Error downloading SteamCMD:', error);
  process.exit(1);
});

