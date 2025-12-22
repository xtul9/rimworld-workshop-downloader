/**
 * Rate limiter to prevent being blocked by Steam servers
 */
export class RateLimiter {
  private lastRequestTime: number = 0;
  private minDelay: number; // Minimum delay between requests in ms
  private requestQueue: Array<() => void> = [];
  private processing: boolean = false;

  constructor(minDelay: number = 1000) { // Default 1 second between requests
    this.minDelay = minDelay;
  }

  /**
   * Wait for the rate limit delay
   */
  async wait(): Promise<void> {
    const now = Date.now();
    const timeSinceLastRequest = now - this.lastRequestTime;
    
    if (timeSinceLastRequest < this.minDelay) {
      const waitTime = this.minDelay - timeSinceLastRequest;
      await new Promise(resolve => setTimeout(resolve, waitTime));
    }
    
    this.lastRequestTime = Date.now();
  }

  /**
   * Execute a function with rate limiting
   */
  async execute<T>(fn: () => Promise<T>): Promise<T> {
    await this.wait();
    return await fn();
  }
}

