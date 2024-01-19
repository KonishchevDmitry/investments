// Copyright © 2018-21 Qtrac Ltd. All rights reserved.

use crate::plan::DiffKind;
use crate::util::{PathBufExt, Which};
use approx::abs_diff_eq;
use std::fmt;
use std::path::{Path, PathBuf};

static EXPECTED_PATH: &str = "$EXPECTED_PATH";
static ACTUAL_PATH: &str = "$ACTUAL_PATH";
static OUT_PATH: &str = "$OUT_PATH";

/// Represents a single test which can be generated or retested.
///
/// Create a new test using [`new()`](struct.Test.html#method.new).
/// Override its defaults using the builder methods, and finally call
/// [`build()`](struct.Test.html#method.build).
///
/// New `Test`s should be passed to the
/// [`Plan::push()`](struct.Plan.html#method.push) method.
///
/// See also the [builders example](index.html#builders-example).
#[derive(Clone, Debug)]
pub struct Test {
    pub(crate) name: String,
    pub(crate) exit_code: i32,
    pub(crate) wait: f64,
    pub(crate) app: PathBuf,
    pub(crate) app_args: Vec<String>,
    pub(crate) stdin: Vec<u8>,
    pub(crate) stdin_filename: Option<PathBuf>,
    pub(crate) stdout: Option<PathBuf>,
    pub(crate) diff: Option<DiffKind>,
    pub(crate) diff_args: Vec<String>,
}

impl Test {
    pub(crate) fn default() -> Test {
        Test {
            name: "".to_string(),
            exit_code: 0,
            wait: 0.0,
            app: PathBuf::new(),
            app_args: vec![],
            stdin: vec![],
            stdin_filename: None,
            stdout: None,
            diff: None,
            diff_args: vec![],
        }
    }

    pub(crate) fn with_paths(
        &self,
        expected_path: &Path,
        actual_path: &Path,
        which: &Which,
    ) -> Test {
        let out_path = if which == &Which::Expected {
            expected_path
        } else {
            actual_path
        };
        let mut app_args = Vec::with_capacity(self.app_args.len());
        if !self.app_args.is_empty() {
            let out = out_path.to_string_lossy();
            for arg in &self.app_args {
                app_args.push(arg.replace(OUT_PATH, &out));
            }
        }
        let mut diff_args = Vec::with_capacity(self.diff_args.len());
        if !self.diff_args.is_empty() {
            let exp_path = &expected_path.to_string_lossy();
            let act_path = &actual_path.to_string_lossy();
            for arg in &self.diff_args {
                diff_args.push(
                    arg.replace(EXPECTED_PATH, exp_path)
                        .replace(ACTUAL_PATH, act_path),
                );
            }
        }
        let stdout = self.maybe_stdout_with_path(out_path);
        Test {
            name: self.name.clone(),
            exit_code: self.exit_code,
            wait: 0.0,
            app: self.app.clone(),
            app_args,
            stdin: self.stdin.clone(),
            stdin_filename: self.stdin_filename.clone(),
            stdout,
            diff: self.diff.clone(),
            diff_args,
        }
    }

    pub(crate) fn output_filename(
        &self,
        out_path: &Path,
        which: &Which,
    ) -> PathBuf {
        let out = out_path.to_string_lossy().into_owned();
        for arg in &self.app_args {
            if arg.contains(&out) {
                return Path::new(arg).to_path_buf();
            } else if arg.contains(OUT_PATH) {
                return Path::new(&arg.replace(OUT_PATH, &out))
                    .to_path_buf();
            }
        }
        if let Some(path) = self.maybe_stdout_with_path(out_path) {
            return path;
        }
        for arg in &self.diff_args {
            if which == &Which::Expected && arg.contains(EXPECTED_PATH) {
                return Path::new(&arg.replace(EXPECTED_PATH, &out))
                    .to_path_buf();
            } else if which == &Which::Actual && arg.contains(ACTUAL_PATH)
            {
                return Path::new(&arg.replace(ACTUAL_PATH, &out))
                    .to_path_buf();
            }
        }
        PathBuf::new()
    }

    fn maybe_stdout_with_path(&self, out_path: &Path) -> Option<PathBuf> {
        match &self.stdout {
            Some(stdout) => {
                if !stdout.is_empty() {
                    if stdout.starts_with(OUT_PATH) {
                        return Some(stdout.replace(OUT_PATH, out_path));
                    } else {
                        return Some(out_path.join(stdout));
                    }
                }
            }
            None => return None,
        }
        None
    }

    pub(crate) fn is_ok(&self) -> bool {
        if self.app.is_empty() {
            return false;
        }
        if !self.app_args.is_empty() {
            return true;
        }
        match &self.stdout {
            Some(stdout) => !stdout.is_empty(),
            None => false,
        }
    }

    pub(crate) fn will_diff(&self) -> bool {
        self.diff != Some(DiffKind::No)
    }

    pub(crate) fn rt(&self, u: u32) -> String {
        let mut test = format!("[{}]\n", u);
        if !self.name.is_empty() {
            test.push_str(&format!("NAME: {}\n", &self.name));
        }
        if self.exit_code != 0 {
            test.push_str(&format!("EXITCODE: {}\n", self.exit_code));
        }
        if let Some(filename) = &self.stdin_filename {
            if !filename.is_empty() {
                test.push_str(&format!(
                    "STDIN: {}\n",
                    &filename.display()
                ));
            }
        }
        if let Some(stdout) = &self.stdout {
            if !stdout.is_empty() {
                let mut stdout = stdout.to_string_lossy().into_owned();
                if stdout.starts_with(OUT_PATH) {
                    // Allow for / or \
                    stdout = stdout[OUT_PATH.len() + 1..].to_string();
                }
                test.push_str(&format!("STDOUT: {}\n", &stdout));
            }
        }
        if !abs_diff_eq!(self.wait, 0.0) {
            test.push_str(&format!("WAIT: {:.3}\n", self.wait));
        }
        test.push_str(&format!("APP: {}\n", &self.app.display()));
        for arg in &self.app_args {
            test.push_str(&format!("     {}\n", &arg));
        }
        if let Some(diff) = &self.diff {
            let text = match diff {
                DiffKind::No => "no".to_string(),
                DiffKind::Binary => "rt-binary".to_string(),
                DiffKind::Image => "rt-image".to_string(),
                DiffKind::Json => "rt-json".to_string(),
                DiffKind::Text => "rt-text".to_string(),
                DiffKind::Custom(name) => format!("{}", name.display()),
            };
            test.push_str(&format!("DIFF: {}\n", text));
            for arg in &self.diff_args {
                test.push_str(&format!("      {}\n", &arg));
            }
        }
        test
    }
}

impl fmt::Display for Test {
    fn fmt(&self, out: &mut fmt::Formatter) -> fmt::Result {
        let mut app = self.app.to_string_lossy().into_owned();
        if app.contains(' ') {
            app.insert(0, '"');
            app.push('"');
        }
        let mut args = Vec::with_capacity(self.app_args.len());
        for arg in &self.app_args {
            args.push(if arg.contains(' ') {
                format!("\"{}\"", arg)
            } else {
                arg.to_string()
            });
        }
        let name = if self.name.is_empty() {
            "".to_string()
        } else {
            format!("{}: ", &self.name)
        };
        write!(
            out,
            "{}{} {} -> {}",
            name,
            app,
            args.join(" "),
            self.exit_code
        )
    }
}
