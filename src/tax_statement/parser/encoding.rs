use core::GenericResult;

pub trait Type: Sized {
    fn decode(data: &str) -> GenericResult<Self>;
}

impl Type for String {
    fn decode(data: &str) -> GenericResult<String> {
        Ok(data.to_owned())
    }
}