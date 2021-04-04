use serde::Deserialize;

use crate::broker_statement::fees::Fee;
use crate::broker_statement::partial::PartialBrokerStatement;
use crate::core::{EmptyResult, GenericResult};
use crate::currency::{Cash, CashAssets};
use crate::types::{Date, Decimal};
use crate::util::{self, DecimalRestrictions};

use super::common::deserialize_date;

#[derive(Deserialize)]
pub struct CashFlows {
    #[serde(rename = "item")]
    cash_flows: Vec<CashFlow>,
}

#[derive(Deserialize)]
struct CashFlow {
    #[serde(rename = "operation_date", deserialize_with = "deserialize_date")]
    date: Date,

    #[serde(rename = "currency_code")]
    currency: String,

    amount: Decimal,

    #[serde(rename = "comment")]
    description: String,
}

impl CashFlows {
    pub fn parse(&self, statement: &mut PartialBrokerStatement) -> EmptyResult {
        for cash_flow in &self.cash_flows {
            let date = cash_flow.date;
            let currency = &cash_flow.currency;
            let amount = cash_flow.amount;

            match CashFlowType::parse(&cash_flow.description)? {
                CashFlowType::DepositOrWithdrawal => {
                    let amount = util::validate_named_decimal(
                        "deposit or withdrawal amount", amount,
                        DecimalRestrictions::NonZero)?;

                    statement.cash_flows.push(CashAssets::new_from_cash(
                        date, Cash::new(currency, amount)));
                },

                CashFlowType::Commission => {
                    // It's taken into account during trades processing
                },

                CashFlowType::Fee(description) => {
                    let amount = -util::validate_named_cash(
                        "fee amount", currency, amount, DecimalRestrictions::StrictlyNegative)?;
                    statement.fees.push(Fee::new(date, amount, Some(description)));
                },
            };
        }

        Ok(())
    }
}

#[derive(Debug)]
enum CashFlowType {
    DepositOrWithdrawal,
    Commission,
    Fee(String),
}

impl CashFlowType {
    fn parse(description: &str) -> GenericResult<CashFlowType> {
        let description = util::fold_spaces(description);

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
    use matches::assert_matches;
    use rstest::rstest;
    use super::*;

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