#[cfg(test)] use std::collections::HashMap;

#[cfg(test)] use crate::commissions::CommissionCalc;
use crate::commissions::{
    CommissionSpec, CommissionSpecBuilder, TradeCommissionSpecBuilder,
    TransactionCommissionSpecBuilder, CumulativeCommissionSpecBuilder};
#[cfg(test)] use crate::currency::Cash;
#[cfg(test)] use crate::currency::converter::CurrencyConverter;
#[cfg(test)] use crate::types::TradeType;

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

// FIXME(konishchev): Add test with real data
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
        let converter = CurrencyConverter::mock();
        let mut calc = CommissionCalc::new(
            converter, super::trader(), Cash::new(currency, portfolio_net_value.into())).unwrap();

        let date = date!(22, 6, 2020);

        assert_eq!(
            calc.add_trade(date, trade_type, 22.into(), Cash::new(currency, dec!(3890))).unwrap(),
            Cash::new(currency, dec!(42.79)),
        );

        assert_eq!(
            calc.add_trade(date, trade_type, 3.into(), Cash::new(currency, dec!(3124))).unwrap(),
            Cash::new(currency, dec!(4.69)),
        );

        assert_eq!(
            calc.add_trade(date, trade_type, 3.into(), Cash::new(currency, dec!(2809.5))).unwrap(),
            Cash::new(currency, dec!(4.21)),
        );

        assert_eq!(
            calc.add_trade(date, trade_type, 3.into(), Cash::new(currency, dec!(2196))).unwrap(),
            Cash::new(currency, dec!(3.29)),
        );

        assert_eq!(
            calc.add_trade(date, trade_type, 45.into(), Cash::new(currency, dec!(864.4))).unwrap(),
            Cash::new(currency, dec!(19.45)),
        );

        let mut additional = HashMap::new();
        if depositary != 0 {
            // Actually the date is the date of the first trade
            additional.insert(date!(1, 7, 2020), Cash::new(currency, depositary.into()));
        }
        assert_eq!(calc.calculate(), additional);
    }
}