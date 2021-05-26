#[cfg(test)] use std::collections::HashMap;

#[cfg(test)] use crate::commissions::CommissionCalc;
use crate::commissions::{
    CommissionSpec, CommissionSpecBuilder, TradeCommissionSpecBuilder,
    TransactionCommissionSpecBuilder, CumulativeCommissionSpecBuilder};
#[cfg(test)] use crate::currency::{Cash, MultiCurrencyCashAccount};
#[cfg(test)] use crate::currency::converter::CurrencyConverter;
#[cfg(test)] use crate::types::TradeType;

pub fn all_inclusive() -> CommissionSpec {
    CommissionSpecBuilder::new("RUB")
        .trade(TradeCommissionSpecBuilder::new()
            .commission(TransactionCommissionSpecBuilder::new()
                .percent(dec!(0.05))
                .minimum(dec!(0.04))
                .build().unwrap())
            .build())
        .build()
}

pub fn iia() -> CommissionSpec {
    CommissionSpecBuilder::new("RUB")
        .trade(TradeCommissionSpecBuilder::new()
            .commission(TransactionCommissionSpecBuilder::new()
                .percent(dec!(0.057))
                .minimum(dec!(0.04))
                .build().unwrap())
            .build())
        .cumulative(CumulativeCommissionSpecBuilder::new()
            .monthly_depositary(dec!(175))
            .build())
        .build()
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use super::*;

    #[rstest(trade_type => [TradeType::Buy, TradeType::Sell])]
    fn all_inclusive(trade_type: TradeType) {
        let currency = "RUB";
        let converter = CurrencyConverter::mock();
        let mut calc = CommissionCalc::new(
            converter, super::all_inclusive(), Cash::new(currency, dec!(0))).unwrap();

        let date = date!(2021, 1, 4);

        for &(quantity, price, commission) in &[
            (  1, dec!(4008.00), dec!( 2.00)),
            (  6, dec!(4008.00), dec!(12.02)),
            ( 45, dec!( 942.10), dec!(21.20)),
            ( 25, dec!(5096.00), dec!(63.70)),
            (387, dec!(   5.64), dec!( 1.09)),
        ] {
            assert_eq!(
                calc.add_trade(date, trade_type, quantity.into(), Cash::new(currency, price)).unwrap(),
                Cash::new(currency, commission),
            );
        }

        assert_eq!(calc.calculate().unwrap(), HashMap::new());
    }

    #[rstest(trade_type => [TradeType::Buy, TradeType::Sell])]
    fn iia(trade_type: TradeType) {
        let currency = "RUB";
        let converter = CurrencyConverter::mock();
        let mut calc = CommissionCalc::new(
            converter, super::iia(), Cash::new(currency, dec!(0))).unwrap();

        let date = date!(2017, 12, 14);

        // Percent commission > minimum commission
        assert_eq!(
            calc.add_trade(date, trade_type, 73.into(), Cash::new(currency, dec!(2758))).unwrap(),
            Cash::new(currency, dec!(114.76)),
        );

        // Percent commission < minimum commission
        assert_eq!(
            calc.add_trade(date, trade_type, 1.into(), Cash::new(currency, dec!(1))).unwrap(),
            Cash::new(currency, dec!(0.04)),
        );

        assert_eq!(calc.calculate().unwrap(), hashmap!{
            // Depositary commission
            // Actually we have different date, but use fist day of the next month for simplicity
            date!(2018, 1, 1) => MultiCurrencyCashAccount::new_from(Cash::new(currency, dec!(175))),
        });
    }
}