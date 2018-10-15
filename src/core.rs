pub type EmptyResult = GenericResult<()>;
pub type GenericResult<T> = Result<T, GenericError>;
pub type GenericError = Box<::std::error::Error + Send + Sync>;

#[cfg(test)]
macro_rules! s {
    ($e:expr) => ($e.to_owned())
}

macro_rules! Err {
    ($($arg:tt)*) => (::std::result::Result::Err(format!($($arg)*).into()))
}