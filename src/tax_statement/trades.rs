use chrono::Datelike;

use crate::broker_statement::BrokerStatement;
use crate::broker_statement::trades::{StockSell, SellDetails, FifoDetails};
use crate::config::PortfolioConfig;
use crate::core::EmptyResult;
use crate::currency::Cash;
use crate::currency::converter::CurrencyConverter;
use crate::formatting::{self, Table, Row, Cell, Alignment};
use crate::localities::Country;
use crate::taxes::TaxPaymentDay;

use super::statement::TaxStatement;

pub fn process_income(
    portfolio: &PortfolioConfig, broker_statement: &BrokerStatement, year: Option<i32>,
    mut tax_statement: Option<&mut TaxStatement>, converter: &CurrencyConverter,
) -> EmptyResult {
    let country = portfolio.get_tax_country();

    let mut trades = Vec::new();
    let mut same_dates = true;
    let mut same_currency = true;

    for trade in &broker_statement.stock_sells {
        if let Some(year) = year {
            if trade.execution_date.year() != year {
                continue;
            }
        }

        same_dates &= trade.execution_date == trade.conclusion_date;
        same_currency &=
            trade.price.currency == country.currency &&
                trade.commission.currency == country.currency;

        let details = trade.calculate(&country, converter)?;

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

    let mut processor = TradesProcessor {
        portfolio,
        broker_statement,
        year,

        country,
        converter,

        table: Table::new(),
        fifo_table: Table::new(),

        same_dates,
        same_currency,
        total_local_profit: Cash::new(country.currency, dec!(0)),
    };

    for (trade_id, (trade, details)) in trades.iter().enumerate() {
        processor.process_trade(trade_id, trade, details)?;

        if let Some(ref mut tax_statement) = tax_statement {
            processor.add_income(tax_statement, trade, details)?;
        }
    }

    processor.print();

    Ok(())
}

struct TradesProcessor<'a> {
    portfolio: &'a PortfolioConfig,
    broker_statement: &'a BrokerStatement,
    year: Option<i32>,

    country: Country,
    converter: &'a CurrencyConverter,

    table: Table,
    fifo_table: Table,

    same_dates: bool,
    same_currency: bool,
    total_local_profit: Cash,
}

impl<'a> TradesProcessor<'a> {
    fn add_income(&self, tax_statement: &mut TaxStatement, trade: &StockSell, details: &SellDetails) -> EmptyResult {
        let name = self.broker_statement.get_instrument_name(&trade.symbol)?;
        let description = format!("{}: Продажа {}", self.broker_statement.broker.name, name);

        let precise_currency_rate = self.converter.precise_currency_rate(
            trade.execution_date, details.revenue.currency, self.country.currency)?;

        tax_statement.add_stock_income(
            &description, trade.execution_date, details.revenue.currency, precise_currency_rate,
            details.revenue.amount, details.local_revenue.amount,
            details.total_local_cost.amount
        ).map_err(|e| format!(
            "Unable to add income from selling {} on {} to the tax statement: {}",
            trade.symbol, formatting::format_date(trade.execution_date), e
        ))?;

        Ok(())
    }

    fn process_trade(&mut self, trade_id: usize, trade: &StockSell, details: &SellDetails) -> EmptyResult {
        let security = self.broker_statement.get_instrument_name(&trade.symbol)?;
        self.total_local_profit.add_assign(details.local_profit).unwrap();

        let mut row = vec![
            Cell::new_align(&trade_id.to_string(), Alignment::RIGHT),
            formatting::date_cell(trade.conclusion_date),
        ];

        if !self.same_dates {
            row.push(formatting::date_cell(trade.execution_date));
        }

        row.extend_from_slice(&[
            Cell::new(&security),
            Cell::new_align(&trade.quantity.to_string(), Alignment::RIGHT),
            formatting::cash_cell(trade.price),
        ]);

        if !self.same_currency {
            let conclusion_precise_currency_rate = self.converter.precise_currency_rate(
                trade.conclusion_date, trade.commission.currency, self.country.currency)?;

            let execution_precise_currency_rate = self.converter.precise_currency_rate(
                trade.execution_date, trade.price.currency, self.country.currency)?;

            row.push(formatting::decimal_cell(conclusion_precise_currency_rate));
            if !self.same_dates {
                row.push(formatting::decimal_cell(execution_precise_currency_rate));
            }
        }

        row.push(formatting::cash_cell(details.revenue));
        if !self.same_currency {
            row.push(formatting::cash_cell(details.local_revenue));
        }

        row.push(formatting::cash_cell(trade.commission));
        if !self.same_currency {
            row.push(formatting::cash_cell(details.local_commission));
        }

        row.extend_from_slice(&[
            formatting::cash_cell(details.purchase_local_cost),
            formatting::cash_cell(details.total_local_cost),
            formatting::cash_cell(details.local_profit),
            formatting::cash_cell(details.tax_to_pay),
        ]);

        row.push(formatting::ratio_cell(details.real_profit_ratio));
        if !self.same_currency {
            row.push(formatting::ratio_cell(details.real_local_profit_ratio));
        }

        self.table.add_row(Row::new(&row));

        for (buy_trade_id, buy_trade) in details.fifo.iter().enumerate() {
            self.process_fifo(&security, trade_id, buy_trade_id, buy_trade)?;
        }

        Ok(())
    }

    fn process_fifo(&mut self, security: &str, trade_id: usize, buy_trade_id: usize, buy_trade: &FifoDetails) -> EmptyResult {
        let mut row = Vec::new();

        row.push(if buy_trade_id == 0 {
            Cell::new_align(&trade_id.to_string(), Alignment::RIGHT)
        } else {
            Cell::new("")
        });

        row.push(formatting::date_cell(buy_trade.conclusion_date));
        if !self.same_dates {
            row.push(formatting::date_cell(buy_trade.execution_date));
        }

        row.extend_from_slice(&[
            Cell::new(&security),
            Cell::new_align(&buy_trade.quantity.to_string(), Alignment::RIGHT),
            formatting::cash_cell(buy_trade.price),
        ]);

        if !self.same_currency {
            let conclusion_precise_currency_rate = self.converter.precise_currency_rate(
                buy_trade.conclusion_date, buy_trade.commission.currency, self.country.currency)?;

            let execution_precise_currency_rate = self.converter.precise_currency_rate(
                buy_trade.execution_date, buy_trade.price.currency, self.country.currency)?;

            row.push(formatting::decimal_cell(conclusion_precise_currency_rate));
            if !self.same_dates {
                row.push(formatting::decimal_cell(execution_precise_currency_rate));
            }
        }

        row.push(formatting::cash_cell(buy_trade.cost));
        if !self.same_currency {
            row.push(formatting::cash_cell(buy_trade.local_cost));
        }

        row.push(formatting::cash_cell(buy_trade.commission));
        if !self.same_currency {
            row.push(formatting::cash_cell(buy_trade.local_commission));
        }

        row.push(formatting::cash_cell(buy_trade.total_local_cost));

        self.fifo_table.add_row(Row::new(&row));

        Ok(())
    }

    fn print(mut self) {
        let tax_to_pay = Cash::new(
            self.country.currency, self.country.tax_to_pay(self.total_local_profit.amount, None));

        let mut trade_columns = vec!["№", "Дата сделки"];
        let mut fifo_columns = trade_columns.clone();

        for columns in &mut [&mut trade_columns, &mut fifo_columns] {
            if !self.same_dates {
                columns.push("Дата расчета");
            }
            columns.extend_from_slice(&["Ценная бумага", "Кол.", "Цена"]);

            if !self.same_currency {
                if self.same_dates {
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
        if !self.same_currency {
            trade_columns.push("Доход от\nпродажи (руб)");
        }

        fifo_columns.push("Расходы");
        if !self.same_currency {
            fifo_columns.push("Расходы (руб)");
        }

        for columns in &mut [&mut trade_columns, &mut fifo_columns] {
            columns.push("Комиссия");
            if !self.same_currency {
                columns.push("Комиссия\n(руб)");
            }
        }

        trade_columns.extend_from_slice(&["Затраты на\nпокупку", "Общие\nзатраты"]);

        let total_local_profit_index = trade_columns.len();
        trade_columns.push("Прибыль");

        let total_tax_index = trade_columns.len();
        trade_columns.push("Налог");

        fifo_columns.push("Общие затраты");

        trade_columns.push("Реальный\nдоход");
        if !self.same_currency {
            trade_columns.push("Реальный\nдоход (руб)");
        }

        let mut totals = Vec::new();
        for index in 0..trade_columns.len() {
            totals.push(if index == total_local_profit_index {
                formatting::cash_cell(self.total_local_profit)
            } else if index == total_tax_index {
                let show_net_tax = match self.portfolio.tax_payment_day {
                    TaxPaymentDay::Day {..} => self.year.is_some(),
                    TaxPaymentDay::OnClose => self.year.is_none(),
                };

                if show_net_tax {
                    formatting::cash_cell(tax_to_pay)
                } else {
                    Cell::new("")
                }
            } else {
                Cell::new("")
            });
        }
        self.table.add_row(Row::new(&totals));

        formatting::print_statement(
            &format!("Расчет прибыли от продажи ценных бумаг, полученной через {}",
                     self.broker_statement.broker.name),
            &trade_columns, self.table,
        );

        formatting::print_statement(
            "Детализация расчета сделок по ФИФО", &fifo_columns, self.fifo_table);
    }
}