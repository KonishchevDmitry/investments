use config::{Config, BrokerConfig};
use core::GenericResult;
use currency::{Cash, CashAssets};
use types::Decimal;

use self::commissions::{CommissionSpec, CommissionSpecBuilder};

mod commissions;

#[derive(Debug)]
pub struct BrokerInfo {
    pub name: &'static str,
    config: BrokerConfig,
    commission_spec: CommissionSpec,
}

impl BrokerInfo {
    pub fn get_deposit_commission(&self, assets: CashAssets) -> GenericResult<Decimal> {
        let currency = assets.cash.currency;

        let commission_spec = match self.config.deposit_commissions.get(currency) {
            Some(commission_spec) => commission_spec,
            None => return Err!(concat!(
                "Unable to calculate commission for {} deposit to {}: there is no commission ",
                "specification in the configuration file"), currency, self.name),
        };

        Ok(commission_spec.fixed_amount)
    }

    pub fn get_trade_commission(&self, shares: u32, price: Cash) -> GenericResult<Cash> {
        self.commission_spec.calculate(shares, price)
    }
}

pub fn interactive_brokers(config: &Config) -> BrokerInfo {
    BrokerInfo {
        name: "Interactive Brokers",
        config: config.interactive_brokers.clone(),
        commission_spec: CommissionSpecBuilder::new()
            .minimum(Cash::new("USD", dec!(1)))
            .per_share(Cash::new("USD", decs!("0.005")))
            .maximum_percent(dec!(1))
            .build().unwrap(),
    }
}