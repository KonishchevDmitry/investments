#[cfg(test)] use crate::commissions::CommissionCalc;
use crate::commissions::{CommissionSpec, CommissionSpecBuilder, CumulativeCommissionSpecBuilder};
#[cfg(test)] use crate::currency::{Cash, converter::CurrencyConverter};
#[cfg(test)] use crate::types::TradeType;
use crate::util::RoundingMethod;

pub fn investor() -> CommissionSpec {
    CommissionSpecBuilder::new("RUB")
        .cumulative(CumulativeCommissionSpecBuilder::new()
            .percent(dec!(0.3))
            .build())
        .build()
}

#[cfg(test)]
fn investor_deprecated() -> CommissionSpec {
    CommissionSpecBuilder::new("RUB")
        .cumulative(CumulativeCommissionSpecBuilder::new()
            .percent(dec!(0.1))
            .build())
        .build()
}

pub fn trader() -> CommissionSpec {
    CommissionSpecBuilder::new("RUB")
        .cumulative(CumulativeCommissionSpecBuilder::new()
            .portfolio_net_value_tiered(btreemap!{
                         0 => dec!(0.0300),
                 2_500_000 => dec!(0.0200),
                 5_000_000 => dec!(0.0150),
                10_000_000 => dec!(0.0125),
                30_000_000 => dec!(0.0100),
            }).unwrap()
            .percent_fee(dec!(0.02))
            .monthly_depositary(dec!(299))
            .build())
        .build()
}

pub fn investor_pro_deprecated() -> CommissionSpec {
    CommissionSpecBuilder::new("RUB")
        .cumulative(CumulativeCommissionSpecBuilder::new()
            .portfolio_net_value_tiered(btreemap!{
                         0 => dec!(0.300),
                   900_000 => dec!(0.035),
                 2_500_000 => dec!(0.030),
                 5_000_000 => dec!(0.025),
                10_000_000 => dec!(0.020),
                30_000_000 => dec!(0.015),
            }).unwrap()
            .percent_fee(dec!(0.01))
            .monthly_depositary(dec!(299))
            .build())
        .build()
}

pub fn professional_deprecated() -> CommissionSpec {
    CommissionSpecBuilder::new("RUB")
        .cumulative(CumulativeCommissionSpecBuilder::new()
            .volume_tiered(btreemap!{
                         0 => dec!(0.0531),
                   100_000 => dec!(0.0413),
                   300_000 => dec!(0.0354),
                 1_000_000 => dec!(0.0295),
                 5_000_000 => dec!(0.0236),
                15_000_000 => dec!(0.0177),
            }).unwrap()
            .minimum_daily(dec!(35.4))
            .minimum_monthly(dec!(177))
            .percent_fee(dec!(0.01)) // Exchange fee
            .monthly_depositary(dec!(177))
            .build())
        .rounding_method(RoundingMethod::Truncate)
        .build()
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use super::*;

    #[rstest(trade_type => [TradeType::Buy, TradeType::Sell])]
    fn investor_deprecated(trade_type: TradeType) {
        let currency = "RUB";
        let converter = CurrencyConverter::mock();
        let mut calc = CommissionCalc::new(
            converter, super::investor_deprecated(), Cash::zero(currency)).unwrap();

        for &(date, shares, price) in &[
            (date!(2020, 10, 13),  28, dec!(1639.9)),
            (date!(2020, 10, 13), 213, dec!(1640.0)),
            (date!(2020, 10, 13),   2, dec!(1640.0)),
            (date!(2020, 10, 13), 100, dec!(1640.1)),

            (date!(2020, 10, 13), 2549, dec!(4.824)),
            (date!(2020, 10, 13), 2000, dec!(4.824)),
            (date!(2020, 10, 13),   33, dec!(4.824)),
            (date!(2020, 10, 13),  418, dec!(4.824)),
            (date!(2020, 10, 13), 2379, dec!(4.826)),
            (date!(2020, 10, 13),  353, dec!(4.826)),

            (date!(2020, 10, 14),  100, dec!(4.808)),
        ] {
            assert_eq!(
                calc.add_trade(date, trade_type, shares.into(), Cash::new(currency, price)).unwrap(),
                Cash::zero(currency),
            );
        }

        assert_eq!(calc.calculate().unwrap(), hashmap!{
            date!(2020, 10, 13) => Cash::new(currency, dec!(599.83)).into(),
            date!(2020, 10, 14) => Cash::new(currency, dec!(  0.48)).into(),
        });
    }

    #[rstest(trade_type => [TradeType::Buy, TradeType::Sell])]
    fn investor_pro_deprecated(trade_type: TradeType) {
        let currency = "RUB";
        let converter = CurrencyConverter::mock();
        let mut calc = CommissionCalc::new(
            converter, super::investor_pro_deprecated(), Cash::new(currency, dec!(1_000_000))).unwrap();

        for &(date, shares, price) in &[
            (date!(2020, 10, 13),  78, dec!(1640.0)),
            (date!(2020, 10, 13),   1, dec!(1640.0)),
            (date!(2020, 10, 13),   9, dec!(1640.1)),
            (date!(2020, 10, 13), 483, dec!(1640.1)),

            (date!(2020, 10, 13), 2645, dec!(4.822)),
            (date!(2020, 10, 13), 1182, dec!(4.824)),
            (date!(2020, 10, 13), 3600, dec!(4.824)),
            (date!(2020, 10, 13), 5671, dec!(4.826)),

            (date!(2020, 10, 14),  100, dec!(4.808)),
        ] {
            assert_eq!(
                calc.add_trade(date, trade_type, shares.into(), Cash::new(currency, price)).unwrap(),
                Cash::zero(currency),
            );
        }

        assert_eq!(calc.calculate().unwrap(), hashmap!{
            date!(2020, 10, 13) => Cash::new(currency, dec!(99.97) + dec!(349.89)).into(),
            date!(2020, 10, 14) => Cash::new(currency, dec!(0.05) + dec!(0.17)).into(),
            // Actually we have different date, but use fist day of the next month for simplicity
            date!(2020, 11,  1) => Cash::new(currency, dec!(299)).into(),
        });
    }

    #[rstest(trade_type => [TradeType::Buy, TradeType::Sell])]
    fn professional_deprecated(trade_type: TradeType) {
        let currency = "RUB";
        let converter = CurrencyConverter::mock();
        let mut calc = CommissionCalc::new(
            converter, super::professional_deprecated(), Cash::zero(currency)).unwrap();

        for &(date, shares, price) in &[
            (date!(2019, 12, 2),  35, dec!(2959.5)),
            (date!(2019, 12, 2),   3, dec!(2960)),
            (date!(2019, 12, 2),  18, dec!(2960)),
            (date!(2019, 12, 3), 107, dec!( 782.4)),
        ] {
            assert_eq!(
                calc.add_trade(date, trade_type, shares.into(), Cash::new(currency, price)).unwrap(),
                Cash::zero(currency),
            );
        }

        assert_eq!(calc.calculate().unwrap(), hashmap!{
            date!(2019, 12, 2) => Cash::new(currency, dec!(68.45) + dec!(16.57)).into(),
            date!(2019, 12, 3) => Cash::new(currency, dec!(44.45) + dec!(8.37)).into(),

            // Actually we have different date, but use fist day of the next month for simplicity
            date!(2020, 1, 1) => Cash::new(currency,
                dec!(64.10) + // Monthly minimum
                dec!(177) // Monthly depositary
            ).into(),
        });
    }
}