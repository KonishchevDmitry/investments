#![allow(unused_imports)] // FIXME
#![allow(dead_code)] // FIXME

use matches::matches;
use serde::Deserialize;
use serde::de::{Deserializer, Error as _};

use crate::commissions::{CommissionCalc, CommissionSpec, CommissionSpecBuilder, TradeCommissionSpecBuilder, TransactionCommissionSpecBuilder, CumulativeCommissionSpecBuilder, TransactionCommissionSpec};
use crate::config::{Config, BrokerConfig};
use crate::core::GenericResult;
use crate::currency::{Cash, CashAssets};
use crate::types::{Decimal, TradeType};
use crate::util::RoundingMethod;

use self::commissions::{
    CommissionSpec as OldCommissionSpec,
    CommissionSpecBuilder as OldCommissionSpecBuilder,
};

mod commissions;

#[derive(Debug, Clone, Copy)]
pub enum Broker {
    Bcs,
    InteractiveBrokers,
    OpenBroker,
}

impl Broker {
    pub fn get_info(self, config: &Config) -> GenericResult<BrokerInfo> {
        Ok(BrokerInfo {
            name: self.get_name(),
            config: self.get_config(config)?,
            commission_spec: self.get_commission_spec(),
            allow_sparse_broker_statements: matches!(self, Broker::Bcs),
        })
    }

    fn get_name(self) -> &'static str {
        match self {
            Broker::Bcs => "ООО «Компания БКС»",
            Broker::InteractiveBrokers => "Interactive Brokers LLC",
            Broker::OpenBroker => "АО «Открытие Брокер»",
        }
    }

    fn get_config(self, config: &Config) -> GenericResult<BrokerConfig> {
        Ok(config.brokers.as_ref().and_then(|brokers| {
            match self {
                Broker::Bcs => brokers.bcs.as_ref(),
                Broker::InteractiveBrokers => brokers.interactive_brokers.as_ref(),
                Broker::OpenBroker => brokers.open_broker.as_ref(),
            }
        }).ok_or_else(|| format!(
            "{} configuration is not set in the configuration file", self.get_name())
        )?.clone())
    }

    fn get_commission_spec(self) -> OldCommissionSpec {
        match self {
            // BCS has tiered commissions that aren't supported yet, so use some average now
            Broker::Bcs => OldCommissionSpecBuilder::new()
                .percent(dec!(0.057))
                .build().unwrap(),

            Broker::InteractiveBrokers => OldCommissionSpecBuilder::new()
                .minimum(Cash::new("USD", dec!(1)))
                .per_share(Cash::new("USD", dec!(0.005)))
                .maximum_percent(dec!(1))

                // Stock selling fee
                .transaction_fee(TradeType::Sell, OldCommissionSpecBuilder::new()
                    .percent(dec!(0.0013))
                    .build().unwrap())

                // FINRA trading activity fee
                .transaction_fee(TradeType::Sell, OldCommissionSpecBuilder::new()
                    .per_share(Cash::new("USD", dec!(0.000119)))
                    .build().unwrap())

                .build().unwrap(),

            Broker::OpenBroker => OldCommissionSpecBuilder::new()
                .minimum(Cash::new("RUB", dec!(0.04)))
                .percent(dec!(0.057))
                .build().unwrap(),
        }
    }

    // FIXME: A temporary solution for transition process
    fn get_new_commission_spec(self) -> CommissionSpec {
        match self {
            Broker::Bcs => {
                // FIXME: Support all commissions
                /*
                Урегулирование сделок	0,01

                До 100 000	0,0531
                От 100 000 до 300 000	0,0413
                От 300 000 до 1 000 000	0,0354
                От 1 000 000 до 5 000 000	0,0295
                От 5 000 000 до 15 000 000	0,0236
                Свыше 15 000 000	0,0177
                */
                CommissionSpecBuilder::new("RUB")
                    .rounding_method(RoundingMethod::Truncate)
                    .cumulative(CumulativeCommissionSpecBuilder::new().tiers(btreemap!{
                        dec!(0) => dec!(0.0531) + dec!(0.01),
                        dec!(100_000) => dec!(0.0413) + dec!(0.01),
                    }).unwrap().build())
                    .build()
            },
            Broker::InteractiveBrokers => {
                CommissionSpecBuilder::new("USD")
                    .trade(TradeCommissionSpecBuilder::new()
                        .commission(TransactionCommissionSpecBuilder::new()
                            .minimum(dec!(1))
                            .per_share(dec!(0.005))
                            .maximum_percent(dec!(1))
                            .build().unwrap())

                        // Stock selling fee
                        .transaction_fee(TradeType::Sell, TransactionCommissionSpecBuilder::new()
                            .percent(dec!(0.0013))
                            .build().unwrap())

                        // FINRA trading activity fee
                        .transaction_fee(TradeType::Sell, TransactionCommissionSpecBuilder::new()
                            .per_share(dec!(0.000119))
                            .build().unwrap())

                        .build())
                    .build()
            },
            Broker::OpenBroker => {
                // FIXME: Support depository commission
                CommissionSpecBuilder::new("RUB")
                    .trade(TradeCommissionSpecBuilder::new()
                        .commission(TransactionCommissionSpecBuilder::new()
                            .minimum(dec!(0.04))
                            .percent(dec!(0.057))
                            .build().unwrap())
                        .build())
                    .build()
            },
        }
    }
}

impl<'de> Deserialize<'de> for Broker {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        let value = String::deserialize(deserializer)?;

        Ok(match value.as_str() {
            "bcs" => Broker::Bcs,
            "interactive-brokers" => Broker::InteractiveBrokers,
            "open-broker" => Broker::OpenBroker,

            _ => return Err(D::Error::unknown_variant(&value, &[
                "bcs", "interactive-brokers", "open-broker",
            ])),
        })
    }
}

#[derive(Debug, Clone)]
pub struct BrokerInfo {
    pub name: &'static str,
    config: BrokerConfig,
    commission_spec: OldCommissionSpec,
    pub allow_sparse_broker_statements: bool,
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

    pub fn get_trade_commission(&self, trade_type: TradeType, shares: u32, price: Cash) -> GenericResult<Cash> {
        self.commission_spec.calculate(trade_type, shares, price)
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use super::*;
    use std::collections::HashMap;

    // FIXME: Add more test data
    #[rstest(trade_type => [TradeType::Buy, TradeType::Sell])]
    fn bcs_commission(trade_type: TradeType) {
        let currency = "RUB";
        let mut calc = CommissionCalc::new(Broker::Bcs.get_new_commission_spec());

        for &(date, shares, price) in &[
            (date!(2, 12, 2019),  35, dec!(2959.5)),
            (date!(2, 12, 2019),   3, dec!(2960)),
            (date!(2, 12, 2019),  18, dec!(2960)),
            (date!(3, 12, 2019), 107, dec!( 782.4)),
        ] {
            assert_eq!(
                calc.add_trade(date, trade_type, shares, Cash::new(currency, price)).unwrap(),
                Cash::new(currency, dec!(0)),
            );
        }

        assert_eq!(calc.calculate(), hashmap!{
            date!(2, 12, 2019) => Cash::new(currency, dec!(85.02)),
            date!(3, 12, 2019) => Cash::new(currency, dec!(52.82)),
        });
    }

    #[test]
    fn interactive_brokers_commission() {
        let mut calc = CommissionCalc::new(Broker::InteractiveBrokers.get_new_commission_spec());

        let currency = "USD";
        let date = date!(1, 1, 1);

        let trade_type = TradeType::Buy;

        // Minimum commission > per share commission
        assert_eq!(calc.add_trade(date, trade_type, 199, Cash::new(currency, dec!(100))).unwrap(),
                   Cash::new(currency, dec!(1)));

        // Minimum commission == per share commission
        assert_eq!(calc.add_trade(date, trade_type, 200, Cash::new(currency, dec!(100))).unwrap(),
                   Cash::new(currency, dec!(1)));

        // Per share commission > minimum commission
        assert_eq!(calc.add_trade(date, trade_type, 201, Cash::new(currency, dec!(100))).unwrap(),
                   Cash::new(currency, dec!(1.01)));

        // Per share commission > minimum commission
        assert_eq!(calc.add_trade(date, trade_type, 300, Cash::new(currency, dec!(100))).unwrap(),
                   Cash::new(currency, dec!(1.5)));

        // Per share commission > maximum commission
        assert_eq!(calc.add_trade(date, trade_type, 300, Cash::new(currency, dec!(0.4))).unwrap(),
                   Cash::new(currency, dec!(1.2)));

        let trade_type = TradeType::Sell;

        assert_eq!(calc.add_trade_precise(date, trade_type, 26, Cash::new(currency, dec!(174.2))).unwrap(),
                   Cash::new(currency, dec!(1.0619736)));

        assert_eq!(calc.add_trade(date, trade_type, 26, Cash::new(currency, dec!(174.2))).unwrap(),
                   Cash::new(currency, dec!(1.06)));

        assert_eq!(calc.calculate(), HashMap::new());
    }

    #[rstest(trade_type => [TradeType::Buy, TradeType::Sell])]
    fn open_broker_commission(trade_type: TradeType) {
        let mut calc = CommissionCalc::new(Broker::OpenBroker.get_new_commission_spec());

        let currency = "RUB";
        let date = date!(14, 12, 2017);

        // Percent commission > minimum commission
        assert_eq!(
            calc.add_trade(date, trade_type, 73, Cash::new(currency, dec!(2758))).unwrap(),
            Cash::new(currency, dec!(114.76)),
        );

        // Percent commission < minimum commission
        assert_eq!(
            calc.add_trade(date, trade_type, 1, Cash::new(currency, dec!(1))).unwrap(),
            Cash::new(currency, dec!(0.04)),
        );

        assert_eq!(calc.calculate(), HashMap::new());
    }
}