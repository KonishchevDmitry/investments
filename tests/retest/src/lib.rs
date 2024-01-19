// Copyright © 2018-21 Qtrac Ltd. All rights reserved.

/*!

The `retest` crate is used to automate black box regression testing.

There is also a [retest](http://www.qtrac.eu/retest.html) executable
(available precompiled for 64-bit Windows; in source form for other
platforms). This uses [retest plan files
(`.rt`)](http://www.qtrac.eu/retest.html#retestplanfile).

There are two phases for regression testing.

1. First there is the **generate** phase. Here, the application to test is
   run and the output (whether a file or captured `stdout`) is saved in an
   _expected_ folder. This is typically done initially, and subsequently
   whenever new tests are added.

2. Second there's the **testing** phase. Here, the application to test is
   run and the output is saved to an _actual_ folder. Then the
   corresponding outputs in the _expected_ and _actual_ folder are
   compared, and any discrepencies reported. This is typically done many
   times.

Note also that it is also possible to just check the application to test's
exit code/error level, if that's all that is required.

# Retest's two APIs

See also, [Notes for Upgraders](index.html#notes-for-upgraders).

## Using Retest Plan Files

This API uses the same [retest plan files
(`.rt`)](http://www.qtrac.eu/retest.html#retestplanfile) used by the
[retest](http://www.qtrac.eu/retest.html) executable. It offers these
benefits: you can write the plan files independently of your code, you can
generate the initial test expecteds and any subsequent ones using the
retest executable, and you can test using the retest executable, making
the whole process easier to debug.

This API is provided by the
[`Plan::new_from_rt()`](struct.Plan.html#method.new_from_rt) and
[`Plan::new_from_rt_filtered()`](struct.Plan.html#method.new_from_rt_filtered)
constructors.

## Using Builders

This API uses builder methods which means you don't have to learn how to
create [retest plan files
(`.rt`)](http://www.qtrac.eu/retest.html#retestplanfile), and may be easier
for integrating retest regression testing with other tools.

This API is provided by two separate builders that are used together. An
overall test [`Plan`](struct.Plan.html) is created using
[`Plan::new()`](struct.Plan.html#method.new) and the `Plan` builder
methods. Then the individual tests are created using
[`Test::new()`](struct.Test.html#method.new) and the `Test` builder
methods. Each test is added to the [`Plan`](struct.Plan.html) using the
[`Plan::push()`](struct.Plan.html#method.push) method.

# Generating and Retesting

Once a [`Plan`](struct.Plan.html) has been created (and populated with
tests) using either API, you can generate expecteds using the
[`Plan::generate()`](struct.Plan.html#method.generate) method, or test and
compare using the [`Plan::retest()`](struct.Plan.html#method.retest)
method. There is also a [`Plan::apply()`](struct.Plan.html#method.apply)
method which will generate or retest depending on the `RETEST_GENERATE`
environment variable.

During generating or retesting, logging output is produced (or not)
depending on the logger you are using and the logging level you have set.
And when finished, both these methods return a
[`Counts`](struct.Counts.html) struct that summarizes the results.

# Dependencies

To use retest, add this line your `Cargo.toml` file's `[dependencies]`
section:

```toml,ignore
retest = { package = "qtrac-retest", version = "4" }
```

Then, in your crate root, for Rust 2015 add `extern crate retest`, and for
Rust 2018 add `use retest`.

Note that the retest crate writes all its output to the current logger, so
a logger must be available. (See the
[log](https://github.com/rust-lang-nursery/log) crate and, for example, the
[simplelog](https://github.com/drakulix/simplelog.rs) crate.)

# Examples

## Retest Plan File Example

```rust,text,ignore
// === This is just a skeleton, so won't work as-is ===
use retest::{Counts, Plan, XResult};
// = use the log module of your choice =
// = initialize the logger =

fn manage_tests(rt_filename: &str, generate: bool) -> XResult<Counts> {
    let plan = Plan::new_from_rt(&rt_filename)?;
    if generate {
        plan.generate()
    } else {
        plan.retest()
    }
}
```
This code will generate or retest every test it finds in the `.rt` file
that's specified. If you want to limit generating or retesting to one or
more specific tests you can do so by creating a `BTreeSet<u32>` of the
test numbers you want to generate or retest, and using the
[`Plan::new_from_rt_filtered()`](struct.Plan.html#method.new_from_rt_filtered)
method.

The `XResult` type is an alias for `Result<T, XError>`. The `XError` type
is just a wrapper around the various errors retest can encounter.

For a complete example of this use case see the source code's
`src/bin/retest/main.rs` file.

## Builders Example

```rust,text,ignore
// === This is just a skeleton, so won't work as-is ===
use retest::{Counts, DiffKind, Plan, Test, XResult};
// = use the log module of your choice =
// = initialize the logger =

fn run_tests() -> XResult<Counts> {
    let app = r"V:\bin\myapp.exe";
    let plan = Plan::new()
        .expected_path(r"U:\myapp\expected")
        .actual_path(r"U:\myapp\actual")
        .push(Test::new(&app)
              .name("Text test")
              .args(&["-v", "--format=light", r"$OUT_PATH\01.txt"])
              .build())
        .push(Test::new(&app)
              .name("Bad error level test")
              .exit_code(1)
              .build())
        .push(Test::new(&app)
              .name("Compare PDFs test")
              .args(&["/out", r"$OUT_PATH\03.pdf")
              .diff(DiffKind::custom(
                  r"C:\bin\comparepdfcmd\comparepdfcmd.exe")
              .diff_args(&[r"$EXPECTED_PATH\03.pdf",
                           r"$ACTUAL_PATH\03.pdf"])
              .build())
        .build();
    plan.retest() // Could generate using plan.generate().
    // Or could use plan.apply() which will generate or retest
    // depending on the RETEST_GENERATE environment variable.
}
```

For an example of this use case see the source code's
`src/plan/builder_tests.rs` file's “plan13” test.

Note that the `Plan` and `Test` builder methods can be used as setters,
which is useful in conditionals, e.g.,
```rust,ignore
let mut planner = Plan::new();
planner.expected_path("/home/user/myapp/expected");
if !custom_actual_path.is_empty() {
    planner.actual_path(&custom_actual_path);
}
let mut test = Test::new("myapp");
test.name("Help Test");
test.args(&["--help"]);
if expect_invalid {
    test.exit_code(2);
}
planner.push(test.build());
// Create and push more tests...
let plan = planner.build();
plan.retest();
```

For a small example of this use case see the source code's
`src/plan/builder_tests.rs` file's “plan14” test.

# Notes for Upgraders

## Upgrading from 3._x_ to 4._x_

Remove any calls to `Plan::app_path_env_var()`. Instead, when creating a
new `Test` give the `app` with a full path, e.g.,
`Path::new(env!("CARGO_TARGET_DIR")).join("myapp.exe")`, unless it is in
the`PATH` already.

## Upgrading from 2._x_ to 3._x_

- Rename `PlanBuilder` to `Plan`;
- Rename `Plan`'s `append()` method to `push()`.
- Replace `DiffKind::Custom` with `DiffKind::custom`, e.g.:
    - `DiffKind::Custom("`_diff_`".to_string()) //` _old-style_
    - `DiffKind::custom("`_diff_`") //` _new-style_

Naturally, if you recompile, rust will find all the places these changes
are needed.

New methods:
- [`DiffKind::custom()`](enum.DiffKind.html#method.custom)
- [`Plan::apply()`](struct.Plan.html#method.apply)

## Upgrading from 1._x_ to 2._x_

- Rename `TestBuilder` to `Test`;
- Rename `Test`'s `diff_by()` to `diff()`;
- Ensure that `diff()` is passed a `DiffKind` rather than a `&str`.

# License

Retest is free open source software (FOSS) licensed under the GNU
General Public License version 3 (GPLv3).

*/

mod plan;
mod util;
mod xerror;

pub use plan::{Counts, DiffKind, Plan, Test};
#[doc(hidden)]
pub use util::maybe_s;
pub use xerror::{xerror, XResult};
