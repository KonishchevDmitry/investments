use chrono::Datelike;
use prettytable::{Table, Row, Cell};
use prettytable::format::Alignment;

use crate::broker_statement::BrokerStatement;
use crate::core::EmptyResult;
use crate::currency::{self, Cash, MultiCurrencyCashAccount};
use crate::currency::converter::CurrencyConverter;
use crate::formatting;
use crate::localities::Country;
use crate::types::Decimal;

use super::statement::TaxStatement;

pub fn process_income(
    broker_statement: &BrokerStatement, year: i32, mut tax_statement: Option<&mut TaxStatement>,
    country: &Country, converter: &CurrencyConverter,
) -> EmptyResult {
    let mut table = Table::new();

    let mut total_foreign_amount = MultiCurrencyCashAccount::new();
    let mut total_amount = dec!(0);

    let mut total_foreign_paid_tax = MultiCurrencyCashAccount::new();
    let mut total_paid_tax = dec!(0);
    let mut total_tax_to_pay = dec!(0);

    let mut total_income = dec!(0);

    for (trade_id, trade) in broker_statement.stock_sells.iter().enumerate() {
        if trade.execution_date.year() != year {
            continue;
        }

        let security = broker_statement.get_instrument_name(&trade.symbol)?;
        let details = trade.calculate(country, converter)?;

        let conclusion_precise_currency_rate = converter.precise_currency_rate(
            trade.conclusion_date, trade.commission.currency, country.currency)?;

        let commission = currency::round(converter.convert_to(
            trade.conclusion_date, trade.commission, country.currency)?);

        let execution_precise_currency_rate = converter.precise_currency_rate(
            trade.execution_date, trade.price.currency, country.currency)?;

        let mut purchase_local_cost = Cash::new(country.currency, dec!(0));
        for buy_trade in details.fifo {
            purchase_local_cost.add_assign(buy_trade.purchase_local_cost).unwrap();
        }

        /*

        let foreign_amount = dividend.amount.round();
        total_foreign_amount.deposit(foreign_amount);


        let amount = currency::round(converter.convert_to(
            dividend.date, dividend.amount, country.currency)?);
        total_amount += amount;

        let foreign_paid_tax = dividend.paid_tax;
        total_foreign_paid_tax.deposit(foreign_paid_tax);

        let paid_tax = currency::round(converter.convert_to(
            dividend.date, dividend.paid_tax, country.currency)?);
        total_paid_tax += paid_tax;

        let tax_to_pay = dividend.tax_to_pay(&country, converter)?;
        total_tax_to_pay += tax_to_pay;

        let income = amount - paid_tax - tax_to_pay;
        total_income += income;
        */

        table.add_row(Row::new(vec![
            Cell::new_align(&trade_id.to_string(), Alignment::RIGHT),
            Cell::new(&security),

            formatting::date_cell(trade.conclusion_date),
            formatting::decimal_cell(conclusion_precise_currency_rate),
            formatting::date_cell(trade.execution_date),
            formatting::decimal_cell(execution_precise_currency_rate),

            Cell::new_align(&trade.quantity.to_string(), Alignment::RIGHT),
            formatting::cash_cell(trade.price),
            formatting::cash_cell(details.revenue),
            formatting::cash_cell(details.local_revenue),
            formatting::cash_cell(trade.commission),
            formatting::cash_cell(Cash::new(country.currency, commission)),

            // FIXME: rounds
            formatting::cash_cell(details.local_cost.round()),
            formatting::cash_cell(purchase_local_cost.round()),
            formatting::cash_cell(details.local_profit.round()),
            formatting::cash_cell(details.tax_to_pay.round()),
//            formatting::date_cell(dividend.date),
//            Cell::new_align(&issuer, Alignment::LEFT),
//            Cell::new_align(foreign_amount.currency, Alignment::CENTER),
//
//            formatting::cash_cell(foreign_amount),
//            formatting::decimal_cell(precise_currency_rate),
//            formatting::cash_cell(Cash::new(country.currency, amount)),
//
//            formatting::cash_cell(foreign_paid_tax),
//            formatting::cash_cell(Cash::new(country.currency, paid_tax)),
//            formatting::cash_cell(Cash::new(country.currency, tax_to_pay)),
//            formatting::cash_cell(Cash::new(country.currency, income)),
        ]));

        if let Some(ref mut tax_statement) = tax_statement {
            // FIXME: Stock selling support
        }
    }

    if !table.is_empty() {
        /*
            table.add_row(Row::new(vec![
                formatting::empty_cell(),
                formatting::empty_cell(),
                formatting::empty_cell(),

                formatting::multi_currency_cash_cell(total_foreign_amount),
                formatting::empty_cell(),
                formatting::cash_cell(Cash::new(country.currency, total_amount)),

                formatting::multi_currency_cash_cell(total_foreign_paid_tax),
                formatting::cash_cell(Cash::new(country.currency, total_paid_tax)),
                formatting::cash_cell(Cash::new(country.currency, total_tax_to_pay)),
                formatting::cash_cell(Cash::new(country.currency, total_income)),
            ]));
        */

        formatting::print_statement(
            &format!("Расчет дохода от продажи ценных бумаг, полученного через {}",
                     broker_statement.broker.name),
            &[
                "№", "Наименование ценной бумаги",
                "Дата сделки", "Курс руб.", "Дата расчета", "Курс руб.",
                "Кол.", "Цена", "Стоимость", "Стоимость (руб)", "Комиссия", "Комиссия (руб)",
                "Затраты на покупку", "Общие затраты", "Доход", "К уплате",
                // "Валюта", "Сумма", "Сумма (руб)",
                // "Уплачено", "Уплачено (руб)", "К доплате", "Реальный доход",
            ],
            table,
        );
    }

    Ok(())
}