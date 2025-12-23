use std::time::{Duration, Instant};
use tokio::time::sleep;

/// Rate limiter to prevent being blocked by servers
pub struct RateLimiter {
    last_request_time: Instant,
    min_delay: Duration,
}

impl RateLimiter {
    /// Create a new rate limiter with minimum delay between requests
    pub fn new(min_delay: Duration) -> Self {
        Self {
            last_request_time: Instant::now() - min_delay, // Allow first request immediately
            min_delay,
        }
    }

    /// Wait for the rate limit delay
    pub async fn wait(&mut self) {
        let now = Instant::now();
        let time_since_last_request = now.duration_since(self.last_request_time);
        
        if time_since_last_request < self.min_delay {
            let wait_time = self.min_delay - time_since_last_request;
            sleep(wait_time).await;
        }
        
        self.last_request_time = Instant::now();
    }

    /// Execute a function with rate limiting
    pub async fn execute<F, Fut, T>(&mut self, f: F) -> T
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = T>,
    {
        self.wait().await;
        f().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limiter_delay() {
        let mut limiter = RateLimiter::new(Duration::from_millis(100));
        let start = Instant::now();
        
        limiter.wait().await; // First call should be immediate
        let first_duration = start.elapsed();
        assert!(first_duration < Duration::from_millis(50));
        
        let start = Instant::now();
        limiter.wait().await; // Second call should wait
        let second_duration = start.elapsed();
        assert!(second_duration >= Duration::from_millis(90));
    }

    #[tokio::test]
    async fn test_rate_limiter_execute() {
        let mut limiter = RateLimiter::new(Duration::from_millis(50));
        
        let result = limiter.execute(|| async {
            42
        }).await;
        
        assert_eq!(result, 42);
    }
}

