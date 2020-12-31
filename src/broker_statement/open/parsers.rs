#[cfg(test)] use matches::assert_matches;
use lazy_static::lazy_static;
use num_traits::{FromPrimitive, ToPrimitive};
use regex::Regex;
use serde::{Deserialize, Deserializer};
use serde::de::Error;

use crate::core::GenericResult;
use crate::types::{Date, Decimal};
use crate::util;

fn parse_date(date: &str) -> GenericResult<Date> {
    util::parse_date(date, "%Y-%m-%dT00:00:00")
}

pub fn deserialize_date<'de, D>(deserializer: D) -> Result<Date, D::Error>
    where D: Deserializer<'de>
{
    let value: String = Deserialize::deserialize(deserializer)?;
    parse_date(&value).map_err(D::Error::custom)
}

pub fn parse_security_description(mut issuer: &str) -> &str {
    if let Some(index) = issuer.find("п/у") {
        issuer = &issuer[..index];
    }

    if let Some(index) = issuer.find('(') {
        issuer = &issuer[..index];
    }

    issuer.trim()
}

pub fn parse_quantity(decimal_quantity: Decimal, allow_zero: bool) -> GenericResult<u32> {
    Ok(decimal_quantity.to_u32().and_then(|quantity| {
        if Decimal::from_u32(quantity).unwrap() != decimal_quantity {
            return None;
        }

        if !allow_zero && quantity == 0 {
            return None;
        }

        Some(quantity)
    }).ok_or_else(|| format!("Invalid quantity: {}", decimal_quantity))?)
}

#[derive(Debug)]
pub enum CashFlowType {
    DepositOrWithdrawal,
    Commission,
    Fee(String),
}

impl CashFlowType {
    pub fn parse(description: &str) -> GenericResult<CashFlowType> {
        lazy_static! {
            static ref SPACES_REGEX: Regex = Regex::new(r"\s{2,}").unwrap();
        }
        let description = SPACES_REGEX.replace_all(description, " ");

        for &commission_description in &[
            "Вознаграждение брокера за заключение сделок Конвертации ДС",
            r#"Комиссия Брокера / Доп. комиссия Брокера "Сборы ТС" за заключение сделок"#,
        ] {
            if description.starts_with(commission_description) {
                return Ok(CashFlowType::Commission);
            }
        }

        for &deposit_or_withdrawal_description in &[
            "Перевод денежных средств",
            "Списаны средства клиента",
            "Поставлены на торги средства клиента",
        ] {
            if description.starts_with(deposit_or_withdrawal_description) {
                return Ok(CashFlowType::DepositOrWithdrawal);
            }
        }

        for &fee_description in &[
            "Комиссия за ведение учета ЦБ",
            "Ежегодная комиссия за ведение учета ЦБ",
            "Комиссия за предоставление информации Брокером по ЦБ",
            "Минимальная комиссия Брокера за обработку поручений и предоставление информации",
            "Вознаграждение Брокера за обработку заявления на вывод безналичных денежных средств",
            "Вознаграждение Брокера за предоставление информации по движению и учету ценных бумаг",
        ] {
            if description.starts_with(fee_description) {
                return Ok(CashFlowType::Fee(fee_description.to_owned()))
            }
        }

        return Err!("Unable to determine cash flow type by its description: {:?}", description);
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use super::*;

    #[test]
    fn date_parsing() {
        assert_eq!(parse_date("2017-12-31T00:00:00").unwrap(), date!(31, 12, 2017));
    }

    #[test]
    fn security_description_parsing() {
        assert_eq!(parse_security_description(
            "FinEx MSCI China UCITS ETF (USD Share Class) п/у FinEx Investment Management LLP"),
            "FinEx MSCI China UCITS ETF");
    }

    #[rstest(description => [
        "Списаны средства клиента 123456 для возврата на расчетный счет",
        "Поставлены на торги средства клиента  123456i; п/п 17021 от 07.12.2017",
        "Перевод  денежных средств с клиента 123456 портфель ВР МБ на клиента 123456 портфель ФР МБ",
    ])]
    fn deposit_or_withdrawal_description_parsing(description: &str) {
        assert_matches!(
            CashFlowType::parse(description).unwrap(),
            CashFlowType::DepositOrWithdrawal
        );
    }

    #[rstest(description => [
        "Вознаграждение брокера за заключение сделок Конвертации ДС 11.05.2018 на ФР МБ по счету 123456",
        r#"Комиссия Брокера / Доп. комиссия Брокера "Сборы ТС" за заключение сделок 12.12.2017 на Фондовый Рынок Московской биржи по счету 123456i"#
    ])]
    fn commission_description_parsing(description: &str) {
        assert_matches!(
            CashFlowType::parse(description).unwrap(),
            CashFlowType::Commission
        );
    }

    #[rstest(description, expected,
        case("Комиссия за ведение учета ЦБ в НКО АО НРД за февраль, 2018 г.",
             "Комиссия за ведение учета ЦБ"),
        case("Ежегодная комиссия за ведение учета ЦБ в НКО АО НРД за 2017 г.",
             "Ежегодная комиссия за ведение учета ЦБ"),
        case("Комиссия за предоставление информации Брокером по ЦБ по месту хранения НКО АО НРД за апрель, 2018 г.",
             "Комиссия за предоставление информации Брокером по ЦБ"),
        case("Минимальная комиссия Брокера за обработку поручений и предоставление информации за июль 2017 клиент 123456",
             "Минимальная комиссия Брокера за обработку поручений и предоставление информации"),
        case("Вознаграждение Брокера за обработку заявления на вывод безналичных денежных средств клиента 141106",
             "Вознаграждение Брокера за обработку заявления на вывод безналичных денежных средств"),
        case("Вознаграждение Брокера за предоставление информации по движению и учету ценных бумаг/ИФИ в портфеле Фондовый Рынок Московской биржи за январь 2020",
             "Вознаграждение Брокера за предоставление информации по движению и учету ценных бумаг"),
    )]
    fn fee_description_parsing(description: &str, expected: &str) {
        assert_matches!(
            CashFlowType::parse(description).unwrap(),
            CashFlowType::Fee(description) if description == expected
        );
    }
}