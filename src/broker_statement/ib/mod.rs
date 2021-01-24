mod cash;
mod common;
mod confirmation;
mod corporate_actions;
mod dividends;
mod fees;
mod interest;
mod instruments;
mod summary;
mod taxes;
mod trades;

use std::cell::RefCell;
use std::iter::Iterator;

#[cfg(test)] use chrono::Datelike;
use csv::{self, StringRecord};
use log::{trace, warn};

#[cfg(test)] use crate::brokers::Broker;
#[cfg(test)] use crate::config::Config;
use crate::core::{GenericResult, EmptyResult};
use crate::currency::Cash;
use crate::formatting::format_date;
use crate::taxes::TaxRemapping;
use crate::types::Date;

#[cfg(test)] use super::{BrokerStatement};
use super::{BrokerStatementReader, PartialBrokerStatement};

use self::common::{RecordSpec, Record, RecordParser, format_record};
use self::confirmation::{TradeExecutionDates, OrderId};

pub struct StatementReader {
    tax_remapping: RefCell<TaxRemapping>,
    trade_execution_dates: RefCell<TradeExecutionDates>,

    warn_on_margin_account: bool,
    warn_on_missing_execution_date: bool,
}

impl StatementReader {
    pub fn new(tax_remapping: TaxRemapping, strict_mode: bool) -> GenericResult<Box<dyn BrokerStatementReader>> {
        Ok(Box::new(StatementReader {
            tax_remapping: RefCell::new(tax_remapping),
            trade_execution_dates: RefCell::new(TradeExecutionDates::new()),

            warn_on_margin_account: true,
            warn_on_missing_execution_date: strict_mode,
        }))
    }
}

impl BrokerStatementReader for StatementReader {
    fn is_statement(&self, path: &str) -> GenericResult<bool> {
        if !path.ends_with(".csv") {
            return Ok(false)
        }

        // This is a hack. We exploit here our knowledge that this method will be called for each
        // file before any statement reading. This is done because for now adding generalizations
        // for this functionality to the trait will overcomplicate it, so for now the hack is
        // preferable.
        let trade_execution_dates = &mut self.trade_execution_dates.borrow_mut();
        let is_confirmation_report = confirmation::try_parse(path, trade_execution_dates)
            .map_err(|e| format!("Error while reading {:?}: {}", path, e))?;

        Ok(!is_confirmation_report)
    }

    fn read(&mut self, path: &str, _is_last: bool) -> GenericResult<PartialBrokerStatement> {
        StatementParser {
            statement: PartialBrokerStatement::new(false),

            base_currency: None,
            base_currency_summary: None,

            tax_remapping: &mut self.tax_remapping.borrow_mut(),
            trade_execution_dates: &self.trade_execution_dates.borrow(),

            warn_on_margin_account: &mut self.warn_on_margin_account,
            warn_on_missing_execution_date: &mut self.warn_on_missing_execution_date,
        }.parse(path)
    }

    fn close(self: Box<StatementReader>) -> EmptyResult {
        self.tax_remapping.borrow().ensure_all_mapped()
    }
}

enum State {
    None,
    Record(StringRecord),
    Header(StringRecord),
}

pub struct StatementParser<'a> {
    statement: PartialBrokerStatement,

    base_currency: Option<String>,
    base_currency_summary: Option<Cash>,

    tax_remapping: &'a mut TaxRemapping,
    trade_execution_dates: &'a TradeExecutionDates,

    warn_on_margin_account: &'a mut bool,
    warn_on_missing_execution_date: &'a mut bool,
}

impl<'a> StatementParser<'a> {
    fn parse(mut self, path: &str) -> GenericResult<PartialBrokerStatement> {
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(true)
            .from_path(path)?;

        let mut records = reader.records();
        let mut state = Some(State::None);

        'state: loop {
            match state.take().unwrap() {
                State::None => {
                    match records.next() {
                        Some(result) => state = Some(State::Record(result?)),
                        None => break,
                    };
                }
                State::Record(record) => {
                    if record.len() < 2 {
                        return Err!("Invalid record: {}", format_record(&record));
                    }

                    if record.get(1).unwrap() == "Header" {
                        state = Some(State::Header(record));
                    } else if record.get(1).unwrap() == "" {
                        trace!("Headerless record: {}.", format_record(&record));
                        state = Some(State::None);
                    } else {
                        return Err!("Invalid record: {}", format_record(&record));
                    }
                },
                State::Header(record) => {
                    let spec = parse_header(&record);
                    let parser: Box<dyn RecordParser> = match spec.name {
                        "Statement" => Box::new(summary::StatementInfoParser {}),
                        "Account Information" => Box::new(summary::AccountInformationParser {}),
                        "Change in NAV" => Box::new(summary::ChangeInNavParser {}),
                        "Cash Report" => Box::new(cash::CashReportParser {}),
                        "Open Positions" => Box::new(instruments::OpenPositionsParser {}),
                        "Corporate Actions" => Box::new(corporate_actions::CorporateActionsParser {}),
                        "Trades" => Box::new(trades::TradesParser {}),
                        "Deposits & Withdrawals" => Box::new(cash::DepositsAndWithdrawalsParser {}),
                        "Fees" => Box::new(fees::FeesParser {}),
                        "Dividends" => Box::new(dividends::DividendsParser {}),
                        "Withholding Tax" => Box::new(taxes::WithholdingTaxParser {}),
                        "Interest" => Box::new(interest::InterestParser {}),
                        "Financial Instrument Information" => Box::new(instruments::FinancialInstrumentInformationParser {}),
                        _ => Box::new(common::UnknownRecordParser {}),
                    };

                    let data_types = parser.data_types();
                    let skip_data_types = parser.skip_data_types();
                    let skip_totals = parser.skip_totals();

                    for result in &mut records {
                        let record = result?;
                        if record.len() < 3 {
                            return Err!("Invalid record: {}", format_record(&record));
                        }

                        if record.get(0).unwrap() != spec.name {
                            state = Some(State::Record(record));
                            continue 'state;
                        }
                        let data_type = record.get(1).unwrap();

                        if data_type == "Header" {
                            state = Some(State::Header(record));
                            continue 'state;
                        } else if data_type == "Notes" {
                            continue
                        }

                        if let Some(skip_data_types) = skip_data_types {
                            if skip_data_types.contains(&data_type) {
                                continue;
                            }
                        }

                        if let Some(data_types) = data_types {
                            if !data_types.contains(&data_type) {
                                return Err!("Invalid data record type: {}", format_record(&record));
                            }
                        }

                        // Matches totals records. For example:
                        // * Deposits & Withdrawals,Data,Total,,,1000
                        // * Deposits & Withdrawals,Data,Total in USD,,,1000
                        // * Deposits & Withdrawals,Data,Total Deposits & Withdrawals in USD,,,1000
                        // * Interest,Data,Total,,,100
                        // * Interest,Data,Total in USD,,,100
                        // * Interest,Data,Total Interest in USD,,,100
                        if skip_totals && record.get(2).unwrap().starts_with("Total") {
                            continue;
                        }

                        parser.parse(&mut self, &Record::new(&spec, &record)).map_err(|e| format!(
                            "Failed to parse ({}) record: {}", format_record(&record), e
                        ))?;
                    }

                    break;
                }
            }
        }

        // When statement has no non-base currency activity it contains only base currency summary
        // and we have to use it as the only source of current cash assets info.
        if self.statement.cash_assets.is_none() {
            let amount = self.base_currency_summary.ok_or("Unable to find base currency summary")?;
            self.statement.cash_assets.get_or_insert_with(Default::default).deposit(amount);
        }

        self.statement.validate()
    }

    fn base_currency(&self) -> GenericResult<&str> {
        Ok(self.base_currency.as_deref().ok_or("Unable to determine account base currency")?)
    }

    fn get_execution_date(&mut self, symbol: &str, conclusion_date: Date) -> Date {
        if let Some(&execution_date) = self.trade_execution_dates.get(&OrderId {
            symbol: symbol.to_owned(),
            date: conclusion_date,
        }) {
            return execution_date;
        }

        if *self.warn_on_missing_execution_date {
            warn!(concat!(
                "The broker statement misses trade settle date information. ",
                "First occurred trade - {} at {}. ",
                "All calculations for such trades will be performed in T+0 mode."
            ), symbol, format_date(conclusion_date));
            *self.warn_on_missing_execution_date = false;
        }

        conclusion_date
    }
}

fn parse_header(record: &StringRecord) -> RecordSpec {
    let offset = 2;
    let name = record.get(0).unwrap();
    let fields = record.iter().skip(offset).collect::<Vec<_>>();
    trace!("Header: {}: {}.", name, format_record(fields.iter().cloned()));
    RecordSpec::new(name, fields, offset)
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use super::*;

    #[test]
    fn parse_real_empty() {
        let statement = parse_full("empty", None);

        assert!(statement.cash_flows.is_empty());
        assert!(!statement.cash_assets.is_empty());

        assert!(statement.fees.is_empty());
        assert!(statement.idle_cash_interest.is_empty());
        assert!(statement.tax_agent_withholdings.is_empty());

        assert!(statement.forex_trades.is_empty());
        assert!(statement.stock_buys.is_empty());
        assert!(statement.stock_sells.is_empty());
        assert!(statement.dividends.is_empty());

        assert!(statement.open_positions.is_empty());
        assert!(statement.instrument_names.is_empty());
    }

    #[test]
    fn parse_real() {
        let tax_remapping = Config::load("testdata/configs/main/config.yaml").unwrap()
            .get_portfolio("ib").unwrap().get_tax_remapping().unwrap();
        let statement = parse_full("my", Some(tax_remapping));
        let current_year = statement.period.1.year();

        assert!(!statement.cash_flows.is_empty());
        assert!(!statement.cash_assets.is_empty());

        assert!(!statement.fees.is_empty());
        assert!(!statement.idle_cash_interest.is_empty());
        assert!(statement.tax_agent_withholdings.is_empty());

        assert!(!statement.forex_trades.is_empty());
        assert!(!statement.stock_buys.is_empty());
        assert!(!statement.stock_sells.is_empty());

        let mut has_buys = false;
        for trade in &statement.stock_buys {
            if trade.conclusion_date.year() < current_year {
                has_buys = true;
                assert_ne!(trade.execution_date, trade.conclusion_date);
            }
        }
        assert!(has_buys);

        let mut has_sells = false;
        for trade in &statement.stock_sells {
            if trade.conclusion_date.year() < current_year {
                has_sells = true;
                assert_ne!(trade.execution_date, trade.conclusion_date);
            }
        }
        assert!(has_sells);

        assert!(!statement.dividends.is_empty());
        assert!(statement.dividends.iter().any(|dividend| dividend.paid_tax.is_positive()));

        assert!(!statement.open_positions.is_empty());
        assert!(!statement.instrument_names.is_empty());
    }

    #[rstest(name => [
        "return-of-capital-with-tax",
        "return-of-capital-without-tax",

        "margin-rub",
        "complex",

        "simple-with-lse",
    ])]
    fn parse_real_other(name: &str) {
        parse_full(name, None);
    }

    #[rstest(name => ["no-activity", "multi-currency-activity"])]
    fn parse_real_partial(name: &str) {
        let path = format!("testdata/interactive-brokers/partial/{}.csv", name);
        StatementReader::new(TaxRemapping::new(), true).unwrap().read(&path, true).unwrap();
    }

    fn parse_full(name: &str, tax_remapping: Option<TaxRemapping>) -> BrokerStatement {
        let broker = Broker::InteractiveBrokers.get_info(&Config::mock(), None).unwrap();
        let path = format!("testdata/interactive-brokers/{}", name);
        let tax_remapping = tax_remapping.unwrap_or_else(TaxRemapping::new);
        BrokerStatement::read(broker, &path, &hashmap!{}, &hashmap!{}, tax_remapping, true).unwrap()
    }
}