use chrono::Datelike;
use log::warn;

use static_table_derive::StaticTable;

use crate::broker_statement::BrokerStatement;
use crate::core::GenericResult;
use crate::currency::{Cash, MultiCurrencyCashAccount};
use crate::currency::converter::CurrencyConverter;
use crate::formatting;
use crate::localities::{Country, Jurisdiction};
use crate::tax_statement::statement::CountryCode;
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
) -> GenericResult<(Cash, bool)> {
    let broker_jurisdiction = broker_statement.broker.type_.jurisdiction();

    let mut table = Table::new();
    let mut has_income = false;

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

        has_income = true;

        let foreign_amount = interest.amount.round();
        total_foreign_amount.deposit(foreign_amount);

        let precise_currency_rate = converter.precise_currency_rate(
            interest.date, foreign_amount.currency, country.currency)?;

        let amount = converter.convert_to_cash_rounding(interest.date, foreign_amount, country.currency)?;
        total_amount += amount;

        let tax_to_pay = interest.tax_to_pay(country, converter)?;
        total_tax_to_pay += tax_to_pay;

        let income = amount - tax_to_pay;
        total_income += income;

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

        if let Some(ref mut statement) = tax_statement {
            match broker_jurisdiction {
                Jurisdiction::Usa => {
                    let country_code = CountryCode::new(broker_jurisdiction.code())?;
                    let description = format!(
                        "{}: Проценты на остаток по брокерскому счету",
                        broker_statement.broker.name);

                    statement.add_interest_income(
                        &description, interest.date, country_code,
                        foreign_amount.currency, precise_currency_rate,
                        foreign_amount.amount, amount.amount
                    ).map_err(|e| format!(
                        "Unable to add interest income from {} to the tax statement: {}",
                        formatting::format_date(interest.date), e
                    ))?;
                },

                Jurisdiction::Russia => {
                    warn!(concat!(
                        "Don't declare interest income in the tax statement ",
                        "assuming that it will be declared by broker's tax agent.",
                    ));
                    tax_statement = None;
                }
            }
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

    Ok((total_tax_to_pay, has_income))
}