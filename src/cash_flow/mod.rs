mod calculator;
mod cash_flow;
mod comparator;

use std::collections::HashMap;

use chrono::{Datelike, Duration};

use crate::broker_statement::BrokerStatement;
use crate::config::Config;
use crate::core::EmptyResult;
use crate::currency::Cash;
use crate::formatting::table::{Table, Column, Cell};
use crate::util;

// FIXME(konishchev): It's only a prototype
pub fn generate_cash_flow_report(config: &Config, portfolio_name: &str, year: Option<i32>) -> EmptyResult {
    let portfolio = config.get_portfolio(portfolio_name)?;
    let statement = BrokerStatement::read(
        config, portfolio.broker, &portfolio.statements, portfolio.get_tax_remapping()?, false)?;

    let mut summary_title = format!("Движение средств по счету в {}", statement.broker.name);
    let mut details_title = format!("Детализация движения средств по счету в {}", statement.broker.name);

    let (start_date, end_date) = match year {
        Some(year) => {
            if year > util::today().year() {
                return Err!("An attempt to generate cash flow report for the future");
            }

            let title_suffix = format!(" за {} год", year);
            summary_title += &title_suffix;
            details_title += &title_suffix;
            statement.check_period_against_tax_year(year)?;

            (date!(1, 1, year), date!(1, 1, year + 1))
        },
        None => statement.period,
    };

    let (summaries, cash_flows) = calculator::calculate(&statement, start_date, end_date);

    let mut summary_columns = vec![Column::new("", None)];
    for &currency in summaries.keys() {
        summary_columns.push(Column::new(currency, None));
    }

    let mut summary_table = Table::new(summary_columns);

    let mut starting_assets_row = vec![start_date.into()];
    for (&currency, summary) in &summaries {
        starting_assets_row.push(Cash::new(currency, summary.start).into());
    }
    summary_table.add_row(starting_assets_row);

    let mut deposits_row = vec!["Зачисления".to_owned().into()];
    for (&currency, summary) in &summaries {
        deposits_row.push(Cash::new(currency, summary.deposits).into());
    }
    summary_table.add_row(deposits_row);

    let mut withdrawals_row = vec!["Списания".to_owned().into()];
    for (&currency, summary) in &summaries {
        withdrawals_row.push(Cash::new(currency, summary.deposits).into());
    }
    summary_table.add_row(withdrawals_row);

    let mut ending_assets_row = vec![(end_date - Duration::days(1)).into()];
    for (&currency, summary) in &summaries {
        ending_assets_row.push(Cash::new(currency, summary.end).into());
    }
    summary_table.add_row(ending_assets_row);

    summary_table.print(&summary_title);

    let mut details_columns = vec![
        Column::new("Дата", None),
        Column::new("Операция", None),
    ];
    let currencies = summaries.iter().enumerate().map(|(index, (&currency, _))| {
        (currency, index + details_columns.len())
    }).collect::<HashMap<&'static str, usize>>();
    for &currency in currencies.keys() {
        details_columns.push(Column::new(currency, None));
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
    if false {
        details_table.print(&details_title);
    }

    Ok(())
}