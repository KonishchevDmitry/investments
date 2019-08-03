pub type EmptyResult = GenericResult<()>;
pub type GenericResult<T> = Result<T, GenericError>;
pub type GenericError = Box<dyn ::std::error::Error + Send + Sync>;

#[cfg(test)]
macro_rules! s {
    ($e:expr) => ($e.to_owned())
}

// TODO: A workaround for IntelliJ Rust plugin
macro_rules! dec {
    ($e:expr) => (::rust_decimal_macros::dec!($e))
}

macro_rules! Err {
    ($($arg:tt)*) => (::std::result::Result::Err(format!($($arg)*).into()))
}