use crate::core::EmptyResult;
use crate::currency::{Cash, CashAssets};
use crate::util::DecimalRestrictions;

use super::StatementParser;
use super::common::{Record, RecordParser};

pub struct CashReportParser {}

impl RecordParser for CashReportParser {
    fn parse(&self, parser: &mut StatementParser, record: &Record) -> EmptyResult {
        if record.get_value("Currency Summary")? != "Ending Cash" {
            return Ok(());
        }

        let currency = record.get_value("Currency")?;
        let amount = record.parse_amount("Total", DecimalRestrictions::No)?;

        record.check_value("Futures", "0")?;
        record.check_value("Total", record.get_value("Securities")?)?;

        if currency == "Base Currency Summary" {
            let summary = Cash::new(parser.base_currency()?, amount);
            if parser.base_currency_summary.replace(summary).is_some() {
                return Err!("Got duplicated base currency summary");
            }
        } else {
            if parser.statement.cash_assets.has_assets(currency) {
                return Err!("Got duplicated {} assets", currency);
            }
            parser.statement.cash_assets.deposit(Cash::new(currency, amount));
        }

        Ok(())
    }
}

pub struct DepositsAndWithdrawalsParser {}

impl RecordParser for DepositsAndWithdrawalsParser {
    fn skip_totals(&self) -> bool {
        true
    }

    fn parse(&self, parser: &mut StatementParser, record: &Record) -> EmptyResult {
        let currency = record.get_value("Currency")?;
        let date = record.parse_date("Settle Date")?;
        let amount = record.parse_cash("Amount", currency, DecimalRestrictions::NonZero)?;
        parser.statement.cash_flows.push(CashAssets::new_from_cash(date, amount));
        Ok(())
    }
}