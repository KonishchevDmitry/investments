// Copyright © 2018-21 Qtrac Ltd. All rights reserved.

use crate::plan::counts::Counts;
use crate::plan::{diff, Plan, Test};
use crate::util::{PathBufExt, Which};
use crate::xerr;
use crate::xerror::{xerror, XError, XResult};
use approx::abs_diff_eq;
use log::{debug, error, info};
use rayon::prelude::*;
use std::{
    borrow::Cow,
    env,
    fs::{self, File},
    io::Write,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::Duration,
};

#[derive(Debug)]
enum Outcome {
    Passed,
    Failed,
    Error,
    Generated(u32),
}

impl Plan {
    /// Returns a [`Counts`](struct.Counts.html) holding the number of
    /// tests, passes, fails, and errors.
    ///
    /// If the `RETEST_GENERATE` environment variable is present _and_ set
    /// to `1`, this method calls
    /// [`generate()`](struct.Plan.html#method.generate); otherwise it
    /// calls [`retest()`](struct.Plan.html#method.retest).
    pub fn apply(&self) -> XResult<Counts> {
        let gen = match env::var("RETEST_GENERATE") {
            Ok(value) => value == "1",
            Err(_) => false,
        };
        if gen {
            self.generate()
        } else {
            self.retest()
        }
    }

    /// Returns a [`Counts`](struct.Counts.html) holding the number of
    /// tests, passes, fails, and errors.
    ///
    /// Runs all the tests in the `Plan`. For each test, providing the exit
    /// code/error level is what's expected, any test output is saved to
    /// the _actual_ path and then the original output that was generated
    /// previously in the _expected_ path is compared with the newly saved
    /// output.
    ///
    /// During retesting, logging output is produced (or not) depending on
    /// the logger you are using and the logging level you have set.
    ///
    /// All the fields in the returned [`Counts`](struct.Counts.html) are
    /// used (although some could be 0).
    pub fn retest(&self) -> XResult<Counts> {
        maybe_create_paths(&self.expected_path, &self.actual_path)?;
        let mut counts = Counts::default();
        for reply in self
            .tests
            .par_iter()
            .map(|(u, test)| self.retest_one(*u, test))
            .collect::<Vec<_>>()
        {
            match reply {
                Ok(reply) => match reply {
                    Outcome::Passed => counts.passed += 1,
                    Outcome::Failed => counts.failed += 1,
                    Outcome::Error => counts.errors += 1,
                    Outcome::Generated(u) => xerr!(
                        "[{: >5}] internal error: generated instead \
                         of tested",
                        u
                    ),
                },
                Err(err) => {
                    counts.errors += 1;
                    error!("ERROR: {}", err);
                }
            };
            counts.total += 1;
        }
        Ok(counts)
    }

    fn retest_one(&self, u: u32, test: &Test) -> XResult<Outcome> {
        let wait = test.wait;
        let expected_filename =
            &test.output_filename(&self.expected_path, &Which::Expected);
        let actual_filename =
            &test.output_filename(&self.actual_path, &Which::Actual);
        let test = test.with_paths(
            &self.expected_path,
            &self.actual_path,
            &Which::Actual,
        );
        debug!("[{: >5}] info:  {}", u, test);
        if !actual_filename.is_empty() {
            // Delete if we can but don't worry
            if fs::remove_file(&actual_filename).is_err() {
                debug!(
                    "[{: >5}] info:  failed to delete \"{}\"",
                    u,
                    &actual_filename.display()
                );
            }
        }
        if !abs_diff_eq!(wait, 0.0) {
            if wait > 0.2 {
                debug!("[{: >5}] waiting {:.3}s before running", u, wait);
            }
            thread::sleep(Duration::from_millis(
                (wait.abs() * 1000.0) as u64,
            ));
        }
        let exit_code = run_test(&test)?;
        if exit_code != test.exit_code {
            error!(
                "[{: >5}] FAIL:  expected exit code {} != {}",
                u, test.exit_code, exit_code
            );
            return Ok(Outcome::Failed);
        }
        self.diff(u, &test, &expected_filename, &actual_filename)
    }

    fn diff<P: AsRef<Path>>(
        &self,
        u: u32,
        test: &Test,
        expected_filename: P,
        actual_filename: P,
    ) -> XResult<Outcome> {
        let expected_filename = expected_filename.as_ref();
        let actual_filename = actual_filename.as_ref();
        match diff::is_same(test, expected_filename, actual_filename) {
            Ok(true) => {
                let name = if test.name.is_empty() {
                    "".to_string()
                } else {
                    format!(":  {}", test.name)
                };
                info!("[{: >5}] Pass{}", u, name);
                Ok(Outcome::Passed)
            }
            Ok(false) => {
                error!(
                    "[{: >5}] FAIL:  \"{}\" != \"{}\"",
                    u,
                    &expected_filename.display(),
                    &actual_filename.display()
                );
                Ok(Outcome::Failed)
            }
            Err(err) => {
                error!("[{: >5}] ERROR: {}", u, err);
                Ok(Outcome::Error)
            }
        }
    }

    /// Returns a [`Counts`](struct.Counts.html) holding the number of
    /// tests and fails.
    ///
    /// Runs all the tests in the `Plan`. For each test, any test output is
    /// saved to the _expected_ path.
    ///
    /// During generating, logging output is produced (or not) depending on
    /// the logger you are using and the logging level you have set.
    ///
    /// Only the `total` and `failed` fields in the returned
    /// [`Counts`](struct.Counts.html) are used.
    pub fn generate(&self) -> XResult<Counts> {
        maybe_create_paths(&self.expected_path, &self.actual_path)?;
        let mut counts = Counts::default();
        for reply in self
            .tests
            .par_iter()
            .map(|(u, test)| self.generate_one(*u, test))
            .collect::<Vec<_>>()
        {
            match reply {
                Ok(reply) => match reply {
                    Outcome::Generated(_) => counts.total += 1,
                    Outcome::Failed => counts.failed += 1,
                    _ => xerr!(
                        "internal error: generated unexpected outcome"
                    ),
                },
                Err(err) => xerr!("ERROR: failed to generate: {}", err),
            }
        }
        Ok(counts)
    }

    fn generate_one(&self, u: u32, test: &Test) -> XResult<Outcome> {
        let test = test.with_paths(
            &self.expected_path,
            &self.actual_path,
            &Which::Expected,
        );
        debug!("[{: >5}] info:  {}", u, test);
        let expected_filename =
            &test.output_filename(&self.expected_path, &Which::Expected);
        if !expected_filename.is_empty()
            && fs::remove_file(&expected_filename).is_err()
        {
            debug!(
                "[{: >5}] info: failed to delete \"{}\"",
                u,
                &expected_filename.display()
            );
        }
        let exit_code = run_test(&test)?;
        if exit_code != test.exit_code {
            error!(
                "[{: >5}] FAIL:  expected exit code {} != {}",
                u, test.exit_code, exit_code
            );
            return Ok(Outcome::Failed);
        }
        if test.will_diff() {
            info!(
                "[{: >5}] Generated: \"{}\"",
                u,
                &test
                    .output_filename(&self.expected_path, &Which::Expected)
                    .display()
            );
        } else {
            info!("[{: >5}] Will check exit code: {}", u, test.exit_code);
        }
        Ok(Outcome::Generated(u))
    }
}

fn maybe_create_paths(
    expected_path: &Path,
    actual_path: &Path,
) -> XResult<()> {
    if !expected_path.exists() {
        fs::create_dir_all(&expected_path)?;
        info!("created \"{}\"", expected_path.display());
    }
    if !actual_path.exists() {
        fs::create_dir_all(&actual_path)?;
        info!("created \"{}\"", actual_path.display());
    }
    Ok(())
}

fn run_test(test: &Test) -> XResult<i32> {
    let mut command = Command::new(&test.app);
    command.args(&test.app_args);
    command.stderr(Stdio::null());
    if !test.stdin.is_empty() {
        command.stdin(Stdio::piped());
    }
    let capture = match &test.stdout {
        Some(path) => !path.is_empty(),
        None => false,
    };
    command.stdout(if capture { Stdio::piped() } else { Stdio::null() });
    let mut child = command.spawn().or_else(|err| {
        xerr!("Failed to run \"{}\": {}", test.app.display(), err)
    })?;
    if !test.stdin.is_empty() {
        let stdin = child.stdin.as_mut().ok_or_else(|| {
            XError::new(format!(
                "Failed to read from \"{}\"",
                test.app.display()
            ))
        })?;
        let stdin_filename = match &test.stdin_filename {
            Some(path) => path.to_string_lossy(),
            None => Cow::from(""),
        };
        stdin.write_all(&test.stdin).or_else(|err| {
            xerr!("Failed to write to \"{}\": {}", stdin_filename, err)
        })?;
    }
    let output = child.wait_with_output().or_else(|err| {
        xerr!("Failed to run \"{}\": {}", test.app.display(), err)
    })?;
    if let Some(stdout) = &test.stdout {
        let mut file = File::create(&stdout)?;
        file.write_all(&output.stdout)?;
    }
    match output.status.code() {
        Some(code) => Ok(code),
        None => Ok(-999), // Unix process terminated by signal
    }
}
