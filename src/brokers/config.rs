use std::collections::HashMap;

use serde::Deserialize;

use crate::quotes::tbank::TbankApiConfig;
use crate::types::Decimal;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrokersConfig {
    pub bcs: Option<BrokerConfig>,
    pub firstrade: Option<BrokerConfig>,
    pub interactive_brokers: Option<BrokerConfig>,
    pub open_broker: Option<BrokerConfig>,
    pub sber: Option<BrokerConfig>,
    #[serde(alias = "tinkoff")]
    pub tbank: Option<TbankConfig>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TbankConfig {
    #[serde(flatten)]
    pub broker: Option<BrokerConfig>,
    #[serde(flatten)]
    pub api: Option<TbankApiConfig>,
}

#[derive(Deserialize, Default, Clone)]
#[serde(deny_unknown_fields)]
pub struct BrokerConfig {
    pub deposit_commissions: HashMap<String, TransactionCommissionSpec>,
}

#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct TransactionCommissionSpec {
    pub fixed_amount: Decimal,
}