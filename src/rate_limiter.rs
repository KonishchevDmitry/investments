use std::num::NonZeroU32;
use std::time::Duration;

use governor::{Quota, DirectRateLimiter};
use governor::clock::Clock;
use log::debug;

pub struct RateLimiter {
    limiters: Vec<DirectRateLimiter>,
}

impl RateLimiter {
    pub fn new() -> RateLimiter {
        RateLimiter {
            limiters: Vec::new(),
        }
    }

    pub fn with_limit(mut self, max_burst: u32, duration: Duration) -> RateLimiter {
        let max_burst = NonZeroU32::new(max_burst).unwrap();
        let quota = Quota::new(max_burst, duration).unwrap();
        self.limiters.push(DirectRateLimiter::new(quota));
        self
    }

    // Please notice: naive implementation.
    // We iterate over limiters which makes us drift to the future which reduces accuracy. To make
    // this impact less noticeable limiters should be added in order of decreasing duration.
    pub fn wait(&self, name: &str) {
        let mut limited = false;

        for limiter in &self.limiters {
            while let Err(until) = limiter.check() {
                if !limited {
                    debug!("Rate limiting {}...", name);
                    limited = true;
                }
                std::thread::sleep(until.wait_time_from(limiter.get_clock().now()));
            }
        }
    }
}