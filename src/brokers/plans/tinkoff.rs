#[cfg(test)] use crate::commissions::CommissionCalc;
use crate::commissions::{
    CommissionSpec, CommissionSpecBuilder, TradeCommissionSpecBuilder,
    TransactionCommissionSpecBuilder, CumulativeCommissionSpecBuilder};
#[cfg(test)] use crate::currency::Cash;
#[cfg(test)] use crate::types::TradeType;

// FIXME(konishchev): Support Tinkoff tiers
pub fn trader() -> CommissionSpec {
    CommissionSpecBuilder::new("RUB")
        .trade(TradeCommissionSpecBuilder::new()
            .commission(TransactionCommissionSpecBuilder::new()
                .percent(dec!(0.05))
                .build().unwrap())
            .build())
        .cumulative(CumulativeCommissionSpecBuilder::new()
            .monthly_depositary(dec!(290)).build())
        .build()
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use super::*;

    #[rstest(trade_type => [TradeType::Buy, TradeType::Sell])]
    fn trader(trade_type: TradeType) {
        let mut calc = CommissionCalc::new(super::trader());

        let currency = "RUB";
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

        assert_eq!(calc.calculate(), hashmap!{
            // Actually we have different date, but use fist day of the next month for simplicity
            date!(1, 7, 2020) => Cash::new(currency, dec!(290)), // Depositary commission
        });
    }
}