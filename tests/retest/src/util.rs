// Copyright © 2018-21 Qtrac Ltd. All rights reserved.

use crate::xerr;
use crate::xerror::{xerror, XResult};
use num_traits::identities::one;
use std::cmp;
use std::fs::{self, File};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub enum Which {
    Expected,
    Actual,
}

pub fn maybe_s<'a, T>(n: T) -> &'a str
where
    T: num::Num + cmp::PartialOrd,
{
    if n == one() {
        ""
    } else {
        "s"
    }
}

pub trait PathExt {
    fn is_empty(&self) -> bool;
    fn len(&self) -> XResult<u64>;
    fn open(&self) -> XResult<File>;
}

impl PathExt for Path {
    fn is_empty(&self) -> bool {
        self.as_os_str().is_empty()
    }

    fn len(&self) -> XResult<u64> {
        Ok(fs::metadata(&self)
            .or_else(|err| {
                xerr!(
                    "Failed to read metadata from \"{}\": {}",
                    self.display(),
                    err
                )
            })?
            .len())
    }

    fn open(&self) -> XResult<File> {
        File::open(&self).or_else(|err| {
            xerr!("Failed to open \"{}\": {}", self.display(), err)
        })
    }
}

pub trait PathBufExt {
    fn is_empty(&self) -> bool;
    fn replace(&self, from: &str, to: &Path) -> PathBuf;
}

impl PathBufExt for PathBuf {
    fn is_empty(&self) -> bool {
        self.as_os_str().is_empty()
    }

    // Only works for UTF-8 encoded paths
    fn replace(&self, from: &str, to: &Path) -> PathBuf {
        PathBuf::from(
            self.to_string_lossy().replace(&from, &to.to_string_lossy()),
        )
    }
}
