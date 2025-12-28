use crate::core::mod_scanner::WorkshopFileDetails;
use crate::core::api_cache::Cache;
use crate::core::api_rate_limiter::RateLimiter;
use std::time::Duration;

const STEAM_API_BASE: &str = "http://api.steampowered.com";
const USER_AGENT: &str = "RimworldWorkshopDownloader/1.0";

pub struct SteamApi {
    file_details_cache: Cache<WorkshopFileDetails>,
    is_collection_cache: Cache<bool>,
    collection_details_cache: Cache<Vec<WorkshopFileDetails>>,
    scraping_rate_limiter: RateLimiter,
}

impl SteamApi {
    pub fn new() -> Self {
        Self {
            file_details_cache: Cache::new(Duration::from_secs(3600)), // 1 hour
            is_collection_cache: Cache::new(Duration::from_secs(3600)), // 1 hour
            collection_details_cache: Cache::new(Duration::from_secs(3600)), // 1 hour
            scraping_rate_limiter: RateLimiter::new(Duration::from_millis(2000)), // 2 seconds
        }
    }

    /// Get file details from Steam Workshop
    pub async fn get_file_details(&mut self, mod_id: &str) -> Result<WorkshopFileDetails, Box<dyn std::error::Error>> {
        // Check cache first
        let cache_key = format!("file-details-{}", mod_id);
        if let Some(cached) = self.file_details_cache.get(&cache_key) {
            return Ok(cached.clone());
        }

        let url = format!("{}/ISteamRemoteStorage/GetPublishedFileDetails/v0001/", STEAM_API_BASE);
        let client = reqwest::Client::new();
        
        let mut params = std::collections::HashMap::new();
        params.insert("itemcount", "1");
        params.insert("publishedfileids[0]", mod_id);
        params.insert("format", "json");

        let response = client
            .post(&url)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("User-Agent", USER_AGENT)
            .header("Accept", "application/json")
            .form(&params)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(format!("Steam API error: {}", response.status()).into());
        }

        let data: serde_json::Value = response.json().await?;
        let file_details = data["response"]["publishedfiledetails"]
            .as_array()
            .and_then(|arr| arr.first())
            .ok_or("No file details found")?;

        let details: WorkshopFileDetails = serde_json::from_value(file_details.clone())?;

        // Cache the result
        self.file_details_cache.set(cache_key, details.clone(), None);

        Ok(details)
    }

    /// Check if a file is a collection
    pub async fn is_collection(&mut self, mod_id: &str) -> Result<bool, Box<dyn std::error::Error>> {
        // Check cache first
        let cache_key = format!("is-collection-{}", mod_id);
        if let Some(cached) = self.is_collection_cache.get(&cache_key) {
            return Ok(*cached);
        }

        // Get file details first
        let details = self.get_file_details(mod_id).await?;

        // Check file_type if available
        let mut is_collection = details.file_type == 2;

        // If file_type is not available or not 2, try scraping
        if !is_collection && details.file_type == 0 {
            is_collection = self.scrape_is_collection(mod_id).await?;
        }

        // Cache the result
        self.is_collection_cache.set(cache_key, is_collection, None);

        Ok(is_collection)
    }

    /// Scrape Steam Workshop page to check if it's a collection
    pub async fn scrape_is_collection(&mut self, mod_id: &str) -> Result<bool, Box<dyn std::error::Error>> {
        let workshop_url = format!("https://steamcommunity.com/sharedfiles/filedetails/?id={}", mod_id);
        
        let page_html = self.scraping_rate_limiter.execute(|| async {
            let client = reqwest::Client::new();
            let response = client
                .get(&workshop_url)
                .header("User-Agent", USER_AGENT)
                .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
                .header("Accept-Language", "en-US,en;q=0.5")
                .send()
                .await?;
            Ok::<String, Box<dyn std::error::Error>>(response.text().await?)
        }).await?;

        // Check for collection-specific HTML elements
        let has_main_contents_collection = page_html.contains("mainContentsCollection") || page_html.contains("id=\"mainContentsCollection\"");
        let has_collection_header = page_html.contains("collectionHeader") || page_html.contains("class=\"collectionHeader\"");
        let has_subscribe_collection_btn = page_html.contains("SubscribeCollectionBtn") || page_html.contains("Subscribe to Collection");
        let has_subscribe_all_btn = page_html.contains("SubscribeAllBtn") || page_html.contains("Subscribe to all");

        Ok(has_main_contents_collection || has_collection_header || has_subscribe_collection_btn || has_subscribe_all_btn)
    }

    /// Get collection details (list of mods in collection)
    pub async fn get_collection_details(&mut self, collection_id: &str) -> Result<Vec<WorkshopFileDetails>, Box<dyn std::error::Error>> {
        // Check cache first
        let cache_key = format!("collection-details-{}", collection_id);
        if let Some(cached) = self.collection_details_cache.get(&cache_key) {
            return Ok(cached.clone());
        }

        // Scrape collection page to get mod IDs
        let mod_ids = self.scrape_collection_mod_ids(collection_id).await?;

        if mod_ids.is_empty() {
            self.collection_details_cache.set(cache_key, vec![], None);
            return Ok(vec![]);
        }

        // Fetch details for all mods in collection using batch query
        let all_details = match crate::core::mod_scanner::query_mod_batch(&mod_ids, 0).await {
            Ok(details) => details,
            Err(_) => {
                // Fallback to individual queries if batch fails
                let mut fallback_details = Vec::new();
                for mod_id in &mod_ids {
                    if let Ok(detail) = self.get_file_details(mod_id).await {
                        fallback_details.push(detail);
                    }
                }
                fallback_details
            }
        };

        // Cache the result
        self.collection_details_cache.set(cache_key, all_details.clone(), None);

        Ok(all_details)
    }

    /// Scrape collection page to extract mod IDs
    pub async fn scrape_collection_mod_ids(&mut self, collection_id: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let workshop_url = format!("https://steamcommunity.com/sharedfiles/filedetails/?id={}", collection_id);
        
        let page_html = self.scraping_rate_limiter.execute(|| async {
            let client = reqwest::Client::new();
            let response = client
                .get(&workshop_url)
                .header("User-Agent", USER_AGENT)
                .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
                .header("Accept-Language", "en-US,en;q=0.5")
                .send()
                .await?;
            Ok::<String, Box<dyn std::error::Error>>(response.text().await?)
        }).await?;

        // Parse mod IDs from HTML (look for sharedfiles/filedetails/?id=XXXXX)
        let mut mod_ids = std::collections::HashSet::new();
        let re = regex::Regex::new(r"sharedfiles/filedetails/\?id=(\d+)")?;
        
        for cap in re.captures_iter(&page_html) {
            if let Some(id) = cap.get(1) {
                let id_str = id.as_str();
                // Filter out the collection ID itself
                if id_str != collection_id {
                    mod_ids.insert(id_str.to_string());
                }
            }
        }

        Ok(mod_ids.into_iter().collect())
    }
}

impl Default for SteamApi {
    fn default() -> Self {
        Self::new()
    }
}

