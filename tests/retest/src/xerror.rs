// Copyright © 2018-21 Qtrac Ltd. All rights reserved.

use std::error::Error;
use std::fmt;
use std::io;
use std::num;

pub type XResult<T> = Result<T, Box<XError>>;

#[inline]
pub fn xerror<T>(message: impl Into<String>) -> XResult<T> {
    Err(Box::new(XError::new(message)))
}

#[macro_export]
macro_rules! xerr {
 ($msg:expr $(,)?) => (return xerror($msg));
 ($fmt:expr $(, $y:expr)+ $(,)?) => (return xerror(format!($fmt, $($y),*)));
}

#[derive(Debug)]
pub enum XError {
    Error(String),
    Image(image::ImageError),
    Io(::std::io::Error),
    Json(json::Error),
    Log(log::SetLoggerError),
    ParseFloat(::std::num::ParseFloatError),
    ParseInt(::std::num::ParseIntError),
    Rayon(rayon::ThreadPoolBuildError),
}

impl Error for XError {}

impl XError {
    #[inline]
    pub fn new(message: impl Into<String>) -> XError {
        XError::Error(message.into())
    }
}

impl fmt::Display for XError {
    fn fmt(&self, out: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            XError::Error(ref err) => write!(out, "{}", err),
            XError::Image(ref err) => write!(out, "Image error: {}", err),
            XError::Io(ref err) => write!(out, "File error: {}", err),
            XError::Json(ref err) => write!(out, "JSON error: {}", err),
            XError::Log(ref err) => {
                write!(out, "Failed to set logger: {}", err)
            }
            XError::ParseFloat(ref err) => {
                write!(out, "Failed to read decimal number: {}", err)
            }
            XError::ParseInt(ref err) => {
                write!(out, "Failed to read whole number: {}", err)
            }
            XError::Rayon(ref err) => {
                write!(out, "Failed to create thread pool: {}", err)
            }
        }
    }
}

impl From<image::ImageError> for Box<XError> {
    #[inline]
    fn from(err: image::ImageError) -> Box<XError> {
        Box::new(XError::Image(err))
    }
}

impl From<io::Error> for Box<XError> {
    #[inline]
    fn from(err: io::Error) -> Box<XError> {
        Box::new(XError::Io(err))
    }
}

impl From<json::Error> for Box<XError> {
    #[inline]
    fn from(err: json::Error) -> Box<XError> {
        Box::new(XError::Json(err))
    }
}

impl From<log::SetLoggerError> for Box<XError> {
    #[inline]
    fn from(err: log::SetLoggerError) -> Box<XError> {
        Box::new(XError::Log(err))
    }
}

impl From<num::ParseFloatError> for Box<XError> {
    #[inline]
    fn from(err: num::ParseFloatError) -> Box<XError> {
        Box::new(XError::ParseFloat(err))
    }
}

impl From<num::ParseIntError> for Box<XError> {
    #[inline]
    fn from(err: num::ParseIntError) -> Box<XError> {
        Box::new(XError::ParseInt(err))
    }
}

impl From<rayon::ThreadPoolBuildError> for Box<XError> {
    #[inline]
    fn from(err: rayon::ThreadPoolBuildError) -> Box<XError> {
        Box::new(XError::Rayon(err))
    }
}
