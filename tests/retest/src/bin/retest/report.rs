// Copyright © 2018-21 Qtrac Ltd. All rights reserved.

use crate::cli::Config;
use crate::retest::{maybe_s, Counts};
use crate::util::human_time;
use log::{info, warn};
use std::time::Duration;

pub fn pre(config: &Config) {
    warn!("read retest plan from \"{}\"", config.filename);
    if config.generate {
        if !config.numbers.is_empty() {
            info!(
                "generating the specified test expected{}",
                maybe_s(config.numbers.len())
            );
        } else {
            info!("generating all the test expecteds");
        }
    } else if !config.numbers.is_empty() {
        info!(
            "running the specified test{} and comparing expecteds \
             with actuals...",
            maybe_s(config.numbers.len())
        );
    } else {
        info!("running tests and comparing expecteds with actuals...");
    }
}

pub fn post_generate(
    fewer: bool,
    count: u32,
    actions: usize,
    duration: Duration,
) {
    let message = if fewer {
        format!(
            "generated {} of {} expected{}",
            count,
            actions,
            maybe_s(count)
        )
    } else {
        format!("generated all {} expected{}", count, maybe_s(count))
    };
    warn!("{} in {}", message, human_time(duration));
}

pub fn post_retest(counts: &Counts, duration: Duration) {
    warn!(
        "\t{}\t{}\t{}\t{}\t{}",
        counts.total,
        counts.passed,
        counts.failed,
        counts.errors,
        human_time(duration)
    );
}
