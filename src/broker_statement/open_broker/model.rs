use chrono::Duration;

use broker_statement::BrokerStatementBuilder;
use core::EmptyResult;
use currency::Cash;
use types::Date;

use super::parsers::deserialize_date;

#[derive(Deserialize)]
pub struct BrokerReport {
    #[serde(deserialize_with = "deserialize_date")]
    date_from: Date,
    #[serde(deserialize_with = "deserialize_date")]
    date_to: Date,

    #[serde(rename = "spot_account_totally")]
    account_summary: AccountSummary,
}

impl BrokerReport {
    pub fn parse(&self, statement: &mut BrokerStatementBuilder) -> EmptyResult {
        statement.period = Some((self.date_from, self.date_to + Duration::days(1)));
        self.account_summary.parse(statement)?;
        Ok(())
    }
}

#[derive(Deserialize)]
struct AccountSummary {
    #[serde(rename = "item")]
    items: Vec<AccountSummaryItem>,
}

impl AccountSummary {
    fn parse(&self, statement: &mut BrokerStatementBuilder) -> EmptyResult {
        for item in &self.items {
            let amount = Cash::new_from_string(&item.currency, &item.amount)?;

            if item.name == "Входящий остаток (факт)" {
                statement.set_starting_value(amount)?;
            } else if item.name == "Исходящий остаток (факт)" {
                statement.cash_assets.deposit(amount);
            }
        }

        Ok(())
    }
}

#[derive(Deserialize)]
struct AccountSummaryItem {
    #[serde(rename = "row_name")]
    name: String,
    #[serde(rename = "value")]
    amount: String,
    currency: String,
}
