mod calculator;
mod comparator;
mod mapper;

use std::collections::BTreeMap;

use crate::broker_statement::{BrokerStatement, ReadingStrictness};
use crate::config::Config;
use crate::core::GenericResult;
use crate::currency::{self, Cash};
use crate::formatting::table::{Table, Column, Cell};
use crate::telemetry::TelemetryRecordBuilder;
use crate::types::Date;

use self::calculator::CashFlowSummary;
use self::mapper::CashFlow;

pub fn generate_cash_flow_report(config: &Config, portfolio_name: &str, year: Option<i32>) -> GenericResult<TelemetryRecordBuilder> {
    let portfolio = config.get_portfolio(portfolio_name)?;
    let broker = portfolio.broker.get_info(config, portfolio.plan.as_ref())?;

    let statement = BrokerStatement::read(
        broker, &portfolio.statements, &portfolio.symbol_remapping, &portfolio.instrument_names,
        portfolio.get_tax_remapping()?, &portfolio.corporate_actions, ReadingStrictness::CASH_FLOW_DATES)?;

    let mut summary_title = format!("Движение денежных средств по счету в {}", statement.broker.name);
    let mut details_title = format!("Детализация движения денежных средств по счету в {}", statement.broker.name);

    let (start_date, end_date) = match year {
        Some(year) => {
            statement.check_period_against_tax_year(year)?;

            let title_suffix = format!(" за {} год", year);
            summary_title += &title_suffix;
            details_title += &title_suffix;

            (
                std::cmp::max(date!(year, 1, 1), statement.period.0),
                std::cmp::min(date!(year + 1, 1, 1), statement.period.1),
            )
        },
        None => statement.period,
    };

    let (summaries, cash_flows) = calculator::calculate(&statement, start_date, end_date);
    generate_summary_report(&summary_title, start_date, end_date, &summaries);
    generate_details_report(&details_title, &summaries, cash_flows);

    Ok(TelemetryRecordBuilder::new_with_broker(portfolio.broker))
}

fn generate_summary_report(
    title: &str, start_date: Date, end_date: Date,
    summaries: &BTreeMap<&'static str, CashFlowSummary>,
) {
    let mut columns = vec![Column::new("")];
    let mut starting_assets_row = vec![start_date.into()];
    let mut deposits_row = vec!["Зачисления".into()];
    let mut withdrawals_row = vec!["Списания".into()];
    let mut ending_assets_row = vec![end_date.pred().into()];

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
    table.print(title);
}

fn generate_details_report(
    title: &str, summaries: &BTreeMap<&'static str, CashFlowSummary>, cash_flows: Vec<CashFlow>
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

    table.print(title);
}