#[cfg(test)] use std::collections::HashMap;

#[cfg(test)] use crate::commissions::CommissionCalc;
use crate::commissions::{CommissionSpec, CommissionSpecBuilder};
#[cfg(test)] use crate::currency::Cash;
#[cfg(test)] use crate::currency::converter::CurrencyConverter;
#[cfg(test)] use crate::types::TradeType;

pub fn free() -> CommissionSpec {
    CommissionSpecBuilder::new("USD").build()
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use super::*;

    #[rstest(trade_type => [TradeType::Buy, TradeType::Sell])]
    fn free(trade_type: TradeType) {
        let currency = "USD";
        let converter = CurrencyConverter::mock();
        let mut calc = CommissionCalc::new(
            converter, super::free(), Cash::zero(currency)).unwrap();

        let date = date!(1, 1, 1);
        assert_eq!(calc.add_trade(date, trade_type, 100.into(), Cash::new(currency, dec!(100))).unwrap(),
                   Cash::zero(currency));

        assert_eq!(calc.calculate().unwrap(), HashMap::new());
    }
}