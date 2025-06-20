use chrono::Datelike;
use lazy_static::lazy_static;
use regex::Regex;
use serde::Deserialize;

use crate::broker_statement::fees::Fee;
use crate::broker_statement::open::common::deserialize_date;
use crate::broker_statement::partial::PartialBrokerStatement;
use crate::broker_statement::Withholding;
use crate::core::{EmptyResult, GenericResult};
use crate::currency::CashAssets;
use crate::instruments::InstrumentId;
use crate::types::{Date, Decimal};
use crate::util::{self, DecimalRestrictions};

#[derive(Deserialize)]
pub struct CashFlows {
    #[serde(rename = "item")]
    cash_flows: Vec<CashFlow>,
}

impl CashFlows {
    pub fn parse(&self, statement: &mut PartialBrokerStatement) -> EmptyResult {
        for cash_flow in &self.cash_flows {
            cash_flow.parse(statement)?;
        }
        Ok(())
    }
}

#[derive(Deserialize)]
struct CashFlow {
    #[serde(rename = "@operation_date", deserialize_with = "deserialize_date")]
    date: Date,

    #[serde(rename = "@currency_code")]
    currency: String,

    #[serde(rename = "@amount")]
    amount: Decimal,

    #[serde(rename = "@comment")]
    description: String,
}

impl CashFlow {
    fn parse(&self, statement: &mut PartialBrokerStatement) -> EmptyResult {
        let date = self.date;
        let currency = &self.currency;
        let amount = self.amount;

        match CashFlowType::parse(&self.description)? {
            CashFlowType::DepositOrWithdrawal => {
                let amount = util::validate_named_cash(
                    "deposit or withdrawal amount", currency, amount,
                    DecimalRestrictions::NonZero)?;
                statement.deposits_and_withdrawals.push(CashAssets::new_from_cash(date, amount));
            },

            CashFlowType::Commission => {
                // It's taken into account during trades processing
            },

            CashFlowType::Fee(description) => {
                let amount = -util::validate_named_cash(
                    "fee amount", currency, amount, DecimalRestrictions::NonZero)?;
                statement.fees.push(Fee::new(date, Withholding::new(amount), Some(description)));
            },

            CashFlowType::TaxAgent => {
                let amount = -util::validate_named_cash(
                    "tax amount", currency, amount, DecimalRestrictions::StrictlyNegative)?;
                statement.tax_agent_withholdings.add(date, date.year(), Withholding::Withholding(amount))?;
            },

            CashFlowType::Dividend(issuer) => {
                let issuer_id = InstrumentId::InternalId(issuer);
                let amount = util::validate_named_cash(
                    "dividend amount", currency, amount, DecimalRestrictions::StrictlyPositive)?;
                statement.dividend_accruals(date, issuer_id, true).add(date, amount);
            },

            CashFlowType::DividendTax(issuer) => {
                let issuer_id = InstrumentId::InternalId(issuer);
                let amount = -util::validate_named_cash(
                    "tax amount", currency, amount, DecimalRestrictions::StrictlyNegative)?;
                statement.tax_accruals(date, issuer_id, true).add(date, amount);
            },
        };

        Ok(())
    }
}

#[derive(Debug)]
enum CashFlowType {
    DepositOrWithdrawal,

    Commission,
    Fee(String),
    TaxAgent,

    Dividend(String),
    DividendTax(String),
}

impl CashFlowType {
    fn parse(description: &str) -> GenericResult<CashFlowType> {
        let description = util::fold_spaces(description);

        for &deposit_or_withdrawal_description in &[
            "Перевод денежных средств",
            "Списаны средства клиента",
            "Поставлены на торги средства клиента",
        ] {
            if description.starts_with(deposit_or_withdrawal_description) {
                return Ok(CashFlowType::DepositOrWithdrawal);
            }
        }

        for &commission_description in &[
            r#"Комиссия Брокера / Доп. комиссия Брокера "Сборы ТС" за заключение сделок"#,
            "Вознаграждение брокера за заключение сделок Конвертации ДС",
            "Комиссия Брокера за заключение Специальных сделок РЕПО",
        ] {
            if description.starts_with(commission_description) {
                return Ok(CashFlowType::Commission);
            }
        }

        for &fee_description in &[
            "Комиссия за ведение учета ЦБ",
            "Возмещение за депозитарные услуги",
            "Ежегодная комиссия за ведение учета ЦБ",
            "Проценты за предоставление займа ДС Брокером",
            "Комиссия за предоставление информации Брокером по ЦБ",
            "Минимальная комиссия Брокера за обработку поручений и предоставление информации",
            "Вознаграждение Брокера за обработку заявления на вывод безналичных денежных средств",
            "Вознаграждение Брокера за предоставление информации по движению и учету ценных бумаг",
            "Комиссия Брокера за предоставление информации по услуге риск-поддержке открытой позиции",

            "Возврат излишне удержанной комиссии брокера",
        ] {
            if description.starts_with(fee_description) {
                return Ok(CashFlowType::Fee(fee_description.to_owned()));
            }
        }

        if description.starts_with("Удержан налог на доход с клиента") {
            return Ok(CashFlowType::TaxAgent);
        }

        lazy_static! {
            static ref DIVIDEND_REGEXES: Vec<Regex> = [
                r"^Выплата дохода клиент [^ ]+ дивиденды (?P<issuer>[^,]+), комиссия платежного агента",
                r"^Выплата дохода клиент [^ ]+ дивиденды (?P<issuer>[^,]+) налог к удержанию",
                r"^Выплата дохода клиент [^ ]+ дивиденды (?P<issuer>[^,]+)$"
            ].iter().map(|regex| Regex::new(regex).unwrap()).collect();
        }

        for regex in DIVIDEND_REGEXES.iter() {
            if let Some(captures) = regex.captures(&description) {
                let issuer = captures.name("issuer").unwrap().as_str().to_owned();
                return Ok(CashFlowType::Dividend(issuer));
            }
        }

        lazy_static! {
            static ref DIVIDEND_TAX_REGEX: Regex = Regex::new(
                r"^Удержан налог на доход по дивидендам (?P<issuer>.+) с клиента").unwrap();
        }

        if let Some(captures) = DIVIDEND_TAX_REGEX.captures(&description) {
            let issuer = captures.name("issuer").unwrap().as_str().to_owned();
            return Ok(CashFlowType::DividendTax(issuer));
        }

        Err!("Unable to determine cash flow type by its description: {:?}", description)
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
        "Комиссия Брокера за заключение Специальных сделок РЕПО 18.01.2022 на Фондовый Рынок Московской биржи по счету 123456",
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
        case("Проценты за предоставление займа ДС Брокером на ФР МБ, остаток в рублях -43.57",
             "Проценты за предоставление займа ДС Брокером"),
        case("Комиссия за предоставление информации Брокером по ЦБ по месту хранения НКО АО НРД за апрель, 2018 г.",
             "Комиссия за предоставление информации Брокером по ЦБ"),
        case("Минимальная комиссия Брокера за обработку поручений и предоставление информации за июль 2017 клиент 123456",
             "Минимальная комиссия Брокера за обработку поручений и предоставление информации"),
        case("Вознаграждение Брокера за обработку заявления на вывод безналичных денежных средств клиента 141106",
             "Вознаграждение Брокера за обработку заявления на вывод безналичных денежных средств"),
        case("Вознаграждение Брокера за предоставление информации по движению и учету ценных бумаг/ИФИ в портфеле Фондовый Рынок Московской биржи за январь 2020",
             "Вознаграждение Брокера за предоставление информации по движению и учету ценных бумаг"),
        case("Возмещение за депозитарные услуги счет услуги вышестоящего Депозитария за хранение ЦБ наим. ЦБ:GLOBALTRANS-GDR, ISIN US37949E2046 от 05.11.2021 НКО АО НРД ",
             "Возмещение за депозитарные услуги"),
        case("Комиссия Брокера за предоставление информации по услуге риск-поддержке открытой позиции, рублевая оценка обеспечения 151891.47, Требуемое ГО 2534560.63 за 28.02.2022",
             "Комиссия Брокера за предоставление информации по услуге риск-поддержке открытой позиции"),

        case("Возврат излишне удержанной комиссии брокера за предоставление информации по услуге риск-поддержке открытой позиции за 28.02.2022",
             "Возврат излишне удержанной комиссии брокера"),
    )]
    fn fee_description_parsing(description: &str, expected: &str) {
        assert_matches!(
            CashFlowType::parse(description).unwrap(),
            CashFlowType::Fee(description) if description == expected
        );
    }

    #[test]
    fn tax_agent_description_parsing() {
        assert_matches!(
            CashFlowType::parse("Удержан налог на доход с клиента 123456").unwrap(),
            CashFlowType::TaxAgent
        );
    }

    #[rstest(description, expected,
        case("Выплата дохода клиент 123456 дивиденды ROS AGRO PLC-GDR, комиссия платежного агента 2.01 долларов",
             "ROS AGRO PLC-GDR"),
        case("Выплата дохода клиент 123456 дивиденды GLOBALTRANS-GDR, комиссия платежного агента 0.10 долларов",
             "GLOBALTRANS-GDR"),
        case("Выплата дохода клиент 123456 дивиденды ГАЗПРОМ-ао-2 налог к удержанию 11.00 рублей",
             "ГАЗПРОМ-ао-2"),
        case("Выплата дохода клиент 123456 дивиденды Татнфт 3ап налог к удержанию 13.00 рублей",
             "Татнфт 3ап"),
        case("Выплата дохода клиент 123456 дивиденды ROS AGRO PLC-GDR",
             "ROS AGRO PLC-GDR"),
    )]
    fn dividend_description_parsing(description: &str, expected: &str) {
        assert_matches!(
            CashFlowType::parse(description).unwrap(),
            CashFlowType::Dividend(issuer) if issuer == expected
        );
    }

    #[rstest(description, expected,
        case("Удержан налог на доход  по дивидендам Татнфт 3ап с клиента 123456",
             "Татнфт 3ап"),
        case("Удержан налог на доход  по дивидендам ГАЗПРОМ-ао-2 с клиента 123456",
             "ГАЗПРОМ-ао-2"),
    )]
    fn dividend_tax_description_parsing(description: &str, expected: &str) {
        assert_matches!(
            CashFlowType::parse(description).unwrap(),
            CashFlowType::DividendTax(issuer) if issuer == expected
        );
    }
}