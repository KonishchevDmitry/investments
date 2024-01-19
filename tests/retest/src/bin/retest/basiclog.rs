// Copyright © 2018-21 Qtrac Ltd. All rights reserved.

use crate::retest::maybe_s;
use log::{Level, Metadata, Record};
use std::io::{self, Write};
use termcolor::{
    Color, ColorChoice, ColorSpec, StandardStream, WriteColor,
};

pub struct BasicLog {
    use_color: bool,
}

impl BasicLog {
    pub fn default(use_color: bool) -> BasicLog {
        BasicLog { use_color }
    }
}

impl log::Log for BasicLog {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Debug
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let msg = format!("{}", record.args());
            if !self.use_color
                || color_println(&msg, record.level()).is_err()
            {
                if !msg.starts_with('\t') {
                    println!("{}", msg);
                } else {
                    let parts =
                        msg.split('\t').skip(1).collect::<Vec<_>>();
                    let total: u32 = parts[0].parse().unwrap();
                    let errors: u32 = parts[3].parse().unwrap();
                    print!(
                        "of {} test{}, {} passed, {} failed",
                        total,
                        maybe_s(total),
                        parts[1],
                        parts[2]
                    );
                    if errors > 0 {
                        print!(", {} error{}", errors, maybe_s(errors));
                    }
                    println!(", in {}", parts[4]);
                }
            }
        }
    }

    fn flush(&self) {}
}

fn color_println(msg: &str, level: Level) -> io::Result<()> {
    let mut stdout = StandardStream::stdout(ColorChoice::Always);
    if msg.starts_with('\t') {
        let parts = msg.split('\t').skip(1).collect::<Vec<_>>();
        let total: u32 = parts[0].parse().unwrap();
        let fails: u32 = parts[2].parse().unwrap();
        let errors: u32 = parts[3].parse().unwrap();
        stdout.set_color(ColorSpec::new().set_fg(Some(Color::Blue)))?;
        write!(&mut stdout, "of {} test{}", total, maybe_s(total))?;
        stdout.set_color(ColorSpec::new().set_fg(Some(Color::Blue)))?;
        write!(&mut stdout, ", ")?;
        stdout.set_color(ColorSpec::new().set_fg(Some(Color::Green)))?;
        write!(&mut stdout, "{} passed", parts[1])?;
        stdout.set_color(ColorSpec::new().set_fg(Some(Color::Blue)))?;
        write!(&mut stdout, ", ")?;
        stdout.set_color(ColorSpec::new().set_fg(Some(
            if fails == 0 { Color::Green } else { Color::Red },
        )))?;
        write!(&mut stdout, "{} failed", parts[2])?;
        if errors > 0 {
            stdout
                .set_color(ColorSpec::new().set_fg(Some(Color::Blue)))?;
            write!(&mut stdout, ", ")?;
            stdout
                .set_color(ColorSpec::new().set_fg(Some(Color::Red)))?;
            write!(&mut stdout, "{} error{}", errors, maybe_s(errors))?;
        }
        stdout.set_color(ColorSpec::new().set_fg(Some(Color::Blue)))?;
        writeln!(&mut stdout, ", in {}", parts[4])?;
    } else {
        let color = match level {
            Level::Error => Color::Red,
            Level::Warn => Color::Blue,
            Level::Info => Color::Green,
            _ => Color::Magenta,
        };
        stdout.set_color(ColorSpec::new().set_fg(Some(color)))?;
        writeln!(&mut stdout, "{}", msg)?;
    }
    stdout.reset()?;
    Ok(())
}
