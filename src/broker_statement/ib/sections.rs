use std::collections::{HashMap, hash_map::Entry};

use csv::StringRecord;
use log::trace;

use crate::core::{GenericResult, EmptyResult};

use super::StatementParser;
use super::cash::{CashReportParser, DepositsAndWithdrawalsParser, StatementOfFundsParser};
use super::common::{RecordSpec, RecordParser, UnknownRecordParser, format_record};
use super::corporate_actions::CorporateActionsParser;
use super::dividends::DividendsParser;
use super::fees::FeesParser;
use super::grants::GrantsParser;
use super::instruments::{OpenPositionsParser, FinancialInstrumentInformationParser};
use super::interest::InterestParser;
use super::summary::{AccountInformationParser, NavParser, ChangeInNavParser, StatementInfoParser};
use super::taxes::WithholdingTaxParser;
use super::trades::TradesParser;

pub struct SectionParsers {
    statement_info_parser: StatementInfoParser,
    account_information_parser: AccountInformationParser,
    nav_parser: NavParser,
    change_in_nav_parser: ChangeInNavParser,
    cash_report_parser: CashReportParser,
    statement_of_funds_parser: StatementOfFundsParser,
    open_positions_parser: OpenPositionsParser,
    corporate_actions_parser: CorporateActionsParser,
    trades_parser: TradesParser,
    grants_parser: GrantsParser,
    deposits_and_withdrawals_parser: DepositsAndWithdrawalsParser,
    fees_parser: FeesParser,
    dividends_parser: DividendsParser,
    withholding_tax_parser: WithholdingTaxParser,
    interest_parser: InterestParser,
    financial_instrument_information_parser: FinancialInstrumentInformationParser,

    unknown_record_parser: UnknownRecordParser,
    duplicated_record_parser: UnknownRecordParser,

    parsed_sections: HashMap<String, bool>,
}

impl SectionParsers {
    pub fn new() -> SectionParsers {
        SectionParsers {
            statement_info_parser: StatementInfoParser {},
            account_information_parser: AccountInformationParser{},
            nav_parser: NavParser{},
            change_in_nav_parser: ChangeInNavParser {},
            cash_report_parser: CashReportParser {},
            statement_of_funds_parser: StatementOfFundsParser {},
            open_positions_parser: OpenPositionsParser {},
            corporate_actions_parser: CorporateActionsParser::new(),
            trades_parser: TradesParser {},
            grants_parser: GrantsParser {},
            deposits_and_withdrawals_parser: DepositsAndWithdrawalsParser {},
            fees_parser: FeesParser {},
            dividends_parser: DividendsParser {},
            withholding_tax_parser: WithholdingTaxParser {},
            interest_parser: InterestParser {},
            financial_instrument_information_parser: FinancialInstrumentInformationParser {},

            unknown_record_parser: UnknownRecordParser {},
            duplicated_record_parser: UnknownRecordParser {},

            parsed_sections: HashMap::new(),
        }
    }

    pub fn select<'p, 'r>(&'p mut self, record: &'r StringRecord) -> GenericResult<(RecordSpec<'r>, &'p mut dyn RecordParser)> {
        let spec = parse_header(record);
        let mut parser: &mut dyn RecordParser = match spec.name {
            "Statement" => &mut self.statement_info_parser,
            "Account Information" => &mut self.account_information_parser,
            "Net Asset Value" => &mut self.nav_parser,
            "Change in NAV" => &mut self.change_in_nav_parser,
            "Cash Report" => &mut self.cash_report_parser,
            "Statement of Funds" => &mut self.statement_of_funds_parser,
            "Open Positions" => &mut self.open_positions_parser,
            "Corporate Actions" => &mut self.corporate_actions_parser,
            "Trades" => &mut self.trades_parser,
            "Grant Activity" => &mut self.grants_parser,
            "Deposits & Withdrawals" => &mut self.deposits_and_withdrawals_parser,
            "Fees" => &mut self.fees_parser,
            "Dividends" => &mut self.dividends_parser,
            "Withholding Tax" => &mut self.withholding_tax_parser,
            "Interest" => &mut self.interest_parser,
            "Financial Instrument Information" => &mut self.financial_instrument_information_parser,
            _ => &mut self.unknown_record_parser,
        };

        if !parser.allow_multiple() {
            let has_code_field = spec.has_field("Code");

            match self.parsed_sections.entry(spec.name.to_owned()) {
                Entry::Occupied(entry) => {
                    let had_code_field = *entry.get();

                    match spec.name {
                        // Rust complains on second mutable borrow if we try to assign
                        // self.unknown_record_parser here, so we created a second parser - just
                        // to workaround this issue.

                        // This section has two different headers. Skip the second variant.
                        "Net Asset Value" if spec.has_field("Time Weighted Rate of Return") => {
                            parser = &mut self.duplicated_record_parser;
                        },

                        // Custom Activity Statement contains duplicated sections because of legacy
                        // sections. Some of them have different names, but others - the same with
                        // modern ones. Here we detect legacy sections by Code field which doesn't
                        // exist in modern ones.
                        "Dividends" | "Deposits & Withdrawals" if !had_code_field && has_code_field => {
                            parser = &mut self.duplicated_record_parser;
                        },

                        _ => return Err!("Got a duplicated {} section", spec.name),
                    }
                },
                Entry::Vacant(entry) => {
                    entry.insert(has_code_field);
                },
            };
        }

        Ok((spec, parser))
    }

    pub fn commit(self, parser: &mut StatementParser) -> EmptyResult {
        // When statement has no non-base currency activity it contains only base currency summary
        // and we have to use it as the only source of current cash assets info.
        if parser.statement.assets.cash.is_none() {
            let amount = parser.base_currency_summary.ok_or("Unable to find base currency summary")?;
            parser.statement.assets.cash.get_or_insert_with(Default::default).deposit(amount);
        }

        self.corporate_actions_parser.commit(parser)
    }
}

fn parse_header(record: &StringRecord) -> RecordSpec<'_> {
    let offset = 2;
    let name = record.get(0).unwrap();
    let fields = record.iter().skip(offset).collect::<Vec<_>>();
    trace!("Header: {}: {}.", name, format_record(fields.iter().cloned()));
    RecordSpec::new(name, fields, offset)
}