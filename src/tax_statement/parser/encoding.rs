pub trait Type {
    fn decode(data: &str) -> Self;
}

impl Type for String {
    fn decode(data: &str) -> String {
        data.to_owned()
    }
}