// Copyright © 2018-21 Qtrac Ltd. All rights reserved.

/// Holds the number of generated or retested tests, passes, fails, and
/// errors.
///
/// When generating, `total` is the number of expecteds generated and
/// `failed` is the number that failed; the other fields are 0.
///
/// When retesting, all the fields are used.
#[derive(Debug)]
pub struct Counts {
    pub total: u32,
    pub passed: u32,
    pub failed: u32,
    pub errors: u32,
}

impl Counts {
    #[doc(hidden)]
    pub fn default() -> Counts {
        Counts { total: 0, passed: 0, failed: 0, errors: 0 }
    }
}
