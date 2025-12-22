import { Router } from 'express';
import { Downloader, ModDownloadInfo } from '../services/downloader.js';
import { ModUpdater } from '../services/modUpdater.js';
import { Cache } from '../services/cache.js';
import { RateLimiter } from '../services/rateLimiter.js';
import * as path from 'path';
import * as fs from 'fs/promises';

export const workshopRouter = Router();

// Shared downloader instance to track active downloads
const downloader = new Downloader();

// Cache for API responses (1 hour TTL)
const fileDetailsCache = new Cache<any>(3600000); // 1 hour
const isCollectionCache = new Cache<boolean>(3600000); // 1 hour
const collectionDetailsCache = new Cache<any[]>(3600000); // 1 hour

// Rate limiter for scraping (2 seconds between requests to be safe)
const scrapingRateLimiter = new RateLimiter(2000);

// User agent to avoid being blocked
const USER_AGENT = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36';

interface SteamPublishedFileDetails {
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

interface SteamAPIResponse {
  response: {
    result: number;
    resultcount: number;
    publishedfiledetails: SteamPublishedFileDetails[];
  };
}

// Steam Workshop API endpoints (no API key needed for anonymous access)
const STEAM_API_BASE = 'http://api.steampowered.com';

/**
 * Get file details from Steam Workshop
 */
workshopRouter.get('/file-details', async (req, res) => {
  try {
    const modId = req.query.id as string;
    if (!modId) {
      return res.status(400).json({ error: 'Mod ID is required' });
    }

    // Check cache first
    const cacheKey = `file-details-${modId}`;
    const cached = fileDetailsCache.get(cacheKey);
    if (cached) {
      return res.json(cached);
    }

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
        'publishedfileids[0]': modId,
        format: 'json',
      }),
    });

    if (!response.ok) {
      throw new Error(`Steam API error: ${response.statusText}`);
    }

    const data = await response.json() as SteamAPIResponse;
    const fileDetails = data.response?.publishedfiledetails?.[0];

    if (!fileDetails) {
      return res.status(404).json({ error: 'Mod not found' });
    }

    // Cache the result
    fileDetailsCache.set(cacheKey, fileDetails);

    res.json(fileDetails);
  } catch (error) {
    console.error('Error fetching file details:', error);
    res.status(500).json({ 
      error: 'Failed to fetch file details',
      message: error instanceof Error ? error.message : String(error)
    });
  }
});

/**
 * Check if a file is a collection
 */
workshopRouter.get('/is-collection', async (req, res) => {
  try {
    const modId = req.query.id as string;
    if (!modId) {
      return res.status(400).json({ error: 'Mod ID is required' });
    }

    // Check cache first
    const cacheKey = `is-collection-${modId}`;
    const cached = isCollectionCache.get(cacheKey);
    if (cached !== null) {
      return res.json({ isCollection: cached });
    }

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
        'publishedfileids[0]': modId,
        format: 'json',
      }),
    });

    if (!response.ok) {
      throw new Error(`Steam API error: ${response.statusText}`);
    }

    const data = await response.json() as SteamAPIResponse;
    const fileDetails = data.response?.publishedfiledetails?.[0];

    if (!fileDetails) {
      return res.status(404).json({ error: 'Mod not found' });
    }

    // Collections have file_type = 2, but API might not return it
    // Alternative: check if file_type exists and equals 2, or try to detect by other means
    let isCollection = false;
    
    if (fileDetails.file_type !== undefined) {
      // Use file_type if available
      isCollection = fileDetails.file_type === 2;
    } else {
      // Alternative detection: scrape the page to check for collection-specific HTML structure
      // Collections have unique HTML elements that single mods don't have
      // Use rate limiter to avoid being blocked
      try {
        const workshopUrl = `https://steamcommunity.com/sharedfiles/filedetails/?id=${modId}`;
        
        // Use rate limiter for scraping
        const pageResponse = await scrapingRateLimiter.execute(async () => {
          return await fetch(workshopUrl, {
            headers: {
              'User-Agent': 'RimworldWorkshopDownloader/1.0',
              'Accept': 'text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8',
              'Accept-Language': 'en-US,en;q=0.5',
            },
          });
        });
        
        const pageHtml = await pageResponse.text();
        
        // Most reliable indicators of a collection:
        // 1. mainContentsCollection div (collections use this, single mods use mainContents)
        // 2. collectionHeader div (only on collections)
        // 3. SubscribeCollectionBtn (Subscribe to Collection button, not "Add to Collection")
        // 4. SubscribeAllBtn (Subscribe to all button)
        
        const hasMainContentsCollection = /mainContentsCollection|id="mainContentsCollection"/i.test(pageHtml);
        const hasCollectionHeader = /collectionHeader|class="collectionHeader"/i.test(pageHtml);
        const hasSubscribeCollectionBtn = /SubscribeCollectionBtn|Subscribe to Collection[^"]*btn/i.test(pageHtml);
        const hasSubscribeAllBtn = /SubscribeAllBtn|Subscribe to all[^"]*btn/i.test(pageHtml);
        
        // Only mark as collection if we find collection-specific structure
        // Avoid false positives from "Add to Collection" links on single mods
        isCollection = hasMainContentsCollection || hasCollectionHeader || hasSubscribeCollectionBtn || hasSubscribeAllBtn;
        
      } catch (scrapeError) {
        console.warn('Failed to detect collection via scraping:', scrapeError);
        // Fallback: assume not a collection if we can't determine
        isCollection = false;
      }
    }

    // Cache the result
    isCollectionCache.set(cacheKey, isCollection);

    res.json({ isCollection });
  } catch (error) {
    console.error('Error checking if collection:', error);
    res.status(500).json({ 
      error: 'Failed to check if collection',
      message: error instanceof Error ? error.message : String(error)
    });
  }
});

/**
 * Get collection details (list of mods in collection)
 * Note: Steam API doesn't provide collection children directly.
 * We need to scrape the Steam Workshop page or use Steam Web API with collection details.
 * For now, we'll try to parse from the collection's metadata or return empty array.
 */
workshopRouter.get('/collection-details', async (req, res) => {
  try {
    const modId = req.query.id as string;
    if (!modId) {
      return res.status(400).json({ error: 'Collection ID is required' });
    }

    // Check cache first
    const cacheKey = `collection-details-${modId}`;
    const cached = collectionDetailsCache.get(cacheKey);
    if (cached) {
      return res.json(cached);
    }

    // Get collection details
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
        'publishedfileids[0]': modId,
        format: 'json',
      }),
    });

    if (!response.ok) {
      throw new Error(`Steam API error: ${response.statusText}`);
    }

    const data = await response.json() as SteamAPIResponse;
    const collectionDetails = data.response?.publishedfiledetails?.[0];

    if (!collectionDetails) {
      return res.status(404).json({ error: 'Collection not found' });
    }

    // Try to get collection children from Steam Workshop page
    // This is a workaround since Steam API doesn't provide collection children directly
    // Use rate limiter to avoid being blocked
    try {
      const workshopUrl = `https://steamcommunity.com/sharedfiles/filedetails/?id=${modId}`;
      
      // Use rate limiter for scraping
      const pageResponse = await scrapingRateLimiter.execute(async () => {
        return await fetch(workshopUrl, {
          headers: {
            'User-Agent': 'RimworldWorkshopDownloader/1.0',
            'Accept': 'text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8',
            'Accept-Language': 'en-US,en;q=0.5',
          },
        });
      });
      
      const pageHtml = await pageResponse.text();

      // Parse mod IDs from the page (they're in a specific format)
      // Look for workshop item IDs in the HTML
      const modIdRegex = /sharedfiles\/filedetails\/\?id=(\d+)/g;
      const modIds: string[] = [];
      let match;
      const seenIds = new Set<string>();

      while ((match = modIdRegex.exec(pageHtml)) !== null) {
        const id = match[1];
        // Filter out the collection ID itself and duplicates
        if (id !== modId && !seenIds.has(id)) {
          seenIds.add(id);
          modIds.push(id);
        }
      }

      // If we found mod IDs, fetch their details
      if (modIds.length > 0) {
        const detailsUrl = `${STEAM_API_BASE}/ISteamRemoteStorage/GetPublishedFileDetails/v0001/`;
        
        // Build URLSearchParams for multiple mod IDs
        const params = new URLSearchParams({
          itemcount: modIds.length.toString(),
          format: 'json',
        });
        
        modIds.forEach((id, index) => {
          params.append(`publishedfileids[${index}]`, id);
        });

        const detailsResponse = await fetch(detailsUrl, {
          method: 'POST',
          headers: {
            'Content-Type': 'application/x-www-form-urlencoded',
            'User-Agent': 'RimworldWorkshopDownloader/1.0',
        'Accept': 'application/json',
          },
          body: params,
        });

        if (detailsResponse.ok) {
          const detailsData = await detailsResponse.json() as SteamAPIResponse;
          const files = detailsData.response?.publishedfiledetails || [];
          
          // Cache the result
          collectionDetailsCache.set(cacheKey, files);
          
          res.json(files);
          return;
        }
      }
    } catch (scrapeError) {
      console.warn('Failed to scrape collection mods from Steam page:', scrapeError);
    }

    // Fallback: return empty array and cache it
    collectionDetailsCache.set(cacheKey, []);
    res.json([]);
  } catch (error) {
    console.error('Error fetching collection details:', error);
    res.status(500).json({ 
      error: 'Failed to fetch collection details',
      message: error instanceof Error ? error.message : String(error)
    });
  }
});

/**
 * Download mod(s) from Steam Workshop
 */
workshopRouter.post('/download', async (req, res) => {
  try {
    const { modId, title, modsPath } = req.body;

    if (!modId || !modsPath) {
      return res.status(400).json({ error: 'modId and modsPath are required' });
    }

    // Check if mod is already downloading
    if (downloader.isDownloading(modId)) {
      return res.status(409).json({ error: 'Mod is already being downloaded' });
    }

    downloader.markDownloading(modId);

    try {
      const modInfo: ModDownloadInfo = {
        modId,
        title: title || modId,
        modsPath
      };

      // Download mod
      const downloadedMods = await downloader.downloadMods([modInfo]);

      if (downloadedMods.length === 0) {
        throw new Error('Mod download completed but no mod folder was created');
      }

      const downloadedMod = downloadedMods[0];

      // Copy mod to mods folder
      const modUpdater = new ModUpdater();
      const downloadPath = path.join(process.cwd(), 'steamcmd', 'steamapps', 'workshop', 'content', '294100');
      const modPath = await modUpdater.updateMod(downloadedMod, downloadPath, modsPath, undefined, false);

      // Get mod details to retrieve time_updated for .lastupdated file
      let timeUpdated: number;
      const cacheKey = `file-details-${modId}`;
      const cachedDetails = fileDetailsCache.get(cacheKey);
      
      if (cachedDetails && cachedDetails.time_updated) {
        timeUpdated = cachedDetails.time_updated;
      } else {
        // Fetch from API if not in cache
        try {
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
              'publishedfileids[0]': modId,
              format: 'json',
            }),
          });

          if (response.ok) {
            const data = await response.json() as SteamAPIResponse;
            const fileDetails = data.response?.publishedfiledetails?.[0];
            if (fileDetails && fileDetails.time_updated) {
              timeUpdated = fileDetails.time_updated;
              // Cache the result
              fileDetailsCache.set(cacheKey, fileDetails);
            } else {
              // Fallback to current time if API doesn't return time_updated
              timeUpdated = Math.floor(Date.now() / 1000);
            }
          } else {
            // Fallback to current time if API request fails
            timeUpdated = Math.floor(Date.now() / 1000);
          }
        } catch (error) {
          console.warn(`[DOWNLOAD] Failed to fetch mod details for .lastupdated file:`, error);
          // Fallback to current time
          timeUpdated = Math.floor(Date.now() / 1000);
        }
      }

      // Create .lastupdated file
      const timestamp = timeUpdated.toString();
      const aboutPath = path.join(modPath, 'About');
      const lastUpdatedPath = path.join(aboutPath, '.lastupdated');
      try {
        // Ensure About directory exists
        await fs.mkdir(aboutPath, { recursive: true });
        await fs.writeFile(lastUpdatedPath, timestamp, 'utf-8');
        console.log(`[DOWNLOAD] Wrote .lastupdated file for mod ${modId} in folder ${path.basename(modPath)} with timestamp ${timestamp}`);
      } catch (error) {
        console.error(`[DOWNLOAD] Failed to write .lastupdated file for mod ${modId}:`, error);
        // Don't fail the request if .lastupdated file can't be created
      }

      downloader.markDownloaded(modId);

      res.json({
        modId: downloadedMod.modId,
        modPath,
        folder: downloadedMod.folder,
        details: downloadedMod.details
      });
    } catch (error) {
      downloader.markDownloaded(modId);
      throw error;
    }
  } catch (error) {
    console.error('Error downloading mod:', error);
    res.status(500).json({ 
      error: 'Failed to download mod',
      message: error instanceof Error ? error.message : String(error)
    });
  }
});

// Export caches for cleanup and management
export { fileDetailsCache, isCollectionCache, collectionDetailsCache };

