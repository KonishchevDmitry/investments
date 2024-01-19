// Copyright © 2019-21 Qtrac Ltd. All rights reserved.

use super::{Plan, Test, RT_ACTUAL_PATH, RT_EXPECTED_PATH};
use std::collections::BTreeMap;
use std::{mem, path::Path};

impl Plan {
    /// Returns a new empty `Plan`.
    ///
    /// Creates a new `Plan` with a default expected path of `rt_expected`
    /// and a default actual path of `rt_actual` (i.e., both are relative
    /// to the current folder), and that contains no tests. When each
    /// [`Test`](struct.Test.html) is run its `app` is assumed to be
    /// in the `PATH` or to include a path. For example, you could do
    /// something like this:
    /// ```rust,text,ignore
    /// let app = Path::new(env!("CARGO_TARGET_DIR")).join("myapp.exe");
    /// let plan = Plan::new()
    ///     .push(Test::new(&app) // etc.
    /// ```
    ///
    /// The `Plan` returned is expected to be used with the `Plan` builder
    /// methods. For example, use
    /// [`expected_path()`](struct.Plan.html#method.expected_path) or
    /// [`actual_path()`](struct.Plan.html#method.actual_path) to set
    /// non-default paths. For every test required, call
    /// [`push()`](struct.Plan.html#method.push) giving it a newly built
    /// [`Test`](struct.Test.html). And finally, call
    /// [`build()`](struct.Plan.html#method.build) to make the `Plan`
    /// ready for use.
    ///
    /// Note that plans can also be created by reading a [retest plan file
    /// (`.rt`)](http://www.qtrac.eu/retest.html#retestplanfile) using the
    /// [`new_from_rt()`](struct.Plan.html#method.new_from_rt) or
    /// [`new_from_rt_filtered()`](struct.Plan.html#method.new_from_rt_filtered)
    /// methods.
    pub fn new() -> Plan {
        let filename = Some(
            Path::new(&format!(
                "///retest v{}///",
                env!("CARGO_PKG_VERSION")
            ))
            .to_path_buf(),
        );
        Plan {
            filename,
            expected_path: Path::new(RT_EXPECTED_PATH).to_path_buf(),
            actual_path: Path::new(RT_ACTUAL_PATH).to_path_buf(),
            tests: BTreeMap::new(),
            u: 0,
        }
    }

    /// Sets the `Plan`'s _expected_ path to the one given.
    ///
    /// The default expected path is `rt_expected` in the current folder.
    pub fn expected_path<P: AsRef<Path>>(
        &mut self,
        expected_path: P,
    ) -> &mut Self {
        self.expected_path = expected_path.as_ref().to_path_buf();
        self
    }

    /// Sets the `Plan`'s _actual_ path to the one given.
    ///
    /// The default actual path is `rt_actual` in the current folder.
    pub fn actual_path<P: AsRef<Path>>(
        &mut self,
        actual_path: P,
    ) -> &mut Self {
        self.actual_path = actual_path.as_ref().to_path_buf();
        self
    }

    /// Appends a new [`Test`](struct.Test.html) to the plan.
    ///
    /// The first test added is number 1, the second 2, and so on. The
    /// number is used when logging passed, failed, or errored tests.
    pub fn push(&mut self, test: Test) -> &mut Self {
        self.u += 1;
        self.tests.insert(self.u, test);
        self
    }

    /// Returns the `Plan` you've built and on which you can call
    /// [`generate()`](struct.Plan.html#method.generate),
    /// [`retest()`](struct.Plan.html#method.retest), or
    /// [`apply()`](struct.Plan.html#method.apply).
    pub fn build(&mut self) -> Self {
        mem::replace(self, Plan::new())
    }
}

/// Returns `Plan::new()`; see that function's documentation.
impl Default for Plan {
    fn default() -> Self {
        Self::new()
    }
}
