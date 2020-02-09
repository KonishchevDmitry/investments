#[cfg(test)] use std::fs;
#[cfg(test)] use std::path::Path;

use csv;

use crate::core::{GenericResult};

use super::common::Record;

pub fn try_parse(path: &str) -> GenericResult<bool> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_path(path)?;
    let mut records = reader.records();

    let headers = match records.next() {
        Some(record) => record?,
        None => return Ok(false),
    };
    let headers = headers.iter().collect::<Vec<&str>>();

    // FIXME: Use more reliable conditions
    /*
    "ClientAccountID","AccountAlias","Model","CurrencyPrimary","AssetClass","Symbol","Description",
    "Conid","SecurityID","SecurityIDType","CUSIP","ISIN","ListingExchange","UnderlyingConid",
    "UnderlyingSymbol","UnderlyingSecurityID","UnderlyingListingExchange","Issuer","Multiplier",
    "Strike","Expiry","Put/Call","PrincipalAdjustFactor","TransactionType","TradeID","OrderID",
    "ExecID","BrokerageOrderID","OrderReference","VolatilityOrderLink","ClearingFirmID",
    "OrigTradePrice","OrigTradeDate","OrigTradeID","OrderTime","Date/Time","ReportDate",
    "SettleDate","TradeDate","Exchange","Buy/Sell","Quantity","Price","Amount","Proceeds",
    "Commission","BrokerExecutionCommission","BrokerClearingCommission",
    "ThirdPartyExecutionCommission","ThirdPartyClearingCommission","ThirdPartyRegulatoryCommission",
    "OtherCommission","CommissionCurrency","Tax","Code","OrderType","LevelOfDetail","TraderID",
    "IsAPIOrder","AllocatedTo","AccruedInterest","RFQID"
    */
    if headers.len() >= 2 && headers[1] == "Header" {
        return Ok(false);
    }

    for record in records {
        let record = Record {
            name: "Trade confirmation",
            fields: &headers,
            values: &record?,
        };
    }

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty() {
        let path = Path::new(file!()).parent().unwrap().join(
            "testdata/empty-trade-confirmation.csv");
        assert!(try_parse(path.to_str().unwrap()).unwrap());
    }

    #[test]
    fn parse_real() {
        let mut count = 0;

        // FIXME: testdata/interactive-brokers/current
        for entry in fs::read_dir(".").unwrap() {
            let path = entry.unwrap().path();
            let path = path.to_str().unwrap();

            if !path.ends_with(".csv") {
                continue
            }

            if try_parse(path).unwrap() {
                count += 1;
            }
        }

        assert_ne!(count, 0);
    }
}