use crate::currency::{Cash, CashAssets};
use crate::types::Date;

pub trait CashFlow {
    fn date(&self) -> Date;
    fn amount(&self) -> Cash;
    fn description(&self) -> &str;
}

impl CashFlow for CashAssets {
    fn date(&self) -> Date {
        return self.date
    }

    fn amount(&self) -> Cash {
        return self.cash
    }

    fn description(&self) -> &str {
        if self.cash.is_positive() {
            "Ввод денежных средств"
        } else {
            "Вывод денежных средств"
        }
    }
}