use crate::commissions::{
    CommissionSpec, CommissionSpecBuilder, TradeCommissionSpecBuilder,
    TransactionCommissionSpecBuilder, CumulativeCommissionSpecBuilder};

// FIXME(konishchev): Add tests
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