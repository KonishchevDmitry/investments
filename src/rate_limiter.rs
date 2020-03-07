use std::num::NonZeroU32;
use std::time::Duration;

use governor::Quota;
use governor::clock::{Clock, DefaultClock};
use governor::state::{RateLimiter as Limiter, NotKeyed, InMemoryState};
use log::debug;

pub struct RateLimiter {
    clock: DefaultClock,
    limiters: Vec<Limiter<NotKeyed, InMemoryState, DefaultClock>>,
}

impl RateLimiter {
    pub fn new() -> RateLimiter {
        RateLimiter {
            clock: DefaultClock::default(),
            limiters: Vec::new(),
        }
    }

    pub fn with_limit(mut self, max_burst: u32, duration: Duration) -> RateLimiter {
        let quota = Quota::with_period(duration / max_burst).unwrap()
            .allow_burst(NonZeroU32::new(max_burst).unwrap());
        self.limiters.push(Limiter::direct_with_clock(quota, &self.clock));
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
                std::thread::sleep(until.wait_time_from(self.clock.now()));
            }
        }
    }
}