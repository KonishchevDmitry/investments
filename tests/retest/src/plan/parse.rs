// Copyright © 2018-21 Qtrac Ltd. All rights reserved.

use crate::plan::{diff_kind_for, Plan, Test};
use crate::util::PathExt;
use crate::xerr;
use crate::xerror::{xerror, XResult};
use fnv::FnvHashMap;
use std::cmp;
use std::collections::BTreeSet;
use std::fs;
use std::io::prelude::*;
use std::io::BufReader;
use std::path::{Path, PathBuf};

struct Pos<'a> {
    pub filename: &'a Path,
    pub lino: usize,
}

type EnvMap = FnvHashMap<String, String>;

impl Plan {
    /// Returns a `Plan` on which you can call
    /// [`generate()`](struct.Plan.html#method.generate),
    /// [`retest()`](struct.Plan.html#method.retest), or
    /// [`apply()`](struct.Plan.html#method.apply).
    ///
    /// Reads the test plan and all the tests from the file with the given
    /// filename which must be in [retest plan file
    /// (`.rt`)](http://www.qtrac.eu/retest.html#retestplanfile) format,
    /// and returns the corresponding `Plan`.
    ///
    /// Note that plans can also be created programatically using
    /// [`new()`](struct.Plan.html#method.new) and the `Plan` builder
    /// methods.
    pub fn new_from_rt<P: AsRef<Path>>(filename: P) -> XResult<Plan> {
        let numbers: BTreeSet<u32> = BTreeSet::new(); // empty implies all
        Plan::new_from_rt_filtered(&filename, &numbers)
    }

    /// Returns a `Plan` on which you can call
    /// [`generate()`](struct.Plan.html#method.generate),
    /// [`retest()`](struct.Plan.html#method.retest), or
    /// [`apply()`](struct.Plan.html#method.apply).
    ///
    /// Reads the test plan from the file with the given filename which
    /// must be in [retest plan file
    /// (`.rt`)](http://www.qtrac.eu/retest.html#retestplanfile) format,
    /// and returns the corresponding `Plan`. Only those tests whose
    /// numbers appear in the given numbers set are included in the `Plan`.
    ///
    /// Note that plans can also be created programatically using
    /// [`new()`](struct.Plan.html#method.new) and the `Plan` builder
    /// methods.
    pub fn new_from_rt_filtered<P: AsRef<Path>>(
        filename: P,
        numbers: &BTreeSet<u32>,
    ) -> XResult<Plan> {
        let mut plan = Plan::new();
        plan.filename = Some(filename.as_ref().to_path_buf());
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let home = home.to_string_lossy();
        let mut env_app = String::new();
        let mut env_app_args: Vec<String> = vec![];
        let mut env_diff = String::new();
        let mut env_diff_args: Vec<String> = vec![];
        let mut env_user = EnvMap::default();
        let mut max_u = 0;
        let mut u = 0; // 0 implies env since tests must be >= 1
        let mut args_for_app = true;
        let mut test = Test::default();
        let mut pos = Pos { filename: filename.as_ref(), lino: 0 };
        let file = filename.as_ref().open()?;
        for (lino, line) in BufReader::new(file).lines().enumerate() {
            pos.lino = lino + 1;
            let line = line?;
            let indented = line.starts_with(|c| c == ' ' || c == '\t');
            let line = line.trim().replace("$HOME", &home);
            if line.is_empty() || line.starts_with('#') {
                continue; // skip blank lines and comments
            }
            if line.starts_with('[') && line.ends_with(']') {
                u = parse_section_head(
                    &pos,
                    &line,
                    u,
                    &mut test,
                    &mut plan,
                    &env_app_args,
                    &env_diff_args,
                    numbers,
                )?;
                max_u = cmp::max(max_u, u);
                continue;
            }
            if indented {
                parse_indented_arg(
                    u,
                    args_for_app,
                    &mut env_app_args,
                    &mut env_diff_args,
                    &mut test,
                    &line,
                    &env_user,
                );
                continue;
            }
            let (key, value) = parse_key_value(&pos, &line)?;
            if u == 0 {
                parse_env_entry(
                    &pos,
                    &key,
                    value,
                    &mut args_for_app,
                    &mut env_app,
                    &mut env_diff,
                    &mut plan,
                    &mut env_user,
                )?;
            } else {
                parse_test_entry(
                    &pos,
                    &key,
                    value,
                    &mut args_for_app,
                    &env_app,
                    &env_diff,
                    &mut test,
                    &env_user,
                )?;
            }
        }
        if u > 0
            && test.is_ok()
            && (numbers.is_empty() || numbers.contains(&u))
        {
            plan.tests.insert(u, test);
        }
        // Just in case someone programmatically adds Tests to a .rt
        // file's tests
        plan.u = max_u;
        Ok(plan)
    }
}

#[allow(clippy::too_many_arguments)]
fn parse_section_head(
    pos: &Pos,
    line: &str,
    u: u32,
    test: &mut Test,
    plan: &mut Plan,
    env_app_args: &[String],
    env_diff_args: &[String],
    numbers: &BTreeSet<u32>,
) -> XResult<u32> {
    let mut new_u = u;
    let head = line.trim_matches(|c| c == '[' || c == ']').to_uppercase();
    if head != "ENV" {
        if u > 0 && test.is_ok() {
            if numbers.is_empty() || numbers.contains(&u) {
                plan.tests.insert(u, test.clone());
            }
            *test = Test::default();
        }
        new_u = head.parse()?;
        if plan.tests.contains_key(&new_u) {
            xerr!(
                "\"{}\" #{}: duplicate test number {}",
                pos.filename.display(),
                pos.lino,
                new_u
            );
        }
        for arg in env_app_args {
            test.app_args.push(arg.to_string());
        }
        for arg in env_diff_args {
            test.diff_args.push(arg.to_string());
        }
    }
    Ok(new_u)
}

fn parse_indented_arg(
    u: u32,
    args_for_app: bool,
    env_app_args: &mut Vec<String>,
    env_diff_args: &mut Vec<String>,
    test: &mut Test,
    line: &str,
    env_user: &EnvMap,
) {
    let line = expand_env(line, env_user);
    if args_for_app {
        if u == 0 {
            env_app_args.push(line);
        } else {
            test.app_args.push(line);
        }
    } else if u == 0 {
        env_diff_args.push(line);
    } else {
        test.diff_args.push(line);
    }
}

fn parse_key_value(pos: &Pos, line: &str) -> XResult<(String, String)> {
    let parts: Vec<_> =
        line.splitn(2, |c| c == ':' || c == '=').collect();
    if parts.len() != 2 {
        xerr!(
            "\"{}\" #{}: invalid entry '{}'",
            pos.filename.display(),
            pos.lino,
            line
        );
    }
    let key = parts[0].trim().to_uppercase();
    let value = parts[1].trim().to_string();
    Ok((key, value))
}

#[allow(clippy::too_many_arguments)]
fn parse_env_entry(
    pos: &Pos,
    key: &str,
    value: String,
    args_for_app: &mut bool,
    env_app: &mut String,
    env_diff: &mut String,
    plan: &mut Plan,
    env_user: &mut EnvMap,
) -> XResult<()> {
    match key {
        "APP" => {
            *args_for_app = true;
            *env_app = if cfg!(windows) {
                value
            } else {
                // Need path on Unix
                match value.find('/') {
                    Some(_) => value,
                    None => format!("./{}", value),
                }
            };
        }
        "EXPECTED_PATH" => {
            plan.expected_path = Path::new(&value).to_path_buf()
        }
        "ACTUAL_PATH" => {
            plan.actual_path = Path::new(&value).to_path_buf()
        }
        "DIFF" => {
            *args_for_app = false;
            *env_diff = value;
        }
        "SET" => {
            let (mut key, value) =
                parse_key_value(pos, value.trim_start())?;
            key.insert(0, '$');
            env_user.insert(key, value);
        }
        _ => xerr!(
            "\"{}\" #{}: unrecognized env key '{}'",
            pos.filename.display(),
            pos.lino,
            key
        ),
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn parse_test_entry(
    pos: &Pos,
    key: &str,
    value: String,
    args_for_app: &mut bool,
    env_app: &str,
    env_diff: &str,
    test: &mut Test,
    env_user: &EnvMap,
) -> XResult<()> {
    let value = expand_env(&value, env_user);
    match key {
        "NAME" => test.name = value,
        "EXIT_CODE" | "EXITCODE" => test.exit_code = value.parse()?,
        "STDIN" => {
            test.stdin_filename = Some(Path::new(&value).to_path_buf());
            test.stdin = fs::read(&value).or_else(|err| {
                xerr!("Failed to read STDIN entry's data: {}", err)
            })?;
        }
        "STDOUT" => test.stdout = Some(Path::new(&value).to_path_buf()),
        "WAIT" => test.wait = value.parse()?,
        "APP" => {
            *args_for_app = true;
            test.app =
                Path::new(&value.replace("$APP", env_app)).to_path_buf();
        }
        "DIFF" => {
            *args_for_app = false;
            test.diff = diff_kind_for(&value.replace("$DIFF", env_diff));
        }
        _ => xerr!(
            "\"{}\" #{}: unrecognized test key '{}'",
            pos.filename.display(),
            pos.lino,
            key
        ),
    }
    Ok(())
}

fn expand_env(value: &str, env_user: &EnvMap) -> String {
    let mut pairs = env_user.iter().collect::<Vec<_>>();
    pairs.sort_unstable_by(|p, q| q.0.len().cmp(&p.0.len())); // By key len
    let mut expanded = value.to_string();
    for (env_key, env_value) in pairs {
        expanded = expanded.replace(env_key, env_value);
    }
    expanded
}
