// Copyright © 2019-21 Qtrac Ltd. All rights reserved.

use super::Test;
use crate::plan::DiffKind;
use crate::util::maybe_s;
use std::{mem, path::Path};

impl Test {
    /// Returns a new `Test`.
    ///
    /// Creates a new `Test` with the application to test set to the given
    /// `app`.
    ///
    /// The given `app` should either be in the `PATH` or should include a
    /// path. For example:
    /// `Path::new(env!("CARGO_TARGET_DIR")).join("myapp.exe")`.
    ///
    /// The `Test`'s defaults are for an expected error code/error level of
    /// 0, no arguments for the application to test, no `stdin` to be fed
    /// into the application, no `stdout` to be captured from the
    /// application, and for expected and actual outputs to be compared
    /// using the [`DiffKind`](enum.DiffKind.html) that retest itself
    /// determines.
    ///
    /// All the defaults can be overridden using the `Test`'s
    /// methods.
    pub fn new<P: AsRef<Path>>(app: P) -> Test {
        let mut test = Test::default();
        test.app = app.as_ref().to_path_buf();
        test
    }

    /// Sets the command line arguments to be passed to the application to
    /// test.
    ///
    /// If one of these arguments is the filename of a file which you want
    /// to generate or retest, then its name _must_ be prefixed with
    /// `$OUT_PATH`, e.g., `.args(&["-o", "$OUT_PATH/123.png"])`. When
    /// generating, `$OUT_PATH` is replaced by the _expected_ path, and
    /// when retesting by the _actual_ path.
    ///
    /// Note that retest can compare _either_ a single output file _or_
    /// the application to test's `stdout` (captured to a file), but not
    /// both at the same time. If you need to test both, then create two
    /// separate tests. This is because each comparison may need to be
    /// done differently. For example the output file might be compared as
    /// an image and the `stdout` as text.
    ///
    /// See also [`stdout()`](struct.Test.html#method.stdout).
    pub fn args(&mut self, args: &[&str]) -> &mut Self {
        self.app_args = args.iter().map(|arg| arg.to_string()).collect();
        self
    }

    /// Sets the `Test`'s name.
    ///
    /// This is used when logging passed tests and in all cases when
    /// logging output is at the “debug” level.
    pub fn name(&mut self, name: &str) -> &mut Self {
        self.name = name.to_string();
        self
    }

    /// Sets the expected exit code/error level when the application to
    /// test is run.
    pub fn exit_code(&mut self, exit_code: i32) -> &mut Self {
        self.exit_code = exit_code;
        self
    }

    /// Sets how long (in seconds) to wait before running the application
    /// to test.
    ///
    /// The default is 0.0, i.e., don't wait.
    ///
    /// This option can be useful if a test sometimes “outruns” the
    /// operating system causing it to needlessly fail, and which can be
    /// cured by a small wait.
    pub fn wait(&mut self, wait: f64) -> &mut Self {
        self.wait = wait;
        self
    }

    /// Sets the raw bytes to be passed to the application to test's
    /// `stdin`.
    ///
    /// Note that if you use this method and subsequently use the
    /// [`Plan::rt()`](struct.Plan.html#method.rt)
    /// method, the returned [retest plan file
    /// (`.rt`)](http://www.qtrac.eu/retest.html#retestplanfile) format
    /// string will have a `STDIN` placeholder for each test this method is
    /// used on. For example:
    ///
    /// ```ignore
    /// STDIN: ///87 raw bytes///
    /// ```
    ///
    /// This is because the bytes are fed directly into the method rather
    /// than read from a file as happens when using a [retest plan file
    /// (`.rt`)](http://www.qtrac.eu/retest.html#retestplanfile).
    pub fn stdin_redirect(&mut self, stdin: &[u8]) -> &mut Self {
        self.stdin_filename = Some(
            Path::new(&format!(
                "///{} raw byte{}///",
                stdin.len(),
                maybe_s(stdin.len())
            ))
            .to_path_buf(),
        );
        self.stdin = stdin.to_vec();
        self
    }

    /// Sets the name of the file to capture the application to test's
    /// `stdout` to.
    ///
    /// This file will be written to the _expected_ path when generating,
    /// or to the _actual_ path when retesting.
    ///
    /// Note that retest can compare _either_ a single output file _or_
    /// the application to test's `stdout` (captured to a file), but not
    /// both at the same time. If you need to test both, then create two
    /// separate tests. This is because each comparison may need to be
    /// done differently. For example the output file might be compared as
    /// an image and the `stdout` as text.
    ///
    /// See also [`args()`](struct.Test.html#method.args).
    pub fn stdout<P: AsRef<Path>>(&mut self, stdout: P) -> &mut Self {
        self.stdout = Some(stdout.as_ref().to_path_buf());
        self
    }

    /// Sets how retest comparisons between expecteds and actuals is done.
    ///
    /// By default, the [`Plan::retest()`](struct.Plan.html#method.retest)
    /// method guesses what kind of comparison to do when
    /// comparing an expected output with an actual output during a retest
    /// run.
    ///
    /// For files with no suffix it assumes `DiffKind::Binary`, and for
    /// files with suffix `.jsn` or `.json` it assumes `DiffKind::Json`
    /// (and compares the JSON data, ignoring irrelevant whitespace). For
    /// common image formats it assumes `DiffKind::Image` and compares
    /// pixel for pixel; otherwise it assumes `DiffKind::Text` and compares
    /// text (but ignores line-endings).
    ///
    /// You can force retest to use the comparison mode of your choice for
    /// a particular test by passing a [`DiffKind`](enum.DiffKind.html) to
    /// this method.
    ///
    /// Or you can make [`Plan::retest()`](struct.Plan.html#method.retest)
    /// use an external tool to do the comparison by passing the tool's
    /// name (e.g., pass `DiffKind::custom("diff")` on Unix, to
    /// use the `diff` tool). The tool must return 0 if the compared files
    /// are (or are considered to be) the same, or non-zero otherwise.
    ///
    /// Or you can force retest _not_ to compare output (i.e., if you only
    /// want to check the exit status/error level) by passing
    /// `DiffKind::No`.
    pub fn diff(&mut self, diff: DiffKind) -> &mut Self {
        self.diff = Some(diff);
        self
    }

    /// Sets the command line arguments to pass to an external tool if one
    /// is specified using the
    /// [`diff()`](struct.Test.html#method.diff) method.
    ///
    /// Normally you will need at least two arguments giving the names of
    /// the two files to compare, e.g., [`"$EXPECTED_PATH/test05.dat"`,
    /// `"$ACTUAL_PATH/test05.dat"`]. At runtime these paths are replaced
    /// with `rt_expected` and `rt_actual` as appropriate, or to the
    /// path(s) set using
    /// [`Plan::expected_path()`](struct.Plan.html#method.expected_path)
    /// or
    /// [`Plan::actual_path()`](struct.Plan.html#method.actual_path).
    pub fn diff_args(&mut self, args: &[&str]) -> &mut Self {
        self.diff_args = args.iter().map(|arg| arg.to_string()).collect();
        self
    }

    /// Returns the built `Test` ready to be passed to
    /// [`Plan::push()`](struct.Plan.html#method.push).
    pub fn build(&mut self) -> Self {
        mem::replace(self, Test::default())
    }
}
