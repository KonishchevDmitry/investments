mod calculator;
mod comparator;

use chrono::Datelike;

use crate::broker_statement::BrokerStatement;
use crate::config::Config;
use crate::core::EmptyResult;
use crate::formatting::table::{Table, Column};
use crate::util;

// FIXME(konishchev): It's only a prototype
pub fn generate_cash_flow_report(config: &Config, portfolio_name: &str, year: Option<i32>) -> EmptyResult {
    let portfolio = config.get_portfolio(portfolio_name)?;
    let statement = BrokerStatement::read(
        config, portfolio.broker, &portfolio.statements, portfolio.get_tax_remapping()?, false)?;

    let mut title = format!("Движение средств по счету в {}", statement.broker.name);
    let mut details_title = format!("Детализация вижения средств по счету в {}", statement.broker.name);

    if let Some(year) = year {
        if year > util::today().year() {
            return Err!("An attempt to generate cash flow report for the future");
        }

        let title_suffix = format!(" за {} год", year);
        title += &title_suffix;
        details_title += &title_suffix;
        statement.check_period_against_tax_year(year)?;
    }

    let cash_flows = calculator::calculate(&statement);


    let table = Table::new(vec![
        Column::new("Дата", None),
        Column::new("Операция", None),
    ]);
    table.print(&title);

    let mut details_table = Table::new(vec![
        Column::new("Дата", None),
        Column::new("Операция", None),
    ]);

    for cash_flow in cash_flows {
        details_table.add_row(vec![cash_flow.date.into(), cash_flow.description.into()]);
    }
    details_table.print(&details_title);

    Ok(())
}