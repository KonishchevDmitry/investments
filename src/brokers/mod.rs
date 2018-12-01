use config::{Config, Broker, BrokerConfig};
use core::GenericResult;
use currency::{Cash, CashAssets};
use types::Decimal;

use self::commissions::{CommissionSpec, CommissionSpecBuilder};

mod commissions;

#[derive(Debug, Clone)]
pub struct BrokerInfo {
    pub name: &'static str,
    config: BrokerConfig,
    commission_spec: CommissionSpec,
}

impl BrokerInfo {
    pub fn get(config: &Config, broker: Broker) -> GenericResult<BrokerInfo> {
        match broker {
            Broker::InteractiveBrokers => interactive_brokers(config),
            Broker::OpenBroker => open_broker(config),
        }
    }

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

pub fn interactive_brokers(config: &Config) -> GenericResult<BrokerInfo> {
    let name = "Interactive Brokers";

    Ok(BrokerInfo {
        name: name,
        config: get_broker_config(name, &config.brokers.interactive_brokers)?,
        commission_spec: CommissionSpecBuilder::new()
            .minimum(Cash::new("USD", dec!(1)))
            .per_share(Cash::new("USD", decs!("0.005")))
            .maximum_percent(dec!(1))
            .build().unwrap(),
    })
}

pub fn open_broker(config: &Config) -> GenericResult<BrokerInfo> {
    let name = "Open Broker";

    Ok(BrokerInfo {
        name: name,
        config: get_broker_config(name, &config.brokers.open_broker)?,
        commission_spec: CommissionSpecBuilder::new()
            .minimum(Cash::new("RUB", decs!("0.04")))
            .percent(decs!("0.057"))
            .build().unwrap(),
    })
}

fn get_broker_config(name: &str, broker_config: &Option<BrokerConfig>) -> GenericResult<BrokerConfig> {
    Ok(broker_config.clone().ok_or_else(|| format!(
        "{} configuration is not set in the configuration file", name))?)
}