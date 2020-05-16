#[cfg(test)] use std::collections::HashMap;

use matches::matches;
use serde::Deserialize;
use serde::de::{Deserializer, Error as _};

#[cfg(test)] use crate::commissions::CommissionCalc;
use crate::commissions::{
    CommissionSpec, CommissionSpecBuilder, TradeCommissionSpecBuilder,
    TransactionCommissionSpecBuilder, CumulativeCommissionSpecBuilder};
use crate::config::{Config, BrokerConfig};
use crate::core::GenericResult;
#[cfg(test)] use crate::currency::Cash;
use crate::currency::CashAssets;
use crate::types::{Decimal, TradeType};
use crate::util::RoundingMethod;

#[derive(Debug, Clone, Copy)]
pub enum Broker {
    Bcs,
    InteractiveBrokers,
    OpenBroker,
    Tinkoff,
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
            Broker::Tinkoff => "АО «Тинькофф Банк»",
        }
    }

    fn get_config(self, config: &Config) -> GenericResult<BrokerConfig> {
        Ok(config.brokers.as_ref().and_then(|brokers| {
            match self {
                Broker::Bcs => brokers.bcs.as_ref(),
                Broker::InteractiveBrokers => brokers.interactive_brokers.as_ref(),
                Broker::OpenBroker => brokers.open_broker.as_ref(),
                Broker::Tinkoff => brokers.tinkoff.as_ref(),
            }
        }).ok_or_else(|| format!(
            "{} configuration is not set in the configuration file", self.get_name())
        )?.clone())
    }

    fn get_commission_spec(self) -> CommissionSpec {
        match self {
            Broker::Bcs => CommissionSpecBuilder::new("RUB")
                .cumulative(CumulativeCommissionSpecBuilder::new()
                    .tiers(btreemap!{
                        dec!(         0) => dec!(0.0531),
                        dec!(   100_000) => dec!(0.0413),
                        dec!(   300_000) => dec!(0.0354),
                        dec!( 1_000_000) => dec!(0.0295),
                        dec!( 5_000_000) => dec!(0.0236),
                        dec!(15_000_000) => dec!(0.0177),
                    }).unwrap()
                    .minimum_daily(dec!(35.4))
                    .minimum_monthly(dec!(177))
                    .percent_fee(dec!(0.01)) // Exchange fee
                    .monthly_depositary(dec!(177))
                    .build())
                .rounding_method(RoundingMethod::Truncate)
                .build(),

            Broker::InteractiveBrokers => CommissionSpecBuilder::new("USD")
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
                .build(),

            // FIXME(konishchev): Add Tinkoff commission
            Broker::OpenBroker | Broker::Tinkoff => CommissionSpecBuilder::new("RUB")
                .trade(TradeCommissionSpecBuilder::new()
                    .commission(TransactionCommissionSpecBuilder::new()
                        .minimum(dec!(0.04))
                        .percent(dec!(0.057))
                        .build().unwrap())
                    .build())
                .cumulative(CumulativeCommissionSpecBuilder::new()
                    .monthly_depositary(dec!(175)).build())
                .build(),
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
            "tinkoff" if cfg!(debug_assertions) => Broker::Tinkoff,  // FIXME(konishchev): Support

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
    pub commission_spec: CommissionSpec,
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
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use super::*;

    #[rstest(trade_type => [TradeType::Buy, TradeType::Sell])]
    fn bcs_commission(trade_type: TradeType) {
        let mut calc = CommissionCalc::new(Broker::Bcs.get_commission_spec());

        let currency = "RUB";
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
            date!(2, 12, 2019) => Cash::new(currency, dec!(68.45) + dec!(16.57)),
            date!(3, 12, 2019) => Cash::new(currency, dec!(44.45) + dec!(8.37)),

            // Actually we have different dates, but use fist day of the next month for simplicity
            date!(1,  1, 2020) => Cash::new(currency,
                dec!(64.10) + // Monthly minimum
                dec!(177) // Monthly depositary
            ),
        });
    }

    #[test]
    fn interactive_brokers_commission() {
        let mut calc = CommissionCalc::new(Broker::InteractiveBrokers.get_commission_spec());

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
        let mut calc = CommissionCalc::new(Broker::OpenBroker.get_commission_spec());

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

        assert_eq!(calc.calculate(), hashmap!{
            // Actually we have different date, but use fist day of the next month for simplicity
            date!(1, 1, 2018) => Cash::new(currency, dec!(175)), // Depositary commission
        });
    }
}