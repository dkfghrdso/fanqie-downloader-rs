use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

pub struct TokenBucket {
    rate: f64,
    capacity: f64,
    tokens: Arc<Mutex<f64>>,
    last_update: Arc<Mutex<Instant>>,
}

impl TokenBucket {
    pub fn new(rate: f64, capacity: f64) -> Self {
        Self {
            rate,
            capacity,
            tokens: Arc::new(Mutex::new(capacity)),
            last_update: Arc::new(Mutex::new(Instant::now())),
        }
    }

    pub async fn acquire(&self) {
        loop {
            {
                let mut tokens = self.tokens.lock().await;
                let mut last_update = self.last_update.lock().await;

                let now = Instant::now();
                let elapsed = now.duration_since(*last_update).as_secs_f64();
                
                *tokens = (*tokens + elapsed * self.rate).min(self.capacity);
                *last_update = now;

                if *tokens >= 1.0 {
                    *tokens -= 1.0;
                    return;
                }

                let wait_time = (1.0 - *tokens) / self.rate;
                drop(tokens);
                drop(last_update);

                tokio::time::sleep(Duration::from_secs_f64(wait_time)).await;
            }
        }
    }

    pub async fn try_acquire(&self) -> bool {
        let mut tokens = self.tokens.lock().await;
        let mut last_update = self.last_update.lock().await;

        let now = Instant::now();
        let elapsed = now.duration_since(*last_update).as_secs_f64();
        
        *tokens = (*tokens + elapsed * self.rate).min(self.capacity);
        *last_update = now;

        if *tokens >= 1.0 {
            *tokens -= 1.0;
            true
        } else {
            false
        }
    }

    pub fn get_rate(&self) -> f64 {
        self.rate
    }

    pub fn get_capacity(&self) -> f64 {
        self.capacity
    }
}
