#[cfg(test)] use crate::commissions::CommissionCalc;
use crate::commissions::{CommissionSpec, CommissionSpecBuilder, CumulativeCommissionSpecBuilder};
#[cfg(test)] use crate::currency::Cash;
#[cfg(test)] use crate::currency::converter::CurrencyConverter;
#[cfg(test)] use crate::types::TradeType;
use crate::util::RoundingMethod;

pub fn investor() -> CommissionSpec {
    CommissionSpecBuilder::new("RUB")
        .cumulative(CumulativeCommissionSpecBuilder::new()
            .percent(dec!(0.1))
            .build())
        .build()
}

// FIXME(konishchev): Support portfolio size tiers
pub fn investor_pro() -> CommissionSpec {
    CommissionSpecBuilder::new("RUB")
        .cumulative(CumulativeCommissionSpecBuilder::new()
            .percent(dec!(0.035))
            .percent_fee(dec!(0.01))
            .monthly_depositary(dec!(299))
            .build())
        .build()
}

pub fn professional() -> CommissionSpec {
    CommissionSpecBuilder::new("RUB")
        .cumulative(CumulativeCommissionSpecBuilder::new()
            .tiers(btreemap!{
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

    // FIXME(konishchev): Add real test data
    #[rstest(trade_type => [TradeType::Buy, TradeType::Sell])]
    fn investor(trade_type: TradeType) {
        let currency = "RUB";
        let converter = CurrencyConverter::mock();
        let mut calc = CommissionCalc::new(
            &converter, super::investor(), Cash::new(currency, dec!(0))).unwrap();

        for &(date, shares, price) in &[
            (date!(13, 10, 2020),  28, dec!(1639.9)),
            (date!(13, 10, 2020), 213, dec!(1640.0)),
            (date!(13, 10, 2020),   2, dec!(1640.0)),
            (date!(13, 10, 2020), 100, dec!(1640.1)),

            (date!(13, 10, 2020), 2549, dec!(4.824)),
            (date!(13, 10, 2020), 2000, dec!(4.824)),
            (date!(13, 10, 2020),   33, dec!(4.824)),
            (date!(13, 10, 2020),  418, dec!(4.824)),
            (date!(13, 10, 2020), 2379, dec!(4.826)),
            (date!(13, 10, 2020),  353, dec!(4.826)),
        ] {
            assert_eq!(
                calc.add_trade(date, trade_type, shares.into(), Cash::new(currency, price)).unwrap(),
                Cash::new(currency, dec!(0)),
            );
        }

        assert_eq!(calc.calculate(), hashmap!{
            date!(13, 10, 2020) => Cash::new(currency, dec!(599.83)),
        });
    }

    #[rstest(trade_type => [TradeType::Buy, TradeType::Sell])]
    fn investor_pro(trade_type: TradeType) {
        let currency = "RUB";
        let converter = CurrencyConverter::mock();
        let mut calc = CommissionCalc::new(
            &converter, super::investor_pro(), Cash::new(currency, dec!(0))).unwrap();

        for &(date, shares, price) in &[
            (date!(13, 10, 2020),  78, dec!(1640.0)),
            (date!(13, 10, 2020),   1, dec!(1640.0)),
            (date!(13, 10, 2020),   9, dec!(1640.1)),
            (date!(13, 10, 2020), 483, dec!(1640.1)),

            (date!(13, 10, 2020), 2645, dec!(4.822)),
            (date!(13, 10, 2020), 1182, dec!(4.824)),
            (date!(13, 10, 2020), 3600, dec!(4.824)),
            (date!(13, 10, 2020), 5671, dec!(4.826)),

            (date!(14, 10, 2020),  100, dec!(4.808)),
        ] {
            assert_eq!(
                calc.add_trade(date, trade_type, shares.into(), Cash::new(currency, price)).unwrap(),
                Cash::new(currency, dec!(0)),
            );
        }

        assert_eq!(calc.calculate(), hashmap!{
            date!(13, 10, 2020) => Cash::new(currency, dec!(99.97) + dec!(349.89)),
            date!(14, 10, 2020) => Cash::new(currency, dec!(0.05) + dec!(0.17)),
            // Actually we have different date, but use fist day of the next month for simplicity
            date!(1,  11, 2020) => Cash::new(currency, dec!(299)),
        });
    }

    #[rstest(trade_type => [TradeType::Buy, TradeType::Sell])]
    fn professional(trade_type: TradeType) {
        let currency = "RUB";
        let converter = CurrencyConverter::mock();
        let mut calc = CommissionCalc::new(
            &converter, super::professional(), Cash::new(currency, dec!(0))).unwrap();

        for &(date, shares, price) in &[
            (date!(2, 12, 2019),  35, dec!(2959.5)),
            (date!(2, 12, 2019),   3, dec!(2960)),
            (date!(2, 12, 2019),  18, dec!(2960)),
            (date!(3, 12, 2019), 107, dec!( 782.4)),
        ] {
            assert_eq!(
                calc.add_trade(date, trade_type, shares.into(), Cash::new(currency, price)).unwrap(),
                Cash::new(currency, dec!(0)),
            );
        }

        assert_eq!(calc.calculate(), hashmap!{
            date!(2, 12, 2019) => Cash::new(currency, dec!(68.45) + dec!(16.57)),
            date!(3, 12, 2019) => Cash::new(currency, dec!(44.45) + dec!(8.37)),

            // Actually we have different date, but use fist day of the next month for simplicity
            date!(1,  1, 2020) => Cash::new(currency,
                dec!(64.10) + // Monthly minimum
                dec!(177) // Monthly depositary
            ),
        });
    }
}