use std::collections::HashMap;

use serde::Deserialize;

use crate::broker_statement::open::common::InstrumentType;
use crate::broker_statement::partial::PartialBrokerStatement;
use crate::core::GenericResult;
use crate::exchanges::Exchange;

#[derive(Deserialize)]
pub struct Securities {
    #[serde(rename = "item")]
    securities: Vec<Security>,
}

#[derive(Deserialize)]
struct Security {
    #[serde(rename = "security_name")]
    name: String,
    #[serde(rename = "issuer_name")]
    issuer: String,
    isin: String,
    #[serde(rename = "security_type")]
    type_: String,
    #[serde(rename = "ticker")]
    symbol: String,
    #[serde(rename = "board_name")]
    exchange: String,
}

impl Securities {
    pub fn parse(&self, statement: &mut PartialBrokerStatement) -> GenericResult<HashMap<String, String>> {
        let mut securities = HashMap::new();

        for security in &self.securities {
            let name = match InstrumentType::parse(&security.type_)? {
                InstrumentType::Stock => parse_issuer_name(&security.issuer),
                InstrumentType::DepositaryReceipt => parse_security_name(&security.name),
            };

            let exchange = match security.exchange.as_str() {
                "ПАО Московская биржа" => Exchange::Moex,
                _ => return Err!("Unknown exchange: {:?}", security.exchange),
            };

            if securities.insert(security.name.clone(), security.symbol.clone()).is_some() {
                return Err!("Duplicated security name: {:?}", security.name);
            }

            let instrument = statement.instrument_info.add(&security.symbol)?;
            instrument.set_name(name);
            instrument.add_isin(&security.isin)?;
            instrument.add_exchange(exchange);
        }

        Ok(securities)
    }
}

fn parse_security_name(name: &str) -> &str {
    name.trim_end().trim_end_matches('_')
}

fn parse_issuer_name(mut issuer: &str) -> &str {
    if let Some(index) = issuer.find("п/у") {
        if index != 0 {
            issuer = &issuer[..index];
        }
    }

    if let Some(index) = issuer.find('(') {
        if index != 0 {
            issuer = &issuer[..index];
        }
    }

    issuer.trim()
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use super::*;

    #[rstest(name, expected,
        case("AGRO-гдр  ", "AGRO-гдр"),
        case("FXUK ETF_", "FXUK ETF"),
    )]
    fn security_name_parsing(name: &str, expected: &str) {
        assert_eq!(parse_security_name(name), expected);
    }

    #[test]
    fn issuer_name_parsing() {
        assert_eq!(parse_issuer_name(
            "FinEx MSCI China UCITS ETF (USD Share Class) п/у FinEx Investment Management LLP"),
            "FinEx MSCI China UCITS ETF");
    }
}