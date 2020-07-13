#[cfg(test)] use std::collections::HashMap;

#[cfg(test)] use crate::commissions::CommissionCalc;
use crate::commissions::{CommissionSpec, CommissionSpecBuilder};
#[cfg(test)] use crate::currency::Cash;
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
        let mut calc = CommissionCalc::new(super::free());

        let currency = "USD";
        let date = date!(1, 1, 1);

        assert_eq!(calc.add_trade(date, trade_type, 100.into(), Cash::new(currency, dec!(100))).unwrap(),
                   Cash::new(currency, dec!(0)));

        assert_eq!(calc.calculate(), HashMap::new());
    }
}