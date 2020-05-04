mod calculator;
mod comparator;
mod mapper;

use std::collections::{HashMap, BTreeMap};

use chrono::Duration;

use crate::broker_statement::BrokerStatement;
use crate::brokers::Broker;
use crate::config::Config;
use crate::core::EmptyResult;
use crate::currency::Cash;
use crate::formatting::table::{Table, Column, Cell};
use crate::types::Date;

use self::calculator::CashFlowSummary;

pub fn generate_cash_flow_report(config: &Config, portfolio_name: &str, year: Option<i32>) -> EmptyResult {
    let portfolio = config.get_portfolio(portfolio_name)?;
    if !matches!(portfolio.broker, Broker::InteractiveBrokers) {
        return Err!(
            "Cash flow report is not supported for {}", portfolio.broker.get_info(config)?.name);
    }

    let statement = BrokerStatement::read(
        config, portfolio.broker, &portfolio.statements, portfolio.get_tax_remapping()?, false)?;

    let mut summary_title = format!("Движение средств по счету в {}", statement.broker.name);
    let mut details_title = format!("Детализация движения средств по счету в {}", statement.broker.name);

    let (start_date, end_date) = match year {
        Some(year) => {
            statement.check_period_against_tax_year(year)?;

            let title_suffix = format!(" за {} год", year);
            summary_title += &title_suffix;
            details_title += &title_suffix;

            (
                std::cmp::max(date!(1, 1, year), statement.period.0),
                std::cmp::min(date!(1, 1, year + 1), statement.period.1),
            )
        },
        None => statement.period,
    };

    // FIXME(konishchev): Rewrite all below
    let (summaries, cash_flows) = calculator::calculate(&statement, start_date, end_date);
    generate_summary_report(&summary_title, start_date, end_date, &summaries);

    let mut details_columns = vec![
        Column::new("Дата"),
        Column::new("Операция"),
    ];
    let currencies = summaries.iter().enumerate().map(|(index, (&currency, _))| {
        (currency, index + details_columns.len())
    }).collect::<HashMap<&'static str, usize>>();
    for &currency in currencies.keys() {
        details_columns.push(Column::new(currency));
    }

    let mut details_table = Table::new(details_columns);
    for cash_flow in cash_flows {
        let mut row = Vec::with_capacity(2 + currencies.len());
        row.push(cash_flow.date.into());
        row.push(cash_flow.description.into());

        for &currency in currencies.keys() {
            if cash_flow.amount.currency == currency {
                row.push(cash_flow.amount.into());
            } else {
                row.push(Cell::new_empty());
            }
        }

        details_table.add_row(row);
    }
    if true {
        details_table.print(&details_title);
    }

    Ok(())
}

fn generate_summary_report(
    title: &str, start_date: Date, end_date: Date,
    summaries: &BTreeMap<&'static str, CashFlowSummary>,
) {
    let mut columns = vec![Column::new("")];
    let mut starting_assets = vec![start_date.into()];
    let mut deposits = vec!["Зачисления".into()];
    let mut withdrawals = vec!["Списания".into()];
    let mut ending_assets = vec![(end_date - Duration::days(1)).into()];

    for (&currency, summary) in summaries {
        columns.push(Column::new(currency));

        let add_cell = |row: &mut Vec<Cell>, amount| row.push(Cash::new(currency, amount).into());
        add_cell(&mut starting_assets, summary.starting);
        add_cell(&mut deposits, summary.deposits);
        add_cell(&mut withdrawals, -summary.withdrawals);
        add_cell(&mut ending_assets, summary.ending);
    }

    let mut table = Table::new(columns);
    table.add_row(starting_assets);
    table.add_row(deposits);
    table.add_row(withdrawals);
    table.add_row(ending_assets);
    table.print(&title);
}