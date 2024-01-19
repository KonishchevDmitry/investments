// Copyright © 2018-21 Qtrac Ltd. All rights reserved.

extern crate retest;

mod basiclog;
mod cli;
mod report;
mod util;

use basiclog::BasicLog;
// Retest library users would normally just do something like this:
//  use retest::{Counts, DiffKind, Plan, Test, XResult};
use crate::retest::{xerr, xerror, Plan, XResult};
use log::error;
use rayon::ThreadPoolBuilder;
use std::time::Instant;
use termcolor::{ColorChoice, StandardStream, WriteColor};

fn main() {
    let code = match run() {
        Err(err) => {
            error!("{}", err);
            2
        }
        Ok(mut n) => {
            if n >= 10 {
                let mut stdout =
                    StandardStream::stdout(ColorChoice::Always);
                let _ = stdout.reset(); // Nothing we can do if it fails
                n -= 10;
            }
            n
        }
    };
    ::std::process::exit(code);
}

fn run() -> XResult<i32> {
    let config = cli::new()?;
    if let Some(config) = config {
        log::set_boxed_logger(Box::new(BasicLog::default(
            config.use_color,
        )))
        .map(|()| log::set_max_level(config.level))?;
        if config.filename.is_empty() {
            #[cfg(target_family = "windows")]
            xerr!("cannot find \"rt-win.rt\" or \"rt.rt\"");
            #[cfg(target_family = "unix")]
            xerr!("cannot find \"rt-unix.rt\" or \"rt.rt\"");
        }
        let use_color = config.use_color;
        let _ = ctrlc::set_handler(move || {
            if use_color {
                let mut stdout =
                    StandardStream::stdout(ColorChoice::Always);
                let _ = stdout.reset(); // Nothing we can do if it fails
            }
            println!("terminated by user");
            ::std::process::exit(3);
        }); // Nothing we can do if it fails
        let mut code = match run_plan(&config) {
            Err(err) => {
                error!("{}", err);
                2
            }
            Ok(false) => 1, // failed
            Ok(true) => 0,  // all generated/passed
        };
        if config.use_color {
            code += 10;
        }
        Ok(code)
    } else {
        Ok(2)
    }
}

fn run_plan(config: &cli::Config) -> XResult<bool> {
    let plan =
        Plan::new_from_rt_filtered(&config.filename, &config.numbers)?;
    report::pre(config);
    if config.cpus > 0 {
        ThreadPoolBuilder::new()
            .num_threads(config.cpus)
            .build_global()?;
    }
    if config.generate {
        generate(&plan)
    } else {
        retest_(&plan)
    }
}

fn generate(plan: &Plan) -> XResult<bool> {
    let now = Instant::now();
    let counts = plan.generate()?;
    let duration = now.elapsed();
    let fewer = (counts.total as usize) < plan.len();
    report::post_generate(fewer, counts.total, plan.len(), duration);
    Ok(counts.failed == 0)
}

fn retest_(plan: &Plan) -> XResult<bool> {
    let now = Instant::now();
    let counts = plan.retest()?;
    let duration = now.elapsed();
    report::post_retest(&counts, duration);
    Ok(counts.failed == 0 && counts.errors == 0)
}
