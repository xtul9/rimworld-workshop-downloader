// Steam API-related commands

use serde_json;
use tauri::command;
use crate::services::get_steam_api;
use crate::core::mod_scanner::query_mod_batch;

/// Get file details from Steam Workshop (optimized - uses batch query internally)
#[command]
pub async fn get_file_details(mod_id: String) -> Result<serde_json::Value, String> {
    // Use batch query for efficiency (even for single mod)
    match query_mod_batch(&[mod_id.clone()], 0).await {
        Ok(mut details) => {
            if let Some(detail) = details.pop() {
                Ok(serde_json::to_value(detail).unwrap())
            } else {
                Err("No file details found".to_string())
            }
        }
        Err(e) => {
            // Fallback to SteamApi if batch query fails
            let steam_api = get_steam_api();
            let details = {
                let mut api = steam_api.lock().await;
                api.get_file_details(&mod_id).await
            }
            .map_err(|_| format!("Failed to fetch file details: {}", e))?;
            
            Ok(serde_json::to_value(details).unwrap())
        }
    }
}

/// Get file details for multiple mods (optimized batch version)
#[command]
pub async fn get_file_details_batch(
    mod_ids: Vec<String>,
) -> Result<serde_json::Value, String> {
    if mod_ids.is_empty() {
        return Ok(serde_json::json!({}));
    }
    
    // Remove duplicates
    let unique_ids: Vec<String> = mod_ids.iter()
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .cloned()
        .collect();
    
    // Query in batches of 50
    const BATCH_SIZE: usize = 50;
    let mut all_details = Vec::new();
    
    for i in (0..unique_ids.len()).step_by(BATCH_SIZE) {
        let batch_end = std::cmp::min(i + BATCH_SIZE, unique_ids.len());
        let batch = &unique_ids[i..batch_end];
        
        match query_mod_batch(batch, 0).await {
            Ok(mut details) => {
                all_details.append(&mut details);
            }
            Err(_) => {
                // If batch query fails, try individual queries with cache
                let steam_api = get_steam_api();
                for mod_id in batch {
                    let mut api = steam_api.lock().await;
                    if let Ok(detail) = api.get_file_details(mod_id).await {
                        all_details.push(detail);
                    }
                }
            }
        }
        
        // Small delay between batches to avoid rate limiting
        if i + BATCH_SIZE < unique_ids.len() {
            tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;
        }
    }
    
    // Build result map
    let mut result_map = serde_json::Map::new();
    for detail in all_details {
        let mod_id = detail.publishedfileid.clone();
        result_map.insert(mod_id, serde_json::to_value(detail).unwrap());
    }
    
    // Add entries for mods that weren't found (with null values)
    for mod_id in unique_ids {
        if !result_map.contains_key(&mod_id) {
            result_map.insert(mod_id, serde_json::Value::Null);
        }
    }
    
    Ok(serde_json::Value::Object(result_map))
}

/// Check if a file is a collection (optimized - uses batch query internally)
#[command]
pub async fn is_collection(mod_id: String) -> Result<serde_json::Value, String> {
    // Use batch query for efficiency (even for single mod)
    match query_mod_batch(&[mod_id.clone()], 0).await {
        Ok(mut details) => {
            if let Some(detail) = details.pop() {
                // Check file_type if available
                let is_collection = detail.file_type == 2;
                
                // If file_type is not available or not 2, try scraping
                if !is_collection && detail.file_type == 0 {
                    let steam_api = get_steam_api();
                    let mut api = steam_api.lock().await;
                    match api.scrape_is_collection(&mod_id).await {
                        Ok(scraped_result) => {
                            Ok(serde_json::json!({
                                "isCollection": scraped_result
                            }))
                        }
                        Err(_) => {
                            Ok(serde_json::json!({
                                "isCollection": false
                            }))
                        }
                    }
                } else {
                    Ok(serde_json::json!({
                        "isCollection": is_collection
                    }))
                }
            } else {
                // Fallback to SteamApi if batch query returns no results
                let steam_api = get_steam_api();
                let is_collection = {
                    let mut api = steam_api.lock().await;
                    api.is_collection(&mod_id).await
                }
                .map_err(|e| format!("Failed to check if collection: {}", e))?;
                
                Ok(serde_json::json!({
                    "isCollection": is_collection
                }))
            }
        }
        Err(_) => {
            // Fallback to SteamApi if batch query fails
            let steam_api = get_steam_api();
            let is_collection = {
                let mut api = steam_api.lock().await;
                api.is_collection(&mod_id).await
            }
            .map_err(|e| format!("Failed to check if collection: {}", e))?;
            
            Ok(serde_json::json!({
                "isCollection": is_collection
            }))
        }
    }
}

/// Check if multiple files are collections (optimized batch version)
#[command]
pub async fn is_collection_batch(
    mod_ids: Vec<String>,
) -> Result<serde_json::Value, String> {
    if mod_ids.is_empty() {
        return Ok(serde_json::json!({}));
    }
    
    // Remove duplicates
    let unique_ids: Vec<String> = mod_ids.iter()
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .cloned()
        .collect();
    
    // Query details in batches of 50 in parallel
    const BATCH_SIZE: usize = 50;
    let mut batch_futures = Vec::new();
    
    for batch_idx in 0..(unique_ids.len() + BATCH_SIZE - 1) / BATCH_SIZE {
        let start = batch_idx * BATCH_SIZE;
        let end = std::cmp::min(start + BATCH_SIZE, unique_ids.len());
        let batch: Vec<String> = unique_ids[start..end].iter().cloned().collect();
        let steam_api = get_steam_api();
        
        let future = async move {
            // Small delay to stagger requests
            if batch_idx > 0 {
                tokio::time::sleep(tokio::time::Duration::from_millis(100 * batch_idx as u64)).await;
            }
            
            match query_mod_batch(&batch, 0).await {
                Ok(details) => Ok(details),
                Err(_) => {
                    // If batch query fails, try individual queries with cache
                    let mut api = steam_api.lock().await;
                    let mut fallback_details = Vec::new();
                    for mod_id in &batch {
                        if let Ok(detail) = api.get_file_details(mod_id).await {
                            fallback_details.push(detail);
                        }
                    }
                    Ok(fallback_details)
                }
            }
        };
        batch_futures.push(future);
    }
    
    // Wait for all batches in parallel
    let batch_results: Vec<Result<Vec<crate::core::mod_scanner::WorkshopFileDetails>, Box<dyn std::error::Error + Send + Sync>>> = 
        futures::future::join_all(batch_futures).await;
    let mut all_details = Vec::new();
    for result in batch_results {
        if let Ok(mut details) = result {
            all_details.append(&mut details);
        }
    }
    
    // Build details map
    let details_map: std::collections::HashMap<String, crate::core::mod_scanner::WorkshopFileDetails> = 
        all_details.into_iter()
            .map(|d| (d.publishedfileid.clone(), d))
            .collect();
    
    // Check which mods need scraping (file_type == 0)
    let mut mods_to_scrape = Vec::new();
    let mut result_map = serde_json::Map::new();
    
    for mod_id in &unique_ids {
        if let Some(detail) = details_map.get(mod_id) {
            let is_collection = detail.file_type == 2;
            
            if is_collection {
                result_map.insert(mod_id.clone(), serde_json::json!({
                    "isCollection": true
                }));
            } else if detail.file_type == 0 {
                // Need to scrape
                mods_to_scrape.push(mod_id.clone());
            } else {
                result_map.insert(mod_id.clone(), serde_json::json!({
                    "isCollection": false
                }));
            }
        } else {
            // Mod not found
            result_map.insert(mod_id.clone(), serde_json::json!({
                "isCollection": false
            }));
        }
    }
    
    // Scrape mods that need it (in parallel)
    if !mods_to_scrape.is_empty() {
        let mut scrape_futures = Vec::new();
        
        for mod_id in mods_to_scrape {
            let mod_id_clone = mod_id.clone();
            let future = async move {
                let steam_api = get_steam_api();
                let mut api = steam_api.lock().await;
                match api.scrape_is_collection(&mod_id_clone).await {
                    Ok(result) => (mod_id_clone, result),
                    Err(_) => (mod_id_clone, false),
                }
            };
            scrape_futures.push(future);
        }
        
        let scrape_results = futures::future::join_all(scrape_futures).await;
        for (mod_id, is_collection) in scrape_results {
            result_map.insert(mod_id, serde_json::json!({
                "isCollection": is_collection
            }));
        }
    }
    
    // Add entries for mods that weren't found (with false)
    for mod_id in unique_ids {
        if !result_map.contains_key(&mod_id) {
            result_map.insert(mod_id, serde_json::json!({
                "isCollection": false
            }));
        }
    }
    
    Ok(serde_json::Value::Object(result_map))
}

/// Get collection details (list of mods in collection)
#[command]
pub async fn get_collection_details(collection_id: String) -> Result<Vec<serde_json::Value>, String> {
    let steam_api = get_steam_api();
    let details = {
        let mut api = steam_api.lock().await;
        api.get_collection_details(&collection_id).await
    }
    .map_err(|e| format!("Failed to fetch collection details: {}", e))?;
    
    Ok(details.into_iter()
        .map(|d| serde_json::to_value(d).unwrap())
        .collect())
}

/// Get collection details for multiple collections (optimized batch version)
#[command]
pub async fn get_collection_details_batch(
    collection_ids: Vec<String>,
) -> Result<serde_json::Value, String> {
    if collection_ids.is_empty() {
        return Ok(serde_json::json!({}));
    }
    
    // Remove duplicates
    let unique_ids: Vec<String> = collection_ids.iter()
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .cloned()
        .collect();
    
    // Fetch collection details in parallel (each call checks cache first via SteamApi::get_collection_details)
    let mut collection_futures = Vec::new();
    for collection_id in &unique_ids {
        let collection_id_clone = collection_id.clone();
        let future = async move {
            let steam_api = get_steam_api();
            let mut api = steam_api.lock().await;
            match api.get_collection_details(&collection_id_clone).await {
                Ok(details) => (collection_id_clone, details),
                Err(_) => (collection_id_clone, vec![]),
            }
        };
        collection_futures.push(future);
    }
    
    let collection_results = futures::future::join_all(collection_futures).await;
    
    // Build result map: collection_id -> array of mod details
    let mut result_map = serde_json::Map::new();
    for (collection_id, details) in collection_results {
        let collection_mods: Vec<serde_json::Value> = details.into_iter()
            .map(|d| serde_json::to_value(d).unwrap())
            .collect();
        
        result_map.insert(collection_id, serde_json::Value::Array(collection_mods));
    }
    
    Ok(serde_json::Value::Object(result_map))
}

