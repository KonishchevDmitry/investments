#[cfg(test)] use std::collections::HashMap;

#[cfg(test)] use crate::commissions::CommissionCalc;
use crate::commissions::{
    CommissionSpec, CommissionSpecBuilder, TradeCommissionSpecBuilder,
    TransactionCommissionSpecBuilder};
#[cfg(test)] use crate::currency::Cash;
use crate::types::TradeType;

pub fn fixed() -> CommissionSpec {
    CommissionSpecBuilder::new("USD")
        .trade(TradeCommissionSpecBuilder::new()
            .commission(TransactionCommissionSpecBuilder::new()
                .minimum(dec!(1))
                .per_share(dec!(0.005))
                .maximum_percent(dec!(1))
                .build().unwrap())

            // Stock selling fee
            .transaction_fee(TradeType::Sell, TransactionCommissionSpecBuilder::new()
                .percent(dec!(0.0013))
                .build().unwrap())

            // FINRA trading activity fee
            .transaction_fee(TradeType::Sell, TransactionCommissionSpecBuilder::new()
                .per_share(dec!(0.000119))
                .build().unwrap())

            .build())
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed() {
        let mut calc = CommissionCalc::new(super::fixed());

        let currency = "USD";
        let date = date!(1, 1, 1);

        let trade_type = TradeType::Buy;

        // Minimum commission > per share commission
        assert_eq!(calc.add_trade(date, trade_type, 199, Cash::new(currency, dec!(100))).unwrap(),
                   Cash::new(currency, dec!(1)));

        // Minimum commission == per share commission
        assert_eq!(calc.add_trade(date, trade_type, 200, Cash::new(currency, dec!(100))).unwrap(),
                   Cash::new(currency, dec!(1)));

        // Per share commission > minimum commission
        assert_eq!(calc.add_trade(date, trade_type, 201, Cash::new(currency, dec!(100))).unwrap(),
                   Cash::new(currency, dec!(1.01)));

        // Per share commission > minimum commission
        assert_eq!(calc.add_trade(date, trade_type, 300, Cash::new(currency, dec!(100))).unwrap(),
                   Cash::new(currency, dec!(1.5)));

        // Per share commission > maximum commission
        assert_eq!(calc.add_trade(date, trade_type, 300, Cash::new(currency, dec!(0.4))).unwrap(),
                   Cash::new(currency, dec!(1.2)));

        let trade_type = TradeType::Sell;

        assert_eq!(calc.add_trade_precise(date, trade_type, 26, Cash::new(currency, dec!(174.2))).unwrap(),
                   Cash::new(currency, dec!(1.0619736)));

        assert_eq!(calc.add_trade(date, trade_type, 26, Cash::new(currency, dec!(174.2))).unwrap(),
                   Cash::new(currency, dec!(1.06)));

        assert_eq!(calc.calculate(), HashMap::new());
    }
}