// Copyright © 2018-21 Qtrac Ltd. All rights reserved.

use crate::util::PathExt;
use crate::xerr;
use crate::xerror::{xerror, XResult};
use crate::Test;
use image::{self, GenericImageView};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The kind of comparison to do when retesting.
///
/// Retest will work this out for itself unless you override it.
///
/// For files with no suffix it assumes `DiffKind::Binary`, and for files
/// with suffix `.jsn` or `.json` it assumes `DiffKind::Json` (and compares
/// the JSON data, ignoring irrelevant whitespace). For common image
/// formats it assumes `DiffKind::Image` and compares pixel for pixel;
/// otherwise it assumes `DiffKind::Text` and compares text (but ignores
/// line-endings).
///
/// You can force the kind of comparison to use by setting the `DiffKind`.
///
/// Or you can force retest to use an external tool by using
/// [`DiffKind::custom(Path)`](enum.DiffKind.html#method.custom) giving it
/// the tool's name.
///
/// Or you can force retest _not_ to compare output (i.e., if you only want
/// to check the exit status/error level), by setting `DiffKind::No`.
///
/// The `DiffKind` is set per-test using the `DIFF` entry in
/// [retest plan files
/// (`.rt`)](http://www.qtrac.eu/retest.html#retestplanfile), or using the
/// [`Test::diff()`](struct.Test.html#method.diff)
/// method.
#[derive(Clone, Debug, PartialEq)]
pub enum DiffKind {
    No,
    Binary,
    Image,
    Json,
    Text,
    Custom(PathBuf),
}

impl DiffKind {
    /// Returns a `DiffKind::Custom` with the given external tool.
    ///
    /// For example, on Unix: `DiffKind::custom("diff")`.
    pub fn custom<P: AsRef<Path>>(diff: P) -> DiffKind {
        DiffKind::Custom(diff.as_ref().to_path_buf())
    }
}

pub(crate) fn is_same(
    test: &Test,
    expected_filename: &Path,
    actual_filename: &Path,
) -> XResult<bool> {
    let kind = match &test.diff {
        Some(kind) => kind.clone(),
        None => guess_kind(expected_filename),
    };
    Ok(match kind {
        DiffKind::No => true, // do nothing; correct exit code is sufficient
        DiffKind::Binary => {
            is_same_binary(expected_filename, actual_filename)?
        }
        DiffKind::Image => {
            is_same_image(expected_filename, actual_filename)?
        }
        DiffKind::Json => {
            is_same_json(expected_filename, actual_filename)?
        }
        DiffKind::Text => {
            is_same_text(expected_filename, actual_filename)?
        }
        DiffKind::Custom(diff) => is_same_custom(&diff, &test.diff_args)?,
    })
}

pub(crate) fn diff_kind_for(diff: &str) -> Option<DiffKind> {
    Some(match diff.to_lowercase().as_str() {
        "0" | "no" | "false" => DiffKind::No,
        "rt-binary" => DiffKind::Binary,
        "rt-json" => DiffKind::Json,
        "rt-image" => DiffKind::Image,
        "rt-text" => DiffKind::Text,
        _ => DiffKind::Custom(Path::new(diff).to_path_buf()),
    })
}

fn guess_kind(filename: &Path) -> DiffKind {
    match filename.extension() {
        None => DiffKind::Binary,
        Some(ext) => {
            match ext.to_string_lossy().to_lowercase().as_str() {
                "json" | "jsn" => DiffKind::Json,
                "png" | "jpg" | "jpeg" | "gif" | "bmp" | "ico"
                | "tif" | "tiff" | "pbm" | "pgm" | "ppm" | "pam" => {
                    DiffKind::Image
                }
                _ => DiffKind::Text,
            }
        }
    }
}

fn is_same_text(
    expected_filename: &Path,
    actual_filename: &Path,
) -> XResult<bool> {
    let expected = &expected_filename.open()?;
    let actual = &actual_filename.open()?;
    let expected_iter = BufReader::new(expected);
    let actual_iter = BufReader::new(actual);
    for (left, right) in expected_iter.lines().zip(actual_iter.lines()) {
        let left = left?;
        let right = right?;
        if left != right {
            return Ok(false);
        }
    }
    Ok(true)
}

fn is_same_binary(
    expected_filename: &Path,
    actual_filename: &Path,
) -> XResult<bool> {
    if expected_filename.len()? != actual_filename.len()? {
        return Ok(false);
    }
    let expected = &expected_filename.open()?;
    let actual = &actual_filename.open()?;
    for (left, right) in expected.bytes().zip(actual.bytes()) {
        let left = left?;
        let right = right?;
        if left != right {
            return Ok(false);
        }
    }
    Ok(true)
}

fn is_same_image(
    expected_filename: &Path,
    actual_filename: &Path,
) -> XResult<bool> {
    let expected = image::open(&expected_filename).or_else(|err| {
        xerr!(
            "Failed to open \"{}\": {}",
            expected_filename.display(),
            err
        )
    })?;
    let actual = image::open(&actual_filename).or_else(|err| {
        xerr!("Failed to open \"{}\": {}", actual_filename.display(), err)
    })?;
    if expected.dimensions() != actual.dimensions() {
        return Ok(false);
    }
    for (left, right) in expected.pixels().zip(actual.pixels()) {
        if left.2 != right.2 {
            // (x, y, pixel)
            return Ok(false);
        }
    }
    Ok(true)
}

fn is_same_json(
    expected_filename: &Path,
    actual_filename: &Path,
) -> XResult<bool> {
    let mut expected = &expected_filename.open()?;
    let mut expected_json = String::new();
    expected.read_to_string(&mut expected_json)?;
    let expected = json::parse(&expected_json)?;
    let mut actual = &actual_filename.open()?;
    let mut actual_json = String::new();
    actual.read_to_string(&mut actual_json)?;
    let actual = json::parse(&actual_json)?;
    Ok(expected == actual)
}

fn is_same_custom(command: &Path, args: &[String]) -> XResult<bool> {
    let mut exe = Command::new(&command);
    exe.args(args).stdout(Stdio::null()).stderr(Stdio::piped());
    let message = format!("{} {}", command.display(), args.join(" "));
    let child = exe
        .spawn()
        .or_else(|err| xerr!("Failed to run {}: {}", &message, err))?;
    let output = child.wait_with_output().or_else(|err| {
        xerr!("Failed to complete {}: {}", &message, err)
    })?;
    if !output.stderr.is_empty() {
        let stderr =
            String::from_utf8_lossy(&output.stderr).trim_end().to_owned();
        let mut out = String::new();
        for c in stderr.chars() {
            if c == '\n' || c == '\r' || out.len() >= 150 {
                break;
            }
            out.push(c);
        }
        if out.len() < stderr.len() {
            out.push_str("...");
        }
        xerr!("{} -> stderr: {}", &message, out);
    }
    match output.status.code() {
        Some(0) => Ok(true), // 0 implies same
        _ => Ok(false),
    }
}
