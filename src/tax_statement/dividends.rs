use std::collections::BTreeSet;

use chrono::Datelike;
use itertools::Itertools;
use log::warn;

use static_table_derive::StaticTable;

use crate::broker_statement::BrokerStatement;
use crate::brokers::Broker;
use crate::core::GenericResult;
use crate::currency::{Cash, MultiCurrencyCashAccount};
use crate::currency::converter::CurrencyConverter;
use crate::instruments::IssuerTaxationType;
use crate::localities::{Country, Jurisdiction};
use crate::types::{Date, Decimal};

use super::statement::TaxStatement;

#[derive(StaticTable)]
struct Row {
    #[column(name="Дата")]
    date: Date,
    #[column(name="Эмитент")]
    issuer: String,
    #[column(name="Валюта", align="center")]
    currency: String,

    #[column(name="Сумма")]
    foreign_amount: Cash,
    #[column(name="Курс руб.")]
    currency_rate: Option<Decimal>,
    #[column(name="Сумма (руб)")]
    amount: Cash,

    #[column(name="Налог")]
    tax: Cash,
    #[column(name="Уплачено")]
    foreign_paid_tax: Cash,
    #[column(name="Уплачено (руб)")]
    paid_tax: Cash,
    #[column(name="К зачету")]
    tax_deduction: Cash,
    #[column(name="К доплате")]
    tax_to_pay: Cash,
    #[column(name="Реальный доход")]
    income: Cash,
}

pub fn process_income(
    country: &Country, broker_statement: &BrokerStatement, year: Option<i32>,
    mut tax_statement: Option<&mut TaxStatement>, converter: &CurrencyConverter,
) -> GenericResult<Cash> {
    let mut table = Table::new();
    let mut same_currency = true;
    let mut tax_agent_issuers = BTreeSet::new();

    let mut total_foreign_amount = MultiCurrencyCashAccount::new();
    let mut total_amount = Cash::zero(country.currency);

    let mut total_foreign_paid_tax = MultiCurrencyCashAccount::new();
    let mut total_paid_tax = Cash::zero(country.currency);
    let mut total_tax_deduction = Cash::zero(country.currency);
    let mut total_tax_to_pay = Cash::zero(country.currency);

    let mut total_income = Cash::zero(country.currency);

    for dividend in &broker_statement.dividends {
        if let Some(year) = year {
            if dividend.date.year() != year {
                continue;
            }
        }

        let issuer = broker_statement.instrument_info.get_name(&dividend.original_issuer);

        let foreign_amount = dividend.amount.round();
        total_foreign_amount.deposit(foreign_amount);
        same_currency &= foreign_amount.currency == country.currency;

        let precise_currency_rate = converter.precise_currency_rate(
            dividend.date, foreign_amount.currency, country.currency)?;

        let amount = converter.convert_to_cash_rounding(
            dividend.date, foreign_amount, country.currency)?;
        total_amount += amount;

        let tax = dividend.tax(country, converter)?;

        let foreign_paid_tax = dividend.paid_tax.round();
        total_foreign_paid_tax.deposit(foreign_paid_tax);
        same_currency &= foreign_paid_tax.currency == country.currency;

        let paid_tax = converter.convert_to_cash_rounding(
            dividend.date, foreign_paid_tax, country.currency)?;
        total_paid_tax += paid_tax;

        let tax_to_pay = dividend.tax_to_pay(country, converter)?;
        total_tax_to_pay += tax_to_pay;

        let tax_deduction = country.round_tax(paid_tax);
        if dividend.taxation_type == IssuerTaxationType::TaxAgent && tax_deduction != paid_tax {
            return Err!(
                "Got an unexpected withheld tax for {}: {} vs {}",
                dividend.description(), paid_tax, tax_deduction);
        }

        if !tax_to_pay.is_zero() {
            assert_eq!(tax_deduction, tax - tax_to_pay);
        }
        total_tax_deduction += tax_deduction;

        let income = amount - paid_tax - tax_to_pay;
        total_income += income;

        table.add_row(Row {
            date: dividend.date,
            issuer: issuer.to_owned(),
            currency: foreign_amount.currency.to_owned(),

            foreign_amount,
            currency_rate: if foreign_amount.currency == country.currency {
                None
            } else {
                Some(precise_currency_rate)
            },
            amount,

            tax, foreign_paid_tax, paid_tax, tax_deduction, tax_to_pay, income,
        });

        match dividend.taxation_type {
            IssuerTaxationType::Manual => {
                if let Some(ref mut tax_statement) = tax_statement {
                    let description = format!("{}: Дивиденд от {}", broker_statement.broker.name, issuer);

                    if foreign_paid_tax.currency != foreign_amount.currency {
                        return Err!(
                            "{}: Tax currency is different from dividend currency: {} vs {}",
                            dividend.description(), foreign_paid_tax.currency, foreign_amount.currency);
                    }

                    tax_statement.add_dividend_income(
                        &description, dividend.date, foreign_amount.currency, precise_currency_rate,
                        foreign_amount.amount, foreign_paid_tax.amount, amount.amount, paid_tax.amount
                    ).map_err(|e| format!(
                        "Unable to add {} to the tax statement: {}", dividend.description(), e
                    ))?;
                }
            },
            IssuerTaxationType::TaxAgent => {
                tax_agent_issuers.insert(&dividend.original_issuer);
            },
        }
    }

    if !tax_agent_issuers.is_empty() {
        // FIXME(konishchev): Document it
        eprintln!(); warn!(
            "The following dividend issuers are identified as taxed by broker's tax agent: {}.",
            tax_agent_issuers.iter().join(", "));
    }

    if same_currency {
        table.hide_currency_rate();
        table.hide_amount();
        table.hide_paid_tax();
    }

    if !table.is_empty() {
        if broker_statement.broker.type_.jurisdiction() == Jurisdiction::Russia {
            let mut messages = Vec::new();

            if broker_statement.broker.type_ == Broker::Tinkoff {
                // https://github.com/KonishchevDmitry/investments/issues/26
                let url = "http://bit.ly/investments-tinkoff-dividends";
                messages.push(format!(
                    "The following calculations for dividend income are very inaccurate (see {}).",
                    url,
                ));

                if tax_statement.is_some() {
                    messages.push(s!("The result tax statement must be corrected manually."))
                }
            }

            if tax_statement.is_some() {
                // FIXME(konishchev): Determine income country
                messages.push(s!(
                    "Please take into account that all dividends will be declared with USA jurisdiction."));
            };

            if !messages.is_empty() {
                eprintln!(); warn!("{}", messages.join(" "));
            }
        }

        let mut totals = table.add_empty_row();

        totals.set_foreign_amount(total_foreign_amount);
        totals.set_amount(total_amount);

        totals.set_foreign_paid_tax(total_foreign_paid_tax);
        totals.set_paid_tax(total_paid_tax);
        totals.set_tax_deduction(total_tax_deduction);
        totals.set_tax_to_pay(total_tax_to_pay);
        totals.set_income(total_income);

        table.print(&format!(
            "Расчет дохода от дивидендов, полученных через {}", broker_statement.broker.name));
    }

    Ok(total_tax_to_pay)
}