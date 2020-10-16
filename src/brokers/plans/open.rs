#[cfg(test)] use crate::commissions::CommissionCalc;
use crate::commissions::{
    CommissionSpec, CommissionSpecBuilder, TradeCommissionSpecBuilder,
    TransactionCommissionSpecBuilder, CumulativeCommissionSpecBuilder};
#[cfg(test)] use crate::currency::Cash;
#[cfg(test)] use crate::currency::converter::CurrencyConverter;
#[cfg(test)] use crate::types::TradeType;

pub fn iia() -> CommissionSpec {
    CommissionSpecBuilder::new("RUB")
        .trade(TradeCommissionSpecBuilder::new()
            .commission(TransactionCommissionSpecBuilder::new()
                .minimum(dec!(0.04))
                .percent(dec!(0.057))
                .build().unwrap())
            .build())
        .cumulative(CumulativeCommissionSpecBuilder::new()
            .monthly_depositary(dec!(175)).build())
        .build()
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use super::*;

    #[rstest(trade_type => [TradeType::Buy, TradeType::Sell])]
    fn iia(trade_type: TradeType) {
        let currency = "RUB";
        let converter = CurrencyConverter::mock();
        let mut calc = CommissionCalc::new(
            &converter, super::iia(), Cash::new(currency, dec!(0))).unwrap();

        let date = date!(14, 12, 2017);

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

        assert_eq!(calc.calculate(), hashmap!{
            // Actually we have different date, but use fist day of the next month for simplicity
            date!(1, 1, 2018) => Cash::new(currency, dec!(175)), // Depositary commission
        });
    }
}