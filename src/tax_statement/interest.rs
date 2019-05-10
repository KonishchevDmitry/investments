use chrono::Datelike;

use crate::broker_statement::BrokerStatement;
use crate::config::PortfolioConfig;
use crate::core::EmptyResult;
use crate::currency::{self, Cash, MultiCurrencyCashAccount};
use crate::currency::converter::CurrencyConverter;
use crate::formatting;
use crate::formatting::table::{Table, Row, Cell, Alignment, print_table};

use super::statement::TaxStatement;

pub fn process_income(
    portfolio: &PortfolioConfig, broker_statement: &BrokerStatement, year: Option<i32>,
    mut tax_statement: Option<&mut TaxStatement>, converter: &CurrencyConverter,
) -> EmptyResult {
    let mut table = Table::new();
    let country = portfolio.get_tax_country();

    let mut total_foreign_amount = MultiCurrencyCashAccount::new();
    let mut total_amount = dec!(0);
    let mut total_tax_to_pay = dec!(0);
    let mut total_income = dec!(0);

    for interest in &broker_statement.idle_cash_interest {
        if let Some(year) = year {
            if interest.date.year() != year {
                continue;
            }
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

        table.add_row(Row::new(&[
            Cell::new_date(interest.date),
            Cell::new_align(foreign_amount.currency, Alignment::CENTER),
            Cell::new_cash(foreign_amount),
            Cell::new_decimal(precise_currency_rate),
            Cell::new_cash(Cash::new(country.currency, amount)),
            Cell::new_cash(Cash::new(country.currency, tax_to_pay)),
            Cell::new_cash(Cash::new(country.currency, income)),
        ]));

        if let Some(ref mut tax_statement) = tax_statement {
            let description = format!(
                "{}: Проценты на остаток по брокерскому счету", broker_statement.broker.name);

            tax_statement.add_interest_income(
                &description, interest.date, foreign_amount.currency, precise_currency_rate,
                foreign_amount.amount, amount
            ).map_err(|e| format!(
                "Unable to add interest income from {} to the tax statement: {}",
                formatting::format_date(interest.date), e
            ))?;
        }
    }

    if !table.is_empty() {
        table.add_row(Row::new(&[
            Cell::new_empty(),
            Cell::new_empty(),
            Cell::new_multi_currency_cash(total_foreign_amount),
            Cell::new_empty(),
            Cell::new_cash(Cash::new(country.currency, total_amount)),
            Cell::new_cash(Cash::new(country.currency, total_tax_to_pay)),
            Cell::new_cash(Cash::new(country.currency, total_income)),
        ]));

        print_table(
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