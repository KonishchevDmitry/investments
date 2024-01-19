// Copyright © 2018-21 Qtrac Ltd. All rights reserved.

use crate::retest::{xerror, XResult};
use chrono::prelude::*;
use lazy_static::lazy_static;
use log::LevelFilter;
use regex::Regex;
use std::collections::BTreeSet;
use std::env;
use std::path::Path;

static RT_NAME: &str = "rt.rt";
static RT_NAME_WIN: &str = "rt-win.rt";
static RT_NAME_UNIX: &str = "rt-unix.rt";

#[derive(Debug)]
pub struct Config {
    pub filename: String,
    pub level: LevelFilter,
    pub cpus: usize,            // 0 implies all available
    pub numbers: BTreeSet<u32>, // empty implies all
    pub generate: bool,         // false -> run tests; true -> generate
    pub use_color: bool,
}

impl Config {
    fn default() -> Config {
        let filename = if cfg!(windows) && Path::new(RT_NAME_WIN).exists()
        {
            RT_NAME_WIN
        } else if cfg!(unix) && Path::new(RT_NAME_UNIX).exists() {
            RT_NAME_UNIX
        } else {
            RT_NAME
        };
        Config {
            filename: if Path::new(filename).exists() {
                filename.to_string()
            } else {
                "".to_string()
            },
            level: LevelFilter::Warn,
            cpus: 0,
            numbers: BTreeSet::new(),
            generate: false,
            use_color: true,
        }
    }
}

pub fn new() -> XResult<Option<Config>> {
    lazy_static! {
        static ref NUMBERS_RX: Regex =
            Regex::new(r"^(:?\d+|\d+-\d+)(:?,(:?\d+|\d+-\d+))*$")
                .unwrap();
    }
    let mut config = Config::default();
    for arg in env::args().skip(1) {
        if ["/?", "h", "-h", "--help", "help"].contains(&arg.as_str()) {
            show_help();
            return Ok(None);
        }
        if ["doc", "-m", "--manual", "manual"].contains(&arg.as_str()) {
            open::that("http://www.qtrac.eu/retest.html")?;
            return Ok(None);
        }
        if ["-V", "--version", "version"].contains(&arg.as_str()) {
            println!("{}", env!("CARGO_PKG_VERSION"));
            return Ok(None);
        }
        if ["v", "-v", "--verbose", "verbose"].contains(&arg.as_str()) {
            config.level = match config.level {
                LevelFilter::Off => LevelFilter::Error,
                LevelFilter::Error => LevelFilter::Warn,
                LevelFilter::Warn => LevelFilter::Info,
                _ => LevelFilter::Debug,
            };
        } else if ["q", "-q", "--quiet", "quiet"].contains(&arg.as_str())
        {
            config.level = LevelFilter::Off;
        } else if ["nocolor", "--nocolor", "mono", "--mono"]
            .contains(&arg.as_str())
        {
            config.use_color = false;
        } else if let Some(cpus) = arg.strip_prefix("cpus=") {
            config.cpus = cpus.parse()?;
        } else if NUMBERS_RX.is_match(&arg) {
            parse_numbers(&arg, &mut config.numbers)?;
        } else if ["g", "-g", "--generate", "gen", "generate"]
            .contains(&arg.as_str())
        {
            config.generate = true;
        } else if arg.ends_with(".rt") {
            if !Path::new(&arg).exists() {
                return xerror(format!(
                    "can't find retest plan file: {}",
                    arg
                ));
            }
            config.filename = arg.to_string();
        } else {
            return xerror(format!("invalid argument: {}", arg));
        }
    }
    Ok(Some(config))
}

#[allow(clippy::branches_sharing_code)]
fn parse_numbers(s: &str, numbers: &mut BTreeSet<u32>) -> XResult<()> {
    for span in s.split(',') {
        let ranges: Vec<_> = span.splitn(2, '-').collect();
        if ranges.len() == 1 {
            let number: u32 = ranges[0].parse()?;
            if number < 1 {
                return xerror("each test number must be 1 or more");
            }
            numbers.insert(number);
        } else {
            let start: u32 = ranges[0].parse()?;
            if start < 1 {
                return xerror("each test number must be 1 or more");
            }
            let end: u32 = ranges[1].parse()?;
            if end < start {
                return xerror("test ranges must have the form low-high");
            }
            for number in start..=end {
                numbers.insert(number);
            }
        }
    }
    Ok(())
}

fn show_help() {
    let now = Local::now();
    let year = if now.year() == 2019 {
        "2019".to_string()
    } else {
        format!("2019-{}", now.year() - 2000)
    };
    #[cfg(target_family = "windows")]
    let rtp = RT_NAME_WIN;
    #[cfg(target_family = "unix")]
    let rtp = RT_NAME_UNIX;
    println!(
        r#"retest v{version} © {year} Qtrac Ltd. All Rights Reserved.
              http://www.qtrac.eu/retest.html

usage: retest [verbose] [cpus=n] [nocolor] [tests] [{rt}]
              run all (or specified numbered) tests and
              save their outputs in the actuals folder and
              diff their outputs with the expecteds

usage: retest [verbose] [cpus=n] [nocolor] [tests] generate [{rt}]
              run all (or specified numbered) tests and
              save their outputs in the expecteds folder
              (g -g --generate gen generate)

usage: retest doc
              show the manual in your web browser and quit
              (doc -m --manual manual)

usage: retest help
              show this help text and quit (help h -h --help /?)

usage: retest version
              show retest's version and quit
              (version -V --version)

verbose: default is: show summary, errors, failures;
         use one verbose to show each test; use two for more;
         use quiet to only show errors or failures
         (v -v --verbose verbose q -q --quiet quiet)
cpus:    if specified uses at most this number of cpus;
         default is to use all available
nocolor: if specified output is monochrome (useful for redirecting)
         default is to use colors (nocolor --nocolor mono --mono)
tests:   numbers of specific tests to run or generate,
         e.g., 1,3,5,8-21 36-39 52 61-65
{rt}:   the retest plan file to use; defaults to
         {rtp} if it exists, otherwise to rt.rt

The command line arguments may be given in any order.

License: GNU General Public License Version 3.
"#,
        version = env!("CARGO_PKG_VERSION"),
        year = year,
        rt = RT_NAME,
        rtp = rtp
    );
}
