use chrono::Datelike;
use prettytable::{Table, Row, Cell};
use prettytable::format::Alignment;

use crate::broker_statement::BrokerStatement;
use crate::core::EmptyResult;
use crate::currency::{self, Cash, MultiCurrencyCashAccount};
use crate::currency::converter::CurrencyConverter;
use crate::formatting;
use crate::localities::Country;

use super::statement::TaxStatement;

pub fn process_income(
    broker_statement: &BrokerStatement, year: i32, mut tax_statement: Option<&mut TaxStatement>,
    country: &Country, converter: &CurrencyConverter,
) -> EmptyResult {
    let mut table = Table::new();

    let mut total_foreign_amount = MultiCurrencyCashAccount::new();
    let mut total_amount = dec!(0);
    let mut total_tax_to_pay = dec!(0);
    let mut total_income = dec!(0);

    for interest in &broker_statement.idle_cash_interest {
        if interest.date.year() != year {
            continue;
        }

        let foreign_amount = interest.amount.round();
        total_foreign_amount.deposit(foreign_amount);

        let precise_currency_rate = converter.precise_currency_rate(
            interest.date, foreign_amount.currency, country.currency)?;

        let amount = currency::round(converter.convert_to(
            interest.date, interest.amount, country.currency)?);
        total_amount += amount;

        let tax_to_pay = interest.tax_to_pay(&country, converter)?;
        total_tax_to_pay += tax_to_pay;

        let income = amount - tax_to_pay;
        total_income += income;

        table.add_row(Row::new(vec![
            formatting::date_cell(interest.date),
            Cell::new_align(foreign_amount.currency, Alignment::CENTER),
            formatting::cash_cell(foreign_amount),
            formatting::decimal_cell(precise_currency_rate),
            formatting::cash_cell(Cash::new(country.currency, amount)),
            formatting::cash_cell(Cash::new(country.currency, tax_to_pay)),
            formatting::cash_cell(Cash::new(country.currency, income)),
        ]));

        if let Some(ref mut tax_statement) = tax_statement {
            // FIXME: Support idle cash interest
            let _ = tax_statement;
        }
    }

    if !table.is_empty() {
        table.add_row(Row::new(vec![
            formatting::empty_cell(),
            formatting::empty_cell(),
            formatting::multi_currency_cash_cell(total_foreign_amount),
            formatting::empty_cell(),
            formatting::cash_cell(Cash::new(country.currency, total_amount)),
            formatting::cash_cell(Cash::new(country.currency, total_tax_to_pay)),
            formatting::cash_cell(Cash::new(country.currency, total_income)),
        ]));

        formatting::print_statement(
            &format!(
                "Расчет дохода от процентов на остаток по брокерскому счету, полученных через {}",
                broker_statement.broker.name
            ),
            &["Дата", "Валюта", "Сумма", "Курс руб.", "Сумма (руб)", "К уплате", "Реальный доход"],
            table,
        );
    }

    Ok(())
}