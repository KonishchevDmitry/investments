use chrono::Datelike;
use prettytable::{Table, Row, Cell};
use prettytable::format::Alignment;

use crate::broker_statement::BrokerStatement;
use crate::core::EmptyResult;
use crate::currency::converter::CurrencyConverter;
use crate::formatting;
use crate::localities::Country;

use super::statement::TaxStatement;

pub fn process_income(
    broker_statement: &BrokerStatement, year: i32, mut tax_statement: Option<&mut TaxStatement>,
    country: &Country, converter: &CurrencyConverter,
) -> EmptyResult {
    let mut table = Table::new();

    let mut trades = Vec::new();
    let mut same_dates = true;
    let mut same_currency = true; // FIXME: Check on Open Broker

    for trade in &broker_statement.stock_sells {
        if trade.execution_date.year() == year {
            same_dates &= trade.execution_date == trade.conclusion_date;
            same_currency &=
                trade.price.currency == country.currency &&
                trade.commission.currency == country.currency;
            trades.push(trade);
        }
    }

    if trades.is_empty() {
        return Ok(());
    }

    for (trade_id, trade) in trades.iter().enumerate() {
        let security = broker_statement.get_instrument_name(&trade.symbol)?;
        let details = trade.calculate(country, converter)?;

        let mut row = vec![
            Cell::new_align(&trade_id.to_string(), Alignment::RIGHT),
            formatting::date_cell(trade.conclusion_date),
        ];

        if !same_dates {
            row.push(formatting::date_cell(trade.execution_date));
        }

        row.extend_from_slice(&[
            Cell::new(&security),
            Cell::new_align(&trade.quantity.to_string(), Alignment::RIGHT),
            formatting::cash_cell(trade.price),
        ]);

        if !same_currency {
            let conclusion_precise_currency_rate = converter.precise_currency_rate(
                trade.conclusion_date, trade.commission.currency, country.currency)?;

            let execution_precise_currency_rate = converter.precise_currency_rate(
                trade.execution_date, trade.price.currency, country.currency)?;

            row.push(formatting::decimal_cell(conclusion_precise_currency_rate));
            if !same_dates {
                row.push(formatting::decimal_cell(execution_precise_currency_rate));
            }
        }

        row.push(formatting::cash_cell(details.revenue));
        if !same_currency {
            row.push(formatting::cash_cell(details.local_revenue));
        }

        row.push(formatting::cash_cell(trade.commission));
        if !same_currency {
            row.push(formatting::cash_cell(details.local_commission));
        }

        row.extend_from_slice(&[
            formatting::cash_cell(details.purchase_local_cost),
            formatting::cash_cell(details.total_local_cost),
            formatting::cash_cell(details.local_profit),
            formatting::cash_cell(details.tax_to_pay),
        ]);

        row.push(formatting::ratio_cell(details.real_profit_ratio));
        if !same_currency {
            row.push(formatting::ratio_cell(details.real_local_profit_ratio));
        }

        // FIXME: Print FIFO details
        table.add_row(Row::new(row));

        if let Some(ref mut tax_statement) = tax_statement {
            // FIXME: Stock selling support
            let _ = tax_statement;
        }
    }

    let mut columns = vec!["№", "Дата сделки"];
    if !same_dates {
        columns.push("Дата расчета");
    }
    columns.extend_from_slice(&["Ценная бумага", "Кол.", "Цена"]);

    if !same_currency {
        if same_dates {
            columns.push("Курс руб.");
        } else {
            columns.extend_from_slice(&[
                "Курс руб.\nна дату сделки",
                "Курс руб.\nна дату расчета",
            ]);
        }
    }

    columns.push("Доход от\nпродажи");
    if !same_currency {
        columns.push("Доход от\nпродажи (руб)");
    }

    columns.push("Комиссия");
    if !same_currency {
        columns.push("Комиссия\n(руб)");
    }

    columns.extend_from_slice(&["Затраты на\nпокупку", "Общие\nзатраты", "Прибыль", "Налог"]);

    columns.push("Реальный\nдоход");
    if !same_currency {
        columns.push("Реальный\nдоход (руб)");
    }

    formatting::print_statement(
        &format!("Расчет прибыли от продажи ценных бумаг, полученной через {}",
                 broker_statement.broker.name),
        &columns, table,
    );

    Ok(())
}