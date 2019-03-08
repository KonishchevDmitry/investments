use chrono::Datelike;
use prettytable::{Table, Row, Cell};
use prettytable::format::Alignment;

use crate::broker_statement::BrokerStatement;
use crate::core::EmptyResult;
use crate::currency::converter::CurrencyConverter;
use crate::formatting;
use crate::localities::Country;

use super::statement::TaxStatement;

// FIXME: Validate logic by manual calculations
pub fn process_income(
    broker_statement: &BrokerStatement, year: i32, mut tax_statement: Option<&mut TaxStatement>,
    country: &Country, converter: &CurrencyConverter,
) -> EmptyResult {
    let mut table = Table::new();
    let mut fifo_table = Table::new();

    let mut trades = Vec::new();
    let mut same_dates = true;
    let mut same_currency = true; // FIXME: Check on Open Broker

    for trade in &broker_statement.stock_sells {
        if trade.execution_date.year() != year {
            continue;
        }

        same_dates &= trade.execution_date == trade.conclusion_date;
        same_currency &=
            trade.price.currency == country.currency &&
                trade.commission.currency == country.currency;

        let details = trade.calculate(country, converter)?;

        for buy_trade in &details.fifo {
            same_dates &= buy_trade.execution_date == buy_trade.conclusion_date;
            same_currency &=
                buy_trade.price.currency == country.currency &&
                    buy_trade.commission.currency == country.currency;
        }

        trades.push((trade, details));
    }

    if trades.is_empty() {
        return Ok(());
    }

    for (trade_id, (trade, details)) in trades.iter().enumerate() {
        let security = broker_statement.get_instrument_name(&trade.symbol)?;

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

        table.add_row(Row::new(row));

        for (buy_trade_id, buy_trade) in details.fifo.iter().enumerate() {
            let mut row = Vec::new();

            row.push(if buy_trade_id == 0 {
                Cell::new_align(&trade_id.to_string(), Alignment::RIGHT)
            } else {
                Cell::new("")
            });

            row.push(formatting::date_cell(buy_trade.conclusion_date));
            if !same_dates {
                row.push(formatting::date_cell(buy_trade.execution_date));
            }

            row.extend_from_slice(&[
                Cell::new(&security),
                Cell::new_align(&buy_trade.quantity.to_string(), Alignment::RIGHT),
                formatting::cash_cell(buy_trade.price),
            ]);

            if !same_currency {
                let conclusion_precise_currency_rate = converter.precise_currency_rate(
                    buy_trade.conclusion_date, buy_trade.commission.currency, country.currency)?;

                let execution_precise_currency_rate = converter.precise_currency_rate(
                    buy_trade.execution_date, buy_trade.price.currency, country.currency)?;

                row.push(formatting::decimal_cell(conclusion_precise_currency_rate));
                if !same_dates {
                    row.push(formatting::decimal_cell(execution_precise_currency_rate));
                }
            }

            row.push(formatting::cash_cell(buy_trade.cost));
            if !same_currency {
                row.push(formatting::cash_cell(buy_trade.local_cost));
            }

            row.push(formatting::cash_cell(buy_trade.commission));
            if !same_currency {
                row.push(formatting::cash_cell(buy_trade.local_commission));
            }

            row.push(formatting::cash_cell(buy_trade.total_local_cost));

            fifo_table.add_row(Row::new(row));
        }

        if let Some(ref mut tax_statement) = tax_statement {
            // FIXME: Stock selling support
            let _ = tax_statement;
        }
    }

    let mut trade_columns = vec!["№", "Дата сделки"];
    let mut fifo_columns = trade_columns.clone();

    for columns in &mut [&mut trade_columns, &mut fifo_columns] {
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
    }

    trade_columns.push("Доход от\nпродажи");
    if !same_currency {
        trade_columns.push("Доход от\nпродажи (руб)");
    }

    fifo_columns.push("Расходы");
    if !same_currency {
        fifo_columns.push("Расходы (руб)");
    }

    for columns in &mut [&mut trade_columns, &mut fifo_columns] {
        columns.push("Комиссия");
        if !same_currency {
            columns.push("Комиссия\n(руб)");
        }
    }

    trade_columns.extend_from_slice(&["Затраты на\nпокупку", "Общие\nзатраты", "Прибыль", "Налог"]);
    fifo_columns.push("Общие затраты");

    trade_columns.push("Реальный\nдоход");
    if !same_currency {
        trade_columns.push("Реальный\nдоход (руб)");
    }

    formatting::print_statement(
        &format!("Расчет прибыли от продажи ценных бумаг, полученной через {}",
                 broker_statement.broker.name),
        &trade_columns, table,
    );

    formatting::print_statement(
        &format!("Детализация расчета сделок по ФИФО"),
        &fifo_columns, fifo_table,
    );

    Ok(())
}