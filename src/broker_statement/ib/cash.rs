use crate::core::EmptyResult;
use crate::currency::{Cash, CashAssets};
use crate::util::{self, DecimalRestrictions};

use super::StatementParser;
use super::cash_flows::CashFlowId;
use super::common::{Record, RecordParser, parse_date};

pub struct CashReportParser {}

impl RecordParser for CashReportParser {
    fn parse(&mut self, parser: &mut StatementParser, record: &Record) -> EmptyResult {
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
            let cash_assets = parser.statement.cash_assets.get_or_insert_with(Default::default);
            if cash_assets.has_assets(currency) {
                return Err!("Got duplicated {} assets", currency);
            }
            cash_assets.deposit(Cash::new(currency, amount));
        }

        Ok(())
    }
}

pub struct DepositsAndWithdrawalsParser {}

impl RecordParser for DepositsAndWithdrawalsParser {
    fn skip_totals(&self) -> bool {
        true
    }

    fn parse(&mut self, parser: &mut StatementParser, record: &Record) -> EmptyResult {
        let currency = record.get_value("Currency")?;
        let date = record.parse_date("Settle Date")?;
        let amount = record.parse_cash("Amount", currency, DecimalRestrictions::NonZero)?;
        parser.statement.deposits_and_withdrawals.push(CashAssets::new_from_cash(date, amount));
        Ok(())
    }
}

pub struct StatementOfFundsParser {}

impl RecordParser for StatementOfFundsParser {
    fn parse(&mut self, parser: &mut StatementParser, record: &Record) -> EmptyResult {
        let base_currency = parser.base_currency()?;

        let mut currency = record.get_value("Currency")?;
        if currency == "Base Currency Summary" {
            currency = base_currency;
        } else if currency == base_currency {
            // It duplicates "Base Currency Summary". Statements with one currency contain only
            // "Base Currency Summary" info.
            return Ok(());
        }

        let statement_date = record.get_value("Activity Date")?;
        if statement_date.is_empty() {
            return Ok(()); // Opening Balance / Closing Balance / FX Translation P&L
        }
        let statement_date = parse_date(statement_date)?;

        let date = record.parse_date("Report Date")?;
        let description = record.get_value("Description")?;

        let debit = record.get_value("Debit")?.trim_start();
        let credit = record.get_value("Credit")?.trim_start();
        let (amount, restrictions) = match (debit.is_empty(), credit.is_empty()) {
            (false, true) => (debit, DecimalRestrictions::StrictlyNegative),
            (true, false) => (credit, DecimalRestrictions::StrictlyPositive),
            _ => return Err!("Got an unexpected debit + credit combination"),
        };
        let amount = Cash::new(currency, util::parse_decimal(amount, restrictions)?);

        let id = CashFlowId::new(statement_date, description, amount);
        parser.cash_flows.add(id, date);

        Ok(())
    }
}