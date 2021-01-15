use chrono::Datelike;
use num_traits::Zero;

use static_table_derive::StaticTable;

use crate::broker_statement::BrokerStatement;
use crate::core::GenericResult;
use crate::currency::{Cash, MultiCurrencyCashAccount};
use crate::currency::converter::CurrencyConverter;
use crate::localities::Country;
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
    currency_rate: Decimal,
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

    let mut total_foreign_amount = MultiCurrencyCashAccount::new();
    let mut total_amount = dec!(0);

    let mut total_foreign_paid_tax = MultiCurrencyCashAccount::new();
    let mut total_paid_tax = dec!(0);
    let mut total_tax_deduction = dec!(0);
    let mut total_tax_to_pay = dec!(0);

    let mut total_income = dec!(0);

    for dividend in &broker_statement.dividends {
        if let Some(year) = year {
            if dividend.date.year() != year {
                continue;
            }
        }

        let issuer = broker_statement.get_instrument_name(&dividend.issuer);

        let foreign_amount = dividend.amount.round();
        total_foreign_amount.deposit(foreign_amount);

        let precise_currency_rate = converter.precise_currency_rate(
            dividend.date, foreign_amount.currency, country.currency)?;

        let amount = converter.convert_to_rounding(
            dividend.date, foreign_amount, country.currency)?;
        total_amount += amount;

        let tax = dividend.tax(&country, converter)?;

        let foreign_paid_tax = dividend.paid_tax.round();
        total_foreign_paid_tax.deposit(foreign_paid_tax);

        let paid_tax = converter.convert_to_rounding(
            dividend.date, foreign_paid_tax, country.currency)?;
        total_paid_tax += paid_tax;

        let tax_to_pay = dividend.tax_to_pay(&country, converter)?;
        total_tax_to_pay += tax_to_pay;

        let tax_deduction = country.round_tax(paid_tax);
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

            foreign_amount: foreign_amount,
            currency_rate: precise_currency_rate,
            amount: Cash::new(country.currency, amount),

            tax: Cash::new(country.currency, tax),
            foreign_paid_tax: foreign_paid_tax,
            paid_tax: Cash::new(country.currency, paid_tax),
            tax_deduction: Cash::new(country.currency, tax_deduction),
            tax_to_pay: Cash::new(country.currency, tax_to_pay),
            income: Cash::new(country.currency, income),
        });

        if let Some(ref mut tax_statement) = tax_statement {
            let description = format!("{}: Дивиденд от {}", broker_statement.broker.name, issuer);

            if foreign_paid_tax.currency != foreign_amount.currency {
                return Err!(
                    "{}: Tax currency is different from dividend currency: {} vs {}",
                    dividend.description(), foreign_paid_tax.currency, foreign_amount.currency);
            }

            tax_statement.add_dividend_income(
                &description, dividend.date, foreign_amount.currency, precise_currency_rate,
                foreign_amount.amount, foreign_paid_tax.amount, amount, paid_tax
            ).map_err(|e| format!(
                "Unable to add {} to the tax statement: {}", dividend.description(), e
            ))?;
        }
    }

    if !table.is_empty() {
        let mut totals = table.add_empty_row();

        totals.set_foreign_amount(total_foreign_amount);
        totals.set_amount(Cash::new(country.currency, total_amount));

        totals.set_foreign_paid_tax(total_foreign_paid_tax);
        totals.set_paid_tax(Cash::new(country.currency, total_paid_tax));
        totals.set_tax_deduction(Cash::new(country.currency, total_tax_deduction));
        totals.set_tax_to_pay(Cash::new(country.currency, total_tax_to_pay));
        totals.set_income(Cash::new(country.currency, total_income));

        table.print(&format!(
            "Расчет дохода от дивидендов, полученных через {}", broker_statement.broker.name));
    }

    Ok(Cash::new(country.currency, total_tax_to_pay))
}