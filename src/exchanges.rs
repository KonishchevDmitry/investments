#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub enum Exchange {
    Moex,
    Spb,
}

pub enum InstrumentType {
    Currency,
    Stock,
}