#[cfg(test)] use std::collections::HashMap;

#[cfg(test)] use crate::commissions::CommissionCalc;
use crate::commissions::{
    CommissionSpec, CommissionSpecBuilder, TradeCommissionSpecBuilder,
    TransactionCommissionSpecBuilder, CumulativeCommissionSpecBuilder};
#[cfg(test)] use crate::currency::{Cash, MultiCurrencyCashAccount};
#[cfg(test)] use crate::currency::converter::CurrencyConverter;
#[cfg(test)] use crate::types::TradeType;

pub fn investor() -> CommissionSpec {
    CommissionSpecBuilder::new("RUB")
        .trade(TradeCommissionSpecBuilder::new()
            .commission(TransactionCommissionSpecBuilder::new()
                .percent(dec!(0.3))
                .build().unwrap())
            .build())
        .build()
}

// Please note:
// We don't support Tinkoff volume tiers: actual commission depends on the order of trades which is
// inappropriate for our purposes.
pub fn trader() -> CommissionSpec {
    CommissionSpecBuilder::new("RUB")
        .trade(TradeCommissionSpecBuilder::new()
            .commission(TransactionCommissionSpecBuilder::new()
                .percent(dec!(0.05))
                .build().unwrap())
            .build())
        .cumulative(CumulativeCommissionSpecBuilder::new()
            .monthly_depositary_tiered(btreemap!{
                        0 => dec!(290),
                2_000_000 => dec!(0),
            }).unwrap()
            .build())
        .build()
}

pub fn premium() -> CommissionSpec {
    CommissionSpecBuilder::new("RUB")
        .trade(TradeCommissionSpecBuilder::new()
            .commission(TransactionCommissionSpecBuilder::new()
                .percent(dec!(0.025))
                .build().unwrap())
            .build())
        .build()
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use super::*;

    #[rstest(trade_type => [TradeType::Buy, TradeType::Sell])]
    fn investor(trade_type: TradeType) {
        let converter = CurrencyConverter::mock();
        let mut calc = CommissionCalc::new(
            converter, super::investor(), Cash::new("RUB", dec!(0))).unwrap();

        let date = date!(27, 7, 2020);

        for &(currency, quantity, price, commission) in &[
            ("RUB", 30, dec!(184.69), dec!(16.62)),
            ("USD",  1, dec!( 50.48), dec!( 0.15)),
            ("RUB", 20, dec!(184.84), dec!(11.09)),
            ("USD",  1, dec!( 50.22), dec!( 0.15)),
            ("RUB", 10, dec!(201.43), dec!( 6.04)),
        ] {
            assert_eq!(
                calc.add_trade(date, trade_type, quantity.into(), Cash::new(currency, price)).unwrap(),
                Cash::new(currency, commission),
            );
        }

        assert!(calc.calculate().unwrap().is_empty());
    }

    #[rstest(
        portfolio_net_value_tiers => [
            (-1, 290), (0, 290), (1_999_999, 290),
            (2_000_000, 0), (2_000_001, 0)
        ],
        trade_type => [TradeType::Buy, TradeType::Sell],
    )]
    fn trader(portfolio_net_value_tiers: (i64, u64), trade_type: TradeType) {
        let (portfolio_net_value, depositary) = portfolio_net_value_tiers;

        let currency = "RUB";
        let date = date!(22, 6, 2020);

        let converter = CurrencyConverter::mock();
        let mut calc = CommissionCalc::new(
            converter, super::trader(), Cash::new(currency, portfolio_net_value.into())).unwrap();

        for &(quantity, price, commission) in &[
            (22, dec!(3890  ), dec!(42.79)),
            ( 3, dec!(3124  ), dec!( 4.69)),
            ( 3, dec!(2809.5), dec!( 4.21)),
            ( 3, dec!(2196  ), dec!( 3.29)),
            (45, dec!( 864.4), dec!(19.45)),
        ] {
            assert_eq!(
                calc.add_trade(date, trade_type, quantity.into(), Cash::new(currency, price)).unwrap(),
                Cash::new(currency, commission),
            );
        }

        let mut additional = HashMap::new();
        if depositary != 0 {
            // Actually the date is the date of the first trade
            let commissions = MultiCurrencyCashAccount::new_from(Cash::new(currency, depositary.into()));
            additional.insert(date!(1, 7, 2020), commissions);
        }
        assert_eq!(calc.calculate().unwrap(), additional);
    }

    #[rstest(trade_type => [TradeType::Buy, TradeType::Sell])]
    fn trader_mixed_currency(trade_type: TradeType) {
        let converter = CurrencyConverter::mock();
        let mut calc = CommissionCalc::new(
            converter, super::trader(), Cash::new("RUB", dec!(0))).unwrap();

        let date = date!(10, 12, 2020);
        assert_eq!(
            calc.add_trade(date, trade_type, 100.into(), Cash::new("RUB", dec!(8.09))).unwrap(),
            Cash::new("RUB", dec!(0.4)),
        );

        let date = date!(18, 12, 2020);
        for &(currency, quantity, price, commission) in &[
            ("RUB", 100, dec!(8.125), dec!(0.41)),
            ("USD",   1, dec!(15.78), dec!(0.01)),
            ("USD",   6, dec!(15.81), dec!(0.05)),
            ("USD",   2, dec!(15.81), dec!(0.02)),
        ] {
            assert_eq!(
                calc.add_trade(date, trade_type, quantity.into(), Cash::new(currency, price)).unwrap(),
                Cash::new(currency, commission),
            );
        }

        assert_eq!(calc.calculate().unwrap(), hashmap!{
            // Depositary commission
            // Actually the date is the date of the first trade
            date!(1, 1, 2021) => MultiCurrencyCashAccount::new_from(Cash::new("RUB", dec!(290))),
        });
    }

    #[rstest(trade_type => [TradeType::Buy, TradeType::Sell])]
    fn premium(trade_type: TradeType) {
        let converter = CurrencyConverter::mock();
        let mut calc = CommissionCalc::new(
            converter, super::premium(), Cash::new("RUB", dec!(0))).unwrap();

        let date = date!(20, 2, 2021);

        for &(currency, quantity, price, commission) in &[
            ("RUB",   21, dec!( 4727), dec!( 24.82)),
            ("RUB",   16, dec!( 2859), dec!( 11.44)),
            ("RUB", 6000, dec!(73.81), dec!(110.72)),
        ] {
            assert_eq!(
                calc.add_trade(date, trade_type, quantity.into(), Cash::new(currency, price)).unwrap(),
                Cash::new(currency, commission),
            );
        }

        assert!(calc.calculate().unwrap().is_empty());
    }
}