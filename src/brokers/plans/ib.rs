#[cfg(test)] use std::collections::HashMap;

#[cfg(test)] use crate::commissions::CommissionCalc;
use crate::commissions::{
    CommissionSpec, CommissionSpecBuilder, TradeCommissionSpecBuilder,
    TransactionCommissionSpecBuilder};
#[cfg(test)] use crate::currency::Cash;
#[cfg(test)] use crate::currency::converter::CurrencyConverter;
#[cfg(test)] use crate::types::Decimal;
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
    use rstest::rstest;
    use super::*;

    #[rstest(fraction => [dec!(0), dec!(0.1)])]
    fn fixed(fraction: Decimal) {
        let currency = "USD";
        let date = date!(1, 1, 1);
        let converter = CurrencyConverter::mock();
        let mut calc = CommissionCalc::new(
            converter, super::fixed(), Cash::new(currency, dec!(0))).unwrap();

        let trade_type = TradeType::Buy;
        let shares = |shares| Decimal::from(shares) - fraction;

        // Minimum commission > per share commission
        assert_eq!(calc.add_trade(date, trade_type, shares(199), Cash::new(currency, dec!(100))).unwrap(),
                   Cash::new(currency, dec!(1)));

        // Minimum commission == per share commission
        assert_eq!(calc.add_trade(date, trade_type, shares(200), Cash::new(currency, dec!(100))).unwrap(),
                   Cash::new(currency, dec!(1)));

        // Per share commission > minimum commission
        assert_eq!(calc.add_trade(date, trade_type, shares(201), Cash::new(currency, dec!(100))).unwrap(),
                   Cash::new(currency, dec!(1.01)));

        // Per share commission > minimum commission
        assert_eq!(calc.add_trade(date, trade_type, shares(300), Cash::new(currency, dec!(100))).unwrap(),
                   Cash::new(currency, dec!(1.5)));

        // Per share commission > maximum commission
        assert_eq!(calc.add_trade(date, trade_type, shares(300), Cash::new(currency, dec!(0.4))).unwrap(),
                   Cash::new(currency, dec!(1.2)));

        let trade_type = TradeType::Sell;

        if fraction.is_zero() {
            assert_eq!(calc.add_trade_precise(date, trade_type, shares(26), Cash::new(currency, dec!(174.2))).unwrap(),
                       Cash::new(currency, dec!(1.0619736)));
        }

        assert_eq!(calc.add_trade(date, trade_type, shares(26), Cash::new(currency, dec!(174.2))).unwrap(),
                   Cash::new(currency, dec!(1.06)));

        assert_eq!(calc.calculate().unwrap(), HashMap::new());
    }
}