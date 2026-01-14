use serde::{Serialize, Serializer};

use crate::core::GenericResult;
use crate::types::{Date, Decimal};

use super::countries::CountryCode;
use super::encoding::{serialize_date, serialize_decimal, serialize_with_default};

#[derive(Debug, Serialize)]
pub struct ForeignIncome {
    #[serde(rename = "IncomeCod")]
    pub type_: IncomeType,
    #[serde(rename = "SourseName")]
    pub description: String,

    #[serde(rename = "IncomeGetDate", serialize_with = "serialize_date")]
    pub date: Date,
    #[serde(rename = "HoldDate", serialize_with = "serialize_date")]
    pub tax_payment_date: Date,

    #[serde(rename = "CountryCode1")]
    pub source_from: CountryCode,
    #[serde(rename = "CountryCode2")]
    pub received_in: CountryCode,
    #[serde(flatten)]
    pub currency: Currency,

    #[serde(rename = "IncomeSum", serialize_with = "serialize_decimal")]
    pub amount: Decimal,
    #[serde(rename = "IncomeRoubleSum", serialize_with = "serialize_decimal")]
    pub local_amount: Decimal,

    #[serde(flatten, serialize_with = "serialize_with_default")]
    pub paid_tax: Option<PaidTax>,
    #[serde(flatten, serialize_with = "serialize_with_default")]
    pub deduction: Option<Deduction>,

    #[serde(flatten, serialize_with = "serialize_with_default")]
    pub controlled_foreign_company: Option<ControlledForeignCompany>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncomeType {
    Dividend,
    Interest,
    Stock,
}

impl Serialize for IncomeType {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(match self {
            IncomeType::Dividend => "1010", // Дивиденды
            IncomeType::Stock    => "1530", // (01)Доходы от реализации ЦБ (обращ-ся на орг. рынке ЦБ)
            IncomeType::Interest => "6013", // "Доходы в виде процентов, полученных от источников за пределами Российской Федерации, в отношении которых применяется налоговая ставка, предусмотренная пунктом 1 статьи 224 Кодекса"),
        })
    }
}

#[derive(Debug, Serialize)]
pub struct Currency {
    #[serde(rename = "CurrencyCode")]
    pub code: &'static str,
    #[serde(rename = "CurrencyName")]
    pub name: &'static str,

    #[serde(rename = "CurrencyExchange", serialize_with = "serialize_decimal")]
    pub income_date_rate: Decimal,
    #[serde(rename = "CurrencyUnits")]
    pub income_date_units: u16,

    #[serde(rename = "CurrencyExchangeHoldDate", serialize_with = "serialize_decimal")]
    pub tax_payment_date_rate: Decimal,
    #[serde(rename = "CurrencyUnitsHoldDate")]
    pub tax_payment_date_units: u16,

    #[serde(rename = "IsAutoExchange")]
    pub automatic_currency_rates: bool,
}

impl Currency {
    pub fn new(currency: &str, precise_currency_rate: Decimal) -> GenericResult<Currency> {
        let (code, name, units) = match currency {
            "AUD" => ("036", "Австралийский доллар", 100),
            "EUR" => ("978", "Евро", 100),
            "GBP" => ("826", "Фунт стерлингов", 100),
            "HKD" => ("344", "Гонконгский доллар", 100),
            "RUB" => ("643", "Российский рубль", 1000),
            "USD" => ("840", "Доллар США", 100),
            _ => return Err!("{currency} currency is not supported yet"),
        };

        let rate = crate::currency::round(precise_currency_rate * Decimal::from(units));

        Ok(Currency {
            code,
            name,

            income_date_rate: rate,
            income_date_units: units,

            tax_payment_date_rate: rate,
            tax_payment_date_units: units,

            // It's always false for some reason, although it's actually always automatic
            automatic_currency_rates: false,
        })
    }
}

#[derive(Debug, Default, Serialize)]
pub struct PaidTax {
    #[serde(rename = "TaxRate")]
    pub tax_rate: u8,

    #[serde(rename = "TaxCurrencySum", serialize_with = "serialize_decimal")]
    pub amount: Decimal,

    #[serde(rename = "TaxRoubleSum", serialize_with = "serialize_decimal")]
    pub local_amount: Decimal,
}

impl PaidTax {
    pub fn new(amount: Decimal, local_amount: Decimal) -> PaidTax {
        PaidTax {
            tax_rate: 0, // Don't know what to do with it – it's always zero
            amount,
            local_amount,
        }
    }
}

#[derive(Debug, Default, Serialize)]
pub struct Deduction {
    #[serde(rename = "SubtractCod")]
    pub code: &'static str,

    #[serde(rename = "SubtractSum", serialize_with = "serialize_decimal")]
    pub amount: Decimal,
}

#[derive(Debug, Default, Serialize)]
pub struct ControlledForeignCompany {
    #[serde(rename = "KIKIncomeType")]
    profit_calculation_method: u8,

    #[serde(rename = "KIKNum")]
    number: &'static str,

    #[serde(rename = "TaxKIKHoldRF", serialize_with = "serialize_decimal")]
    paid_tax: Decimal,
}