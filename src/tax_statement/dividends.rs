use std::collections::BTreeSet;
use std::fmt;

use chrono::Datelike;
use itertools::Itertools;
use log::warn;

use static_table_derive::StaticTable;

use crate::brokers::Broker;
use crate::broker_statement::{BrokerStatement, Dividend};
use crate::core::{EmptyResult, GenericResult};
use crate::currency::{Cash, MultiCurrencyCashAccount};
use crate::currency::converter::CurrencyConverter;
use crate::instruments::IssuerTaxationType;
use crate::localities::{Country, Jurisdiction};
use crate::types::{Date, Decimal};

use super::statement::{TaxStatement, CountryCode};

pub fn process_income(
    country: &Country, broker_statement: &BrokerStatement, year: Option<i32>,
    tax_statement: Option<&mut TaxStatement>, converter: &CurrencyConverter,
) -> GenericResult<(Cash, bool, bool)> {
    let mut processor = Processor {
        broker_statement, tax_statement, tax_year: year,
        country, converter,

        table: Table::new(),
        warning: false,
        warning_firstrade_income_jurisdiction: false,

        same_currency: true,
        tax_agent_issuers: BTreeSet::new(),

        has_income: false,
        has_income_to_declare: false,

        total_foreign_amount: MultiCurrencyCashAccount::new(),
        total_amount: Cash::zero(country.currency),

        total_foreign_paid_tax: MultiCurrencyCashAccount::new(),
        total_paid_tax: Cash::zero(country.currency),
        total_tax_deduction: Cash::zero(country.currency),
        total_tax_to_pay: Cash::zero(country.currency),

        total_income: Cash::zero(country.currency),
    };

    processor.process_dividends()?;

    let total_tax_to_pay = processor.total_tax_to_pay;
    let has_income = processor.has_income;
    let has_income_to_declare = processor.has_income_to_declare;

    processor.print();

    Ok((total_tax_to_pay, has_income, has_income_to_declare))
}

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

struct Processor<'a> {
    broker_statement: &'a BrokerStatement,
    tax_statement: Option<&'a mut TaxStatement>,
    tax_year: Option<i32>,

    country: &'a Country,
    converter: &'a CurrencyConverter,

    table: Table,
    warning: bool,
    warning_firstrade_income_jurisdiction: bool,

    same_currency: bool,
    tax_agent_issuers: BTreeSet<String>,

    has_income: bool,
    has_income_to_declare: bool,

    total_foreign_amount: MultiCurrencyCashAccount,
    total_amount: Cash,

    total_foreign_paid_tax: MultiCurrencyCashAccount,
    total_paid_tax: Cash,
    total_tax_deduction: Cash,
    total_tax_to_pay: Cash,

    total_income: Cash,
}

impl<'a> Processor<'a> {
    fn process_dividends(&mut self) -> EmptyResult {
        for dividend in &self.broker_statement.dividends {
            if let Some(year) = self.tax_year {
                if dividend.date.year() != year {
                    continue;
                }
            }
            self.process_dividend(dividend)?;
        }

        if !self.tax_agent_issuers.is_empty() {
            // https://github.com/KonishchevDmitry/investments/blob/master/docs/taxes.md#russian-brokers
            let url = "https://bit.ly/investments-russian-brokers-taxes";
            self.warn(format_args!(
                "The following dividend issuers are identified as taxed by broker's tax agent: {} (see {}).",
                self.tax_agent_issuers.iter().join(", "), url));
        }

        Ok(())
    }

    fn process_dividend(&mut self, dividend: &Dividend) -> EmptyResult {
        let issuer = self.broker_statement.instrument_info.get_name(&dividend.original_issuer);

        let foreign_amount = dividend.amount.round();
        self.total_foreign_amount.deposit(foreign_amount);
        self.same_currency &= foreign_amount.currency == self.country.currency;

        let precise_currency_rate = self.converter.precise_currency_rate(
            dividend.date, foreign_amount.currency, self.country.currency)?;

        let amount = self.converter.convert_to_cash_rounding(
            dividend.date, foreign_amount, self.country.currency)?;
        self.total_amount += amount;

        let tax = dividend.tax(self.country, self.converter)?;

        let foreign_paid_tax = dividend.paid_tax.round();
        self.total_foreign_paid_tax.deposit(foreign_paid_tax);
        self.same_currency &= foreign_paid_tax.currency == self.country.currency;

        let paid_tax = self.converter.convert_to_cash_rounding(
            dividend.date, foreign_paid_tax, self.country.currency)?;
        self.total_paid_tax += paid_tax;

        let tax_to_pay = dividend.tax_to_pay(self.country, self.converter)?;
        self.total_tax_to_pay += tax_to_pay;

        let tax_deduction = std::cmp::min(self.country.round_tax(paid_tax), tax);
        if dividend.taxation_type == IssuerTaxationType::TaxAgent && tax_deduction != paid_tax {
            return Err!(
                "Got an unexpected withheld tax for {}: {} vs {}",
                dividend.description(), paid_tax, tax_deduction);
        }

        if !tax_to_pay.is_zero() {
            assert_eq!(tax_deduction, tax - tax_to_pay);
        }
        self.total_tax_deduction += tax_deduction;

        let income = amount - paid_tax - tax_to_pay;
        self.total_income += income;

        self.has_income = true;
        self.table.add_row(Row {
            date: dividend.date,
            issuer: issuer.to_owned(),
            currency: foreign_amount.currency.to_owned(),

            foreign_amount,
            currency_rate: if foreign_amount.currency == self.country.currency {
                None
            } else {
                Some(precise_currency_rate)
            },
            amount,

            tax, foreign_paid_tax, paid_tax, tax_deduction, tax_to_pay, income,
        });

        match dividend.taxation_type {
            IssuerTaxationType::Manual(ref income_country) => {
                self.add_income(
                    dividend, &issuer, income_country.as_deref(),
                    foreign_amount, precise_currency_rate, foreign_paid_tax,
                    amount, paid_tax,
                )?;
            },
            IssuerTaxationType::TaxAgent => {
                self.tax_agent_issuers.insert(dividend.original_issuer.clone());
            },
        }

        Ok(())
    }

    fn add_income(
        &mut self, dividend: &Dividend, issuer: &str, income_country: Option<&str>,
        foreign_amount: Cash, precise_currency_rate: Decimal, foreign_paid_tax: Cash,
        amount: Cash, paid_tax: Cash,
    ) -> EmptyResult {
        let broker = &self.broker_statement.broker;

        let income_country = match income_country {
            Some(country) => country,
            None => match broker.type_ {
                Broker::Firstrade => {
                    if !self.warning_firstrade_income_jurisdiction {
                        self.warning_firstrade_income_jurisdiction = true;

                        // https://github.com/KonishchevDmitry/investments/blob/master/docs/taxes.md#firstrade-income-jurisdiction
                        let url = "https://bit.ly/investments-firstrade-income-jurisdiction";

                        self.warn(format_args!(concat!(
                            "Firstrade don't provide dividend issuer jurisdiction information, ",
                            "so all dividend income will be declared with USA jurisdiction (see {})."
                        ), url));
                    }
                    Jurisdiction::Usa
                },

                // Old IB statements may not contain ISIN/CUSIP information (conid was used instead)
                Broker::InteractiveBrokers if dividend.date.year() < 2020 => {
                    Jurisdiction::Usa
                },

                _ => {
                    return Err!(
                        "Unable to determine {} jurisdiction: there is no ISIN information for it in the broker statement",
                        dividend.original_issuer);
                },
            }.code()
        };

        if foreign_paid_tax.currency != foreign_amount.currency {
            return Err!(
                "{}: Tax currency is different from dividend currency: {} vs {}",
                dividend.description(), foreign_paid_tax.currency, foreign_amount.currency);
        }

        self.has_income_to_declare = true;

        if let Some(ref mut tax_statement) = self.tax_statement {
            let source_from = CountryCode::new(income_country)?;
            let received_in = CountryCode::new(broker.type_.jurisdiction().code())?;
            let description = format!("{}: Дивиденд от {}", broker.name, issuer);

            tax_statement.add_dividend_income(
                &description, dividend.date, source_from, received_in,
                foreign_amount.currency, precise_currency_rate,
                foreign_amount.amount, foreign_paid_tax.amount,
                amount.amount, paid_tax.amount
            ).map_err(|e| format!(
                "Unable to add {} to the tax statement: {}", dividend.description(), e
            ))?;
        }

        Ok(())
    }

    fn print(self) {
        let mut table = self.table;
        if table.is_empty() {
            return;
        }

        if self.same_currency {
            table.hide_currency_rate();
            table.hide_amount();
            table.hide_paid_tax();
        }

        let mut totals = table.add_empty_row();

        totals.set_foreign_amount(self.total_foreign_amount);
        totals.set_amount(self.total_amount);

        totals.set_foreign_paid_tax(self.total_foreign_paid_tax);
        totals.set_paid_tax(self.total_paid_tax);
        totals.set_tax_deduction(self.total_tax_deduction);
        totals.set_tax_to_pay(self.total_tax_to_pay);
        totals.set_income(self.total_income);

        table.print(&format!(
            "Расчет дохода от дивидендов, полученных через {}",
            self.broker_statement.broker.name));
    }

    fn warn(&mut self, args: fmt::Arguments) {
        if !self.warning {
            self.warning = true;
            eprintln!();
        }
        warn!("{}", args);
    }
}