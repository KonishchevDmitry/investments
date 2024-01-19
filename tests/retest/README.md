# Retest

Retest is used to automate black box regression testing.

Retest is provided both as an application, and as a library.

## Retest Application

The retest application works by reading a retest plan (`.rt` plain text
file) and either generating expected files or generating actual files
and comparing them with previously generated expecteds, reporting any
discrepencies. (It can also be used purely to generate files.)

All you need to do to use retest (beyond the easy one-off process of
installing it), is to create a suitable retest plan file for each
application you want to test.

For documentation, source code, and precompiled `retest.exe` for 64-bit
Windows, visit the [retest home page](http://www.qtrac.eu/retest.html).

If you have rust installed, the `retest` application can be downloaded and
installed using `cargo install qtrac-retest` (the executable is called
`retest` or `retest.exe`).

## Retest Library

Retest can also be used as a rust library. This provides two APIs, one for
creating retest plans using retest plan (`.rt`) files, and the other for
creating plans using pure code. Once a plan is created by either API it
can then be generated or retested.

For your `Cargo.toml` we recommend using:
```toml
[dependencies]
retest = { package = "qtrac-retest", version = "4" }
```

Then, in your crate root, for Rust 2015 add `extern crate retest`, and for
Rust 2018 or later simply add `use retest`.

[crates.io](https://crates.io/crates/qtrac-retest)
[docs](https://docs.rs/qtrac-retest/latest/retest/)

## License

Retest is free open source software (FOSS) licensed under the GNU
General Public License version 3 (GPLv3).
