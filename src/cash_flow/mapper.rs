use num_traits::Zero;

use crate::broker_statement::{
    BrokerStatement, ForexTrade, StockBuy, StockBuyType, StockSell, StockSellType, Dividend, Fee,
    IdleCashInterest, TaxWithholding};
use crate::currency::{Cash, CashAssets};
use crate::types::Date;

pub struct CashFlow {
    pub date: Date,
    pub amount: Cash,
    pub sibling_amount: Option<Cash>,
    pub description: String,
}

pub fn map_broker_statement_to_cash_flow(statement: &BrokerStatement) -> Vec<CashFlow> {
    CashFlowMapper{cash_flows: Vec::new()}.process(statement)
}

struct CashFlowMapper {
    cash_flows: Vec<CashFlow>,
}

impl CashFlowMapper {
    fn process(mut self, statement: &BrokerStatement) -> Vec<CashFlow> {
        for deposit in statement.cash_flows.iter().filter(|cash_flow|
            !cash_flow.cash.is_negative()
        ) {
            self.deposit_or_withdrawal(deposit)
        }

        for interest in &statement.idle_cash_interest {
            self.interest(interest);
        }

        for dividend in &statement.dividends {
            self.dividend(&statement.get_instrument_name(&dividend.issuer), dividend);
        }

        for trade in &statement.forex_trades {
            self.forex_trade(trade);
        }

        for trade in &statement.stock_sells {
            self.stock_sell(&statement.get_instrument_name(&trade.symbol), trade);
        }

        for trade in &statement.stock_buys {
            self.stock_buy(&statement.get_instrument_name(&trade.symbol), trade);
        }

        for fee in &statement.fees {
            self.fee(fee);
        }

        for withdrawal in statement.cash_flows.iter().filter(|cash_flow|
            cash_flow.cash.is_negative()
        ) {
            self.deposit_or_withdrawal(withdrawal)
        }

        for withholding in &statement.tax_agent_withholdings {
            self.tax_agent_withholding(withholding);
        }

        self.cash_flows.sort_by_key(|cash_flow| cash_flow.date);
        self.cash_flows
    }

    fn fee(&mut self, fee: &Fee) {
        self.add_static(fee.date, -fee.amount, fee.local_description());
    }

    fn deposit_or_withdrawal(&mut self, assets: &CashAssets) {
        self.add_static(assets.date, assets.cash, if assets.cash.is_positive() {
            "Ввод денежных средств"
        } else {
            "Вывод денежных средств"
        });
    }

    fn interest(&mut self, interest: &IdleCashInterest) {
        self.add_static(interest.date, interest.amount, "Проценты на остаток по счету");
    }

    fn forex_trade(&mut self, trade: &ForexTrade) {
        let description = format!("Конвертация {} -> {}", trade.from, trade.to);
        let cash_flow = self.add(trade.conclusion_date, -trade.from, description);
        cash_flow.sibling_amount.replace(trade.to);

        if !trade.commission.is_zero() {
            let description = format!("Комиссия за конвертацию {} -> {}", trade.from, trade.to);
            self.add(trade.conclusion_date, -trade.commission, description);
        };
    }

    fn stock_buy(&mut self, name: &str, trade: &StockBuy) {
        match trade.type_ {
            StockBuyType::Trade => {
                let description = format!("Покупка {} {}", trade.quantity, name);
                self.add(trade.conclusion_date, -trade.volume, description);

                if !trade.commission.is_zero() {
                    let description = format!("Комиссия за покупку {} {}", trade.quantity, name);
                    self.add(trade.conclusion_date, -trade.commission, description);
                };
            },
            StockBuyType::CorporateAction => {},
        };
    }

    fn stock_sell(&mut self, name: &str, trade: &StockSell) {
        match trade.type_ {
            StockSellType::Trade => {
                let description = format!("Продажа {} {}", trade.quantity, name);
                self.add(trade.conclusion_date, trade.volume, description);

                if !trade.commission.is_zero() {
                    let description = format!("Комиссия за продажу {} {}", trade.quantity, name);
                    self.add(trade.conclusion_date, -trade.commission, description);
                };
            },
            StockSellType::CorporateAction => {},
        }
    }

    fn dividend(&mut self, name: &str, dividend: &Dividend) {
        let description = format!("Дивиденд от {}", name);
        self.add(dividend.date, dividend.amount, description);

        if !dividend.paid_tax.is_zero() {
            let description = format!("Налог, удержанный с дивиденда от {}", name);
            self.add(dividend.date, -dividend.paid_tax, description);
        };
    }

    fn tax_agent_withholding(&mut self, withholding: &TaxWithholding) {
        let description = format!("Удержание налога за {} год", withholding.year);
        self.add(withholding.date, -withholding.amount, description);
    }

    fn add_static(&mut self, date: Date, amount: Cash, description: &str) -> &mut CashFlow {
        self.add(date, amount, description.to_owned())
    }

    fn add(&mut self, date: Date, amount: Cash, description: String) -> &mut CashFlow {
        self.cash_flows.push(CashFlow{date, amount, sibling_amount: None, description});
        self.cash_flows.last_mut().unwrap()
    }
}