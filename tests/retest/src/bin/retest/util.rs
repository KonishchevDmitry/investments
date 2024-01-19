// Copyright © 2018-21 Qtrac Ltd. All rights reserved.

use std::time::Duration;

pub fn human_time(duration: Duration) -> String {
    const HOUR: u64 = 60 * 60;
    let mut hours = 0;
    let mut minutes = 0;
    let mut secs = duration.as_secs();
    let ms = duration.subsec_millis();
    if secs > HOUR {
        hours = secs / HOUR;
        secs %= HOUR;
    }
    if secs > 60 {
        minutes = secs / 60;
        secs %= 60;
    }
    let mut result = String::new();
    if hours > 0 {
        result.push_str(&format!("{}h", hours));
    }
    if minutes > 0 {
        result.push_str(&format!("{}m", minutes));
    }
    result.push_str(&format!("{}", secs));
    if hours == 0 && minutes == 0 {
        result.push_str(&format!(".{:03}", ms));
    }
    result.push('s');
    result
}
