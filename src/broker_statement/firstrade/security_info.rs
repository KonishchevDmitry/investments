use std::fmt;
use std::collections::HashMap;
use std::collections::hash_map::Entry;

use serde::Deserialize;

use crate::core::{EmptyResult, GenericResult};

pub struct SecurityInfo {
    info: HashMap<SecurityId, SecurityType>
}

pub enum SecurityType {
    Interest,
    Stock(String),
}

impl SecurityInfo {
    fn new() -> SecurityInfo {
        SecurityInfo {
            info: HashMap::new(),
        }
    }

    fn add(&mut self, id: SecurityId, info: SecurityType) -> EmptyResult {
        match self.info.entry(id) {
            Entry::Vacant(entry) => entry.insert(info),
            Entry::Occupied(entry) => return Err!("Got duplicated {} security info", entry.key()),
        };
        Ok(())
    }

    pub fn get(&self, id: &SecurityId) -> GenericResult<&SecurityType> {
        Ok(self.info.get(id).ok_or_else(|| format!("Got an unknown {id} security"))?)
    }
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SecurityInfoSection {
    #[serde(rename = "SECLIST", default)]
    security_list: SecurityList,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct SecurityList {
    #[serde(rename = "STOCKINFO", default)]
    stock_info: Vec<StockInfo>,
    #[serde(rename = "OTHERINFO", default)]
    other_info: Vec<OtherInfo>,
}

impl SecurityInfoSection {
    pub fn parse(self) -> GenericResult<SecurityInfo> {
        let all_info = self.security_list;
        let mut securities = SecurityInfo::new();

        for stock_info in all_info.stock_info {
            let info = stock_info.security_info;
            securities.add(info.id, SecurityType::Stock(info.symbol))?;
        }

        for other_info in all_info.other_info {
            let id = other_info.security_info.id;
            let name = other_info.security_info.name;

            if name.starts_with("INTEREST ON CREDIT BALANCE ") || name.starts_with("FULLYPAID LENDING REBATE ") {
                securities.add(id, SecurityType::Interest)?;
            } else {
                return Err!("Got an unsupported security type: {:?}", name);
            }
        }

        Ok(securities)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StockInfo {
    #[serde(rename = "SECINFO")]
    security_info: SecurityInfoModel,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OtherInfo {
    #[serde(rename = "SECINFO")]
    security_info: SecurityInfoModel,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SecurityInfoModel {
    #[serde(rename = "SECID")]
    id: SecurityId,
    #[serde(rename = "SECNAME")]
    name: String,
    #[serde(rename = "TICKER")]
    symbol: String,
}

#[derive(Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SecurityId {
    #[serde(rename = "UNIQUEID")]
    id: String,
    #[serde(rename = "UNIQUEIDTYPE")]
    _type: String,
}

impl fmt::Display for SecurityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self._type, self.id)
    }
}