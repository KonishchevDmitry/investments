mod cash;
mod cash_flows;
mod common;
mod confirmation;
mod corporate_actions;
mod dividends;
mod fees;
mod grants;
mod interest;
mod instruments;
mod sections;
mod summary;
mod taxes;
mod trades;

use std::iter::Iterator;

#[cfg(test)] use chrono::Datelike;
use csv::{self, StringRecord};
use log::{trace, warn};

#[cfg(test)] use crate::brokers::Broker;
#[cfg(test)] use crate::config::Config;
use crate::core::{GenericResult, EmptyResult};
use crate::currency::Cash;
use crate::exchanges::Exchange;
use crate::formatting::format_date;
use crate::taxes::TaxRemapping;
use crate::time::{Date, DateTime};

#[cfg(test)] use super::BrokerStatement;
use super::{BrokerStatementReader, ReadingStrictness, PartialBrokerStatement};

use self::cash_flows::CashFlows;
use self::common::{Record, format_record, format_error_record, is_header_field};
use self::confirmation::{TradeExecutionInfo, OrderId};

pub struct StatementReader {
    tax_remapping: TaxRemapping,
    trade_execution_info: TradeExecutionInfo,

    warn_on_margin_account: bool,
    warn_on_missing_execution_date: bool,
    warn_on_missing_cash_flow_info: bool,
}

impl StatementReader {
    pub fn new(tax_remapping: TaxRemapping, strictness: ReadingStrictness) -> GenericResult<Box<dyn BrokerStatementReader>> {
        Ok(Box::new(StatementReader {
            tax_remapping: tax_remapping,
            trade_execution_info: TradeExecutionInfo::new(),

            warn_on_margin_account: true,
            warn_on_missing_execution_date: strictness.contains(ReadingStrictness::TRADE_SETTLE_DATE),
            warn_on_missing_cash_flow_info: strictness.contains(ReadingStrictness::CASH_FLOW_DATES),
        }))
    }
}

impl BrokerStatementReader for StatementReader {
    fn check(&mut self, path: &str) -> GenericResult<bool> {
        if !path.ends_with(".csv") {
            return Ok(false)
        }

        let is_confirmation_report = confirmation::try_parse(path, &mut self.trade_execution_info)
            .map_err(|e| format!("Error while reading {:?}: {}", path, e))?;

        Ok(!is_confirmation_report)
    }

    fn read(&mut self, path: &str, _is_last: bool) -> GenericResult<PartialBrokerStatement> {
        StatementParser {
            statement: PartialBrokerStatement::new(&[Exchange::Us, Exchange::Other], false),

            base_currency: None,
            base_currency_summary: None,
            cash_flows: CashFlows::new(self.warn_on_missing_cash_flow_info),

            tax_remapping: &mut self.tax_remapping,
            trade_execution_info: &self.trade_execution_info,

            warn_on_margin_account: &mut self.warn_on_margin_account,
            warn_on_missing_execution_date: &mut self.warn_on_missing_execution_date,
            warn_on_missing_cash_flow_info: &mut self.warn_on_missing_cash_flow_info,
        }.parse(path)
    }

    fn close(self: Box<StatementReader>) -> EmptyResult {
        self.tax_remapping.ensure_all_mapped()
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
    cash_flows: CashFlows,

    tax_remapping: &'a mut TaxRemapping,
    trade_execution_info: &'a TradeExecutionInfo,

    warn_on_margin_account: &'a mut bool,
    warn_on_missing_execution_date: &'a mut bool,
    warn_on_missing_cash_flow_info: &'a mut bool,
}

impl StatementParser<'_> {
    fn parse(mut self, path: &str) -> GenericResult<PartialBrokerStatement> {
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(true)
            .from_path(path)?;

        let mut state = Some(State::None);
        let mut records = reader.records();
        let mut section_parsers = sections::SectionParsers::new();

        'state: loop {
            match state.take().unwrap() {
                State::None => {
                    match records.next() {
                        Some(result) => state = Some(State::Record(result?)),
                        None => break 'state,
                    };
                }

                State::Record(record) => {
                    if record.len() < 2 {
                        let value = record.get(0).unwrap_or("");

                        // An empty "Option Exercises, Assignments and Expirations" section in
                        // Custom Activity Statement is rendered as a single value record without
                        // header record:
                        // "No exercises, assignments or expirations for May 21, 2018 - December 31, 2018"
                        if value.starts_with("No exercises, assignments or expirations for ") {
                            state = Some(State::None);
                            continue 'state;
                        }

                        return Err!("Invalid record: {}", format_error_record(&record));
                    }

                    if is_header_field(record.get(1).unwrap()) {
                        state = Some(State::Header(record));
                    } else if record.get(1).unwrap() == "" {
                        trace!("Headerless record: {}.", format_record(&record));
                        state = Some(State::None);
                    } else {
                        return Err!("Invalid record: {}", format_error_record(&record));
                    }
                },

                State::Header(record) => {
                    let (spec, parser) = section_parsers.select(&record)?;

                    let data_types = parser.data_types();
                    let skip_data_types = parser.skip_data_types();
                    let skip_totals = parser.skip_totals();

                    for result in &mut records {
                        let record = result?;
                        if record.get(0) != Some(spec.name) {
                            state = Some(State::Record(record));
                            continue 'state;
                        } else if record.len() < 3 {
                            return Err!("Invalid record: {}", format_error_record(&record));
                        }

                        let data_type = record.get(1).unwrap();
                        if is_header_field(data_type) {
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
                                return Err!("Invalid data record type: {}", format_error_record(&record));
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
                            "Failed to parse {} record: {}", format_error_record(&record), e
                        ))?;
                    }

                    break 'state;
                }
            }
        }

        section_parsers.commit(&mut self)?;
        *self.warn_on_missing_cash_flow_info &= self.cash_flows.commit()?;
        self.statement.validate()
    }

    fn base_currency(&self) -> GenericResult<&str> {
        Ok(self.base_currency.as_deref().ok_or("Unable to determine account base currency")?)
    }

    fn get_execution_date(&mut self, symbol: &str, conclusion_time: DateTime) -> Date {
        if let Some(info) = self.trade_execution_info.get(&OrderId {
            time: conclusion_time,
            symbol: symbol.to_owned(),
        }) {
            return info.execution_date;
        }

        if *self.warn_on_missing_execution_date {
            // https://github.com/KonishchevDmitry/investments/blob/master/docs/brokers.md#ib-trade-settle-date
            let url = "https://bit.ly/investments-ib-trade-settle-date";
            warn!(concat!(
                "The broker statement misses trade settle date information (see {}). ",
                "First occurred trade - {} at {}. ",
                "All calculations for such trades will be performed in T+0 mode.",
            ), url, symbol, format_date(conclusion_time.date()));
            *self.warn_on_missing_execution_date = false;
        }

        conclusion_time.date()
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use super::*;

    #[test]
    fn parse_real_empty() {
        let statement = parse_full("empty", None);

        assert!(!statement.assets.cash.is_empty());
        assert!(statement.assets.other.is_some());
        assert!(statement.deposits_and_withdrawals.is_empty());

        assert!(statement.fees.is_empty());
        assert!(statement.cash_grants.is_empty());
        assert!(statement.idle_cash_interest.is_empty());
        assert!(statement.tax_agent_withholdings.is_empty());

        assert!(statement.forex_trades.is_empty());
        assert!(statement.stock_buys.is_empty());
        assert!(statement.stock_sells.is_empty());
        assert!(statement.dividends.is_empty());

        assert!(statement.open_positions.is_empty());
        assert!(statement.instrument_info.is_empty());
    }

    #[test]
    fn parse_real() {
        let tax_remapping = Config::new("testdata/configs/main", None).unwrap()
            .get_portfolio("ib").unwrap().get_tax_remapping().unwrap();
        let statement = parse_full("my", Some(tax_remapping));
        let current_year = statement.period.next_date().year();

        assert!(!statement.assets.cash.is_empty());
        assert!(statement.assets.other.is_some());
        assert!(!statement.deposits_and_withdrawals.is_empty());

        assert!(!statement.fees.is_empty());
        assert!(statement.cash_grants.is_empty());
        assert!(!statement.idle_cash_interest.is_empty());
        assert!(statement.tax_agent_withholdings.is_empty());

        assert!(!statement.forex_trades.is_empty());
        assert!(!statement.stock_buys.is_empty());
        assert!(!statement.stock_sells.is_empty());

        for trade in &statement.stock_buys {
            if trade.conclusion_time.date.year() < current_year && !trade.out_of_order_execution {
                assert_ne!(trade.execution_date, trade.conclusion_time.date, "{}", trade.symbol);
            }
        }

        for trade in &statement.stock_sells {
            if trade.conclusion_time.date.year() < current_year && !trade.out_of_order_execution {
                assert_ne!(trade.execution_date, trade.conclusion_time.date, "{}", trade.symbol);
            }
        }

        assert!(!statement.dividends.is_empty());
        assert!(statement.dividends.iter().any(|dividend| dividend.paid_tax.is_positive()));

        assert!(!statement.open_positions.is_empty());
        assert!(!statement.instrument_info.is_empty());
    }

    #[rstest(name => [
        "return-of-capital-with-tax",
        "return-of-capital-without-tax",

        "liquidation",
        "margin-rub",
        "complex",

        "reverse-stock-split",
        "reverse-stock-split-reverse-order",

        "simple-with-lse",
        "symbol-with-space",
    ])]
    fn parse_real_other(name: &str) {
        parse_full(name, None);
    }

    #[rstest(name => ["no-activity", "multi-currency-activity"])]
    fn parse_real_partial(name: &str) {
        let path = format!("testdata/interactive-brokers/partial/{}.csv", name);
        StatementReader::new(TaxRemapping::new(), ReadingStrictness::all()).unwrap()
            .read(&path, true).unwrap();
    }

    fn parse_full(name: &str, tax_remapping: Option<TaxRemapping>) -> BrokerStatement {
        let broker = Broker::InteractiveBrokers.get_info(&Config::mock(), None).unwrap();
        let path = format!("testdata/interactive-brokers/{}", name);
        let tax_remapping = tax_remapping.unwrap_or_else(TaxRemapping::new);
        BrokerStatement::read(
            broker, &path, &Default::default(), &Default::default(), &Default::default(), tax_remapping, &[], &[],
            ReadingStrictness::all()).unwrap()
    }
}