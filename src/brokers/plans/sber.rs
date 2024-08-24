#[cfg(test)] use crate::commissions::CommissionCalc;
use crate::commissions::{CommissionSpec, CommissionSpecBuilder, CumulativeCommissionSpecBuilder};
#[cfg(test)] use crate::currency::{Cash, converter::CurrencyConverter};
#[cfg(test)] use crate::types::TradeType;
use crate::util::RoundingMethod;

pub fn investment() -> CommissionSpec {
    CommissionSpecBuilder::new("RUB")
        .cumulative(CumulativeCommissionSpecBuilder::new()
            .percent(dec!(0.3))
            .percent_fee(dec!(0.03)) // Estimated exchange fee
            .build())
        .rounding_method(RoundingMethod::ToBigger)
        .build()
}

pub fn manual() -> CommissionSpec {
    CommissionSpecBuilder::new("RUB")
        .cumulative(CumulativeCommissionSpecBuilder::new()
            .volume_tiered(btreemap!{
                         0 => dec!(0.060),
                 1_000_000 => dec!(0.035),
                50_000_000 => dec!(0.018),
            }).unwrap()
            .percent_fee(dec!(0.03)) // Estimated exchange fee
            .build())
        .rounding_method(RoundingMethod::ToBigger)
        .build()
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use super::*;

    #[rstest(trade_type => [TradeType::Buy, TradeType::Sell])]
    fn manual(trade_type: TradeType) {
        let currency = "RUB";

        let converter = CurrencyConverter::mock();
        let mut calc = CommissionCalc::new(
            converter, super::manual(), Cash::zero(currency)).unwrap();

            // FIXME(konishchev): Add more tests
        for &(date, shares, price) in &[
            (date!(2024, 8, 14),  3, dec!(  6.11)),
            (date!(2024, 8, 14), 20, dec!(280.21)),
        ] {
            assert_eq!(
                calc.add_trade(date, trade_type, shares.into(), Cash::new(currency, price)).unwrap(),
                Cash::zero(currency),
            );
        }

        assert_eq!(calc.calculate().unwrap(), hashmap!{
            date!(2024, 8, 14) => Cash::new(currency, dec!(3.37) + dec!(1.70)).into(),
        });
    }
}