mod calculator;
mod comparator;
mod mapper;

use std::collections::BTreeMap;

use itertools::Itertools;
use log::warn;

use crate::broker_statement::{BrokerStatement, ReadingStrictness, NetAssets};
use crate::config::Config;
use crate::core::{GenericResult, EmptyResult};
use crate::currency::{self, Cash, converter::CurrencyConverter};
use crate::db;
use crate::formatting::{self, table::{Table, Column, Cell}};
use crate::localities::Jurisdiction;
use crate::telemetry::TelemetryRecordBuilder;
use crate::time::{Date, Period};

use self::calculator::CashFlowSummary;
use self::mapper::{CashFlow, Operation};

pub fn generate_cash_flow_report(config: &Config, portfolio_name: &str, year: Option<i32>) -> GenericResult<TelemetryRecordBuilder> {
    let portfolio = config.get_portfolio(portfolio_name)?;
    let broker = portfolio.broker.get_info(config, portfolio.plan.as_ref())?;

    let database = db::connect(&config.db_path)?;
    let converter = CurrencyConverter::new(database, None, year.is_some());

    let statement = BrokerStatement::read(
        broker, portfolio.statements_path()?, &portfolio.symbol_remapping, &portfolio.instrument_internal_ids,
        &portfolio.instrument_names, portfolio.get_tax_remapping()?, &portfolio.corporate_actions,
        ReadingStrictness::CASH_FLOW_DATES)?;

    let period = match year {
        Some(year) => statement.check_period_against_tax_year(year)?,
        None => statement.period,
    };

    let (summaries, cash_flows) = calculator::calculate(&statement, period);
    generate_cash_summary_report(period, &summaries);

    if statement.broker.type_.jurisdiction() == Jurisdiction::Usa {
        generate_other_summary_report(&statement, period, &cash_flows, &converter, "USD")?;
    }

    generate_details_report(&summaries, cash_flows);

    Ok(TelemetryRecordBuilder::new_with_broker(portfolio.broker))
}

fn generate_cash_summary_report(period: Period, summaries: &BTreeMap<&'static str, CashFlowSummary>) {
    let mut columns = vec![Column::new("")];
    let mut starting_assets_row = vec![period.first_date().into()];
    let mut deposits_row = vec!["Зачисления".into()];
    let mut withdrawals_row = vec!["Списания".into()];
    let mut ending_assets_row = vec![period.last_date().into()];

    for (&currency, summary) in summaries {
        columns.push(Column::new(currency));

        let starting = currency::round(summary.starting);
        let deposits = currency::round(summary.deposits);
        let withdrawals = currency::round(summary.withdrawals);
        let ending = starting + deposits - withdrawals;
        assert!(summary.ending - dec!(0.015) <= ending && ending <= summary.ending + dec!(0.015));

        let add_cell = |row: &mut Vec<Cell>, amount| row.push(Cash::new(currency, amount).into());
        add_cell(&mut starting_assets_row, starting);
        add_cell(&mut deposits_row, deposits);
        add_cell(&mut withdrawals_row, -withdrawals);
        add_cell(&mut ending_assets_row, ending);
    }

    let mut table = Table::new(columns);
    table.add_row(starting_assets_row);
    table.add_row(deposits_row);
    table.add_row(withdrawals_row);
    table.add_row(ending_assets_row);
    table.print("Движение денежных средств");
}

fn generate_other_summary_report(
    statement: &BrokerStatement, period: Period, cash_flows: &[CashFlow],
    converter: &CurrencyConverter, jurisdiction_currency: &str,
) -> EmptyResult {
    let mut missing = false;
    let mut currency = None;

    let end_assets = if let Some(NetAssets{other: Some(assets), ..}) = statement.historical_assets.get(&period.last_date()) {
        currency.get_or_insert(assets.currency);
        Cell::from(*assets)
    } else {
        missing = true;
        Cell::new_empty()
    };

    let start_assets = if let Some(NetAssets{other: Some(assets), ..}) = statement.historical_assets.get(&period.prev_date()) {
        currency.get_or_insert(assets.currency);
        Cell::from(*assets)
    } else if period.first_date() == statement.period.first_date() {
        Cell::from(Cash::zero(currency.unwrap_or(jurisdiction_currency)))
    } else {
        missing = true;
        Cell::new_empty()
    };

    let currency = currency.unwrap_or(jurisdiction_currency);

    let mut deposits = dec!(0);
    let mut withdrawals = dec!(0);
    let mut process = |date: Date, amount: Cash| -> EmptyResult {
        let amount = converter.convert_to_rounding(date, -amount, currency)?;

        if amount.is_sign_positive() {
            deposits += amount;
        } else {
            withdrawals += amount;
        }

        Ok(())
    };

    for cash_flow in cash_flows {
        if matches!(cash_flow.operation, Operation::BuyTrade | Operation::SellTrade) {
            process(cash_flow.time.date, cash_flow.amount)?;
            if let Some(amount) = cash_flow.sibling_amount {
                process(cash_flow.time.date, amount)?;
            }
        }
    }

    let mut table = Table::new(vec![Column::new(""), Column::new("")]);
    table.add_row(vec![period.first_date().into(), start_assets]);
    table.add_row(vec!["Зачисления".into(), Cash::new(currency, deposits).into()]);
    table.add_row(vec!["Списания".into(), Cash::new(currency, withdrawals).into()]);
    table.add_row(vec![period.last_date().into(), end_assets]);
    table.hide_titles();
    table.print("Стоимость иных финансовых активов");

    if missing {
        let mut clarification = statement.historical_assets.iter().filter_map(|(&date, assets)| {
            if assets.other.is_some() {
                Some(formatting::format_date(date))
            } else {
                None
            }
        }).join(", ");

        if !clarification.is_empty() {
            clarification = format!(" Available dates: {}.", clarification)
        }

        eprintln!(); warn!(
            "The broker statements don't contain net asset value information for the specified period.{}",
            clarification);
    }

    Ok(())
}

fn generate_details_report(
    summaries: &BTreeMap<&'static str, CashFlowSummary>, cash_flows: Vec<CashFlow>
) {
    let mut columns = vec![Column::new("Дата"), Column::new("Операция")];
    for &currency in summaries.keys() {
        columns.push(Column::new(currency));
    }
    let mut table = Table::new(columns);

    for cash_flow in cash_flows {
        let mut row = Vec::with_capacity(2 + summaries.len());
        row.push(cash_flow.time.date.into());
        row.push(cash_flow.description.into());

        let mut matched = 0;

        for &currency in summaries.keys() {
            if cash_flow.amount.currency == currency {
                row.push(cash_flow.amount.into());
                matched += 1;
                continue
            }

            if let Some(amount) = cash_flow.sibling_amount {
                if amount.currency == currency {
                    row.push(amount.into());
                    matched += 1;
                    continue
                }
            }

            row.push(Cell::new_empty());
        }

        assert_eq!(if cash_flow.sibling_amount.is_some() {
            2
        } else {
            1
        }, matched);

        table.add_row(row);
    }

    table.print("Детализация движения денежных средств");
}