// Copyright © 2018-21 Qtrac Ltd. All rights reserved.

pub mod builder;
mod builder_tests;
pub mod counts;
mod diff;
mod parse;
mod process;
pub mod test;
mod test_builder;

pub use counts::Counts;
use diff::diff_kind_for;
pub use diff::DiffKind;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
pub use test::Test;

pub(crate) static RT_EXPECTED_PATH: &str = "rt_expected";
pub(crate) static RT_ACTUAL_PATH: &str = "rt_actual";

/// Holds one or more [`Test`](struct.Test.html)s which can be used to
/// generate expected files or to retest by creating actual files and
/// comparing them with the expecteds.
///
/// To create a new `Plan` from a [retest plan file
/// (`.rt`)](http://www.qtrac.eu/retest.html#retestplanfile), use the
/// [`new_from_rt()`](struct.Plan.html#method.new_from_rt) or
/// [`new_from_rt_filtered()`](struct.Plan.html#method.new_from_rt_filtered)
/// methods.
///
/// See also the [retest plan file
/// example](index.html#retest-plan-file-example).
///
/// To create a new `Plan` programmatically, use
/// [`new()`](struct.Plan.html#method.new) to create an empty
/// `Plan`, and then the builder methods. For example, use
/// [`expected_path()`](struct.Plan.html#method.expected_path) and
/// [`actual_path()`](struct.Plan.html#method.actual_path) to set
/// non-default paths. Then for each test, call
/// [`push()`](struct.Plan.html#method.push). And finally, call
/// [`build()`](struct.Plan.html#method.build).
///
/// See also the [builders example](index.html#builders-example).
///
/// Once a `Plan` has been created either from a `.rt` file or using the
/// builder methods, you can generate or retest by using
/// [`apply()`](struct.Plan.html#method.apply),
/// [`generate()`](struct.Plan.html#method.generate), or
/// [`retest()`](struct.Plan.html#method.retest).
#[derive(Debug)]
pub struct Plan {
    filename: Option<PathBuf>,
    expected_path: PathBuf, // required
    actual_path: PathBuf,   // required
    tests: BTreeMap<u32, Test>,
    u: u32, // test number used by builder methods
}

impl Plan {
    /// Returns `true` if this `Plan` has no tests; otherwise returns `false`.
    pub fn is_empty(&self) -> bool {
        self.tests.is_empty()
    }

    /// Returns how many tests this `Plan` has (which could be 0).
    pub fn len(&self) -> usize {
        self.tests.len()
    }

    /// Returns this test `Plan` (and any tests it contains) as a single
    /// string in [retest plan file
    /// (`.rt`)](http://www.qtrac.eu/retest.html#retestplanfile) format.
    ///
    /// Note that if the plan is created using
    /// [`Plan::new()`](struct.Plan.html#method.new) and if one or more of
    /// builders' tests (created using
    /// [`Test`](struct.Test.html)s) uses the
    /// [`stdin_redirect()`](struct.Test.html#method.stdin_redirect)
    /// method, the relevant plan's tests will have `STDIN` placeholders
    /// like this example:
    ///
    /// ```ignore
    /// STDIN: ///87 raw bytes///
    /// ```
    ///
    /// This is because the bytes are fed directly into the method rather
    /// than read from a file as happens when using a [retest plan file
    /// (`.rt`)](http://www.qtrac.eu/retest.html#retestplanfile).
    pub fn rt(&self) -> String {
        let mut rt = "[ENV]\n".to_string();
        if self.expected_path != Path::new(RT_EXPECTED_PATH) {
            rt.push_str(&format!(
                "EXPECTED_PATH: {}\n",
                &self.expected_path.display()
            ));
        }
        if self.actual_path != Path::new(RT_ACTUAL_PATH) {
            rt.push_str(&format!(
                "ACTUAL_PATH: {}\n",
                &self.actual_path.display()
            ));
        }
        rt.push('\n');
        for (u, test) in &self.tests {
            rt.push_str(&test.rt(*u));
            rt.push('\n');
        }
        rt
    }
}
