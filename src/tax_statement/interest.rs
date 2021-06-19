use chrono::Datelike;
use log::warn;

use static_table_derive::StaticTable;

use crate::broker_statement::BrokerStatement;
use crate::core::GenericResult;
use crate::currency::{Cash, MultiCurrencyCashAccount};
use crate::currency::converter::CurrencyConverter;
use crate::formatting;
use crate::localities::{Country, Jurisdiction};
use crate::types::{Date, Decimal};

use super::statement::TaxStatement;

#[derive(StaticTable)]
struct Row {
    #[column(name="Дата")]
    date: Date,
    #[column(name="Валюта", align="center")]
    currency: String,
    #[column(name="Сумма")]
    foreign_amount: Cash,
    #[column(name="Курс руб.")]
    currency_rate: Option<Decimal>,
    #[column(name="Сумма (руб)")]
    amount: Cash,
    #[column(name="К уплате")]
    tax_to_pay: Cash,
    #[column(name="Реальный доход")]
    income: Cash,
}

pub fn process_income(
    country: &Country, broker_statement: &BrokerStatement, year: Option<i32>,
    mut tax_statement: Option<&mut TaxStatement>, converter: &CurrencyConverter,
) -> GenericResult<Cash> {
    let mut table = Table::new();

    let mut total_foreign_amount = MultiCurrencyCashAccount::new();
    let mut total_amount = Cash::zero(country.currency);
    let mut total_tax_to_pay = Cash::zero(country.currency);
    let mut total_income = Cash::zero(country.currency);

    for interest in &broker_statement.idle_cash_interest {
        if let Some(year) = year {
            if interest.date.year() != year {
                continue;
            }
        }

        if interest.amount.is_negative() {
            continue;
        }

        let foreign_amount = interest.amount.round();
        total_foreign_amount.deposit(foreign_amount);

        let precise_currency_rate = converter.precise_currency_rate(
            interest.date, foreign_amount.currency, country.currency)?;

        let amount = converter.convert_to_cash_rounding(interest.date, foreign_amount, country.currency)?;
        total_amount.add_assign(amount).unwrap();

        let tax_to_pay = interest.tax_to_pay(&country, converter)?;
        total_tax_to_pay.add_assign(tax_to_pay).unwrap();

        let income = amount.sub(tax_to_pay).unwrap();
        total_income.add_assign(income).unwrap();

        table.add_row(Row {
            date: interest.date,
            currency: foreign_amount.currency.to_owned(),
            foreign_amount: foreign_amount,
            currency_rate: if foreign_amount.currency != country.currency {
                Some(precise_currency_rate)
            } else {
                None
            },
            amount, tax_to_pay, income,
        });

        if tax_statement.is_some() && broker_statement.broker.type_.jurisdiction() != Jurisdiction::Usa {
            warn!(concat!(
                "Tax statement generation for interest income is supported only for brokers with USA jurisdiction. ",
                "Don't adding it to the tax statement."
            ));
            tax_statement = None;
        } else if let Some(ref mut tax_statement) = tax_statement {
            let description = format!(
                "{}: Проценты на остаток по брокерскому счету", broker_statement.broker.name);

            tax_statement.add_interest_income(
                &description, interest.date, foreign_amount.currency, precise_currency_rate,
                foreign_amount.amount, amount.amount
            ).map_err(|e| format!(
                "Unable to add interest income from {} to the tax statement: {}",
                formatting::format_date(interest.date), e
            ))?;
        }
    }

    if !table.is_empty() {
        let mut totals = table.add_empty_row();
        totals.set_foreign_amount(total_foreign_amount);
        totals.set_amount(total_amount);
        totals.set_tax_to_pay(total_tax_to_pay);
        totals.set_income(total_income);

        table.print(&format!(
            "Расчет дохода от процентов на остаток по брокерскому счету, полученных через {}",
            broker_statement.broker.name));
    }

    Ok(total_tax_to_pay)
}