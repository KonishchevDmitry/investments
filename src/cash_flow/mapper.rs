use std::cmp::Ordering;

use crate::broker_statement::{
    BrokerStatement, ForexTrade, StockBuy, StockSource, StockSell, StockSellType, Dividend, Fee,
    IdleCashInterest, TaxWithholding, CashFlow as CashFlowDetails, CashFlowType};
use crate::currency::{Cash, CashAssets};
use crate::formatting;
use crate::time::DateOptTime;

pub struct CashFlow {
    pub time: DateOptTime,
    pub operation: Operation,
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
        for assets in &statement.deposits_and_withdrawals {
            self.deposit_or_withdrawal(assets)
        }

        for interest in &statement.idle_cash_interest {
            self.interest(interest);
        }

        for dividend in &statement.dividends {
            self.dividend(statement, dividend);
        }

        for cash_flow in &statement.cash_flows {
            self.cash_flow(statement, cash_flow);
        }

        for trade in &statement.forex_trades {
            self.forex_trade(trade);
        }

        for trade in &statement.stock_sells {
            self.stock_sell(&statement.instrument_info.get_name(&trade.original_symbol), trade);
        }

        for trade in &statement.stock_buys {
            self.stock_buy(&statement.instrument_info.get_name(&trade.original_symbol), trade);
        }

        for fee in &statement.fees {
            self.fee(fee);
        }

        for withholding in &statement.tax_agent_withholdings {
            self.tax_agent_withholding(withholding);
        }

        self.cash_flows.sort_by(cash_flow_comparator);
        self.cash_flows
    }

    fn fee(&mut self, fee: &Fee) {
        self.add_static(fee.date.into(), Operation::Fee, -fee.amount, fee.local_description());
    }

    fn deposit_or_withdrawal(&mut self, assets: &CashAssets) {
        let (operation, description) = if assets.cash.is_positive() {
            (Operation::Deposit, "Ввод денежных средств")
        } else {
            (Operation::Withdrawal, "Вывод денежных средств")
        };
        self.add_static(assets.date.into(), operation, assets.cash, description);
    }

    fn interest(&mut self, interest: &IdleCashInterest) {
        self.add_static(
            interest.date.into(), Operation::Interest, interest.amount,
            "Проценты на остаток по счету");
    }

    fn forex_trade(&mut self, trade: &ForexTrade) {
        let description = format!("Конвертация {} -> {}", trade.from, trade.to);
        let cash_flow = self.add(trade.conclusion_time, Operation::ForexTrade, -trade.from, description);
        cash_flow.sibling_amount.replace(trade.to);

        if !trade.commission.is_zero() {
            let description = format!("Комиссия за конвертацию {} -> {}", trade.from, trade.to);
            self.add(trade.conclusion_time, Operation::Commission, -trade.commission, description);
        };
    }

    fn stock_buy(&mut self, name: &str, trade: &StockBuy) {
        match trade.type_ {
            StockSource::Trade {volume, commission, ..} => {
                let description = format!("Покупка {} {}", trade.quantity, name);
                self.add(trade.conclusion_time, Operation::BuyTrade, -volume, description);

                if !commission.is_zero() {
                    let description = format!("Комиссия за покупку {} {}", trade.quantity, name);
                    self.add(trade.conclusion_time, Operation::Commission, -commission, description);
                };
            },
            StockSource::CorporateAction => {},
        };
    }

    fn stock_sell(&mut self, name: &str, trade: &StockSell) {
        match trade.type_ {
            StockSellType::Trade {volume, commission, ..} => {
                let description = format!("Продажа {} {}", trade.quantity, name);
                self.add(trade.conclusion_time, Operation::SellTrade, volume, description);

                if !commission.is_zero() {
                    let description = format!("Комиссия за продажу {} {}", trade.quantity, name);
                    self.add(trade.conclusion_time, Operation::Commission, -commission, description);
                };
            },
            StockSellType::CorporateAction => {},
        }
    }

    fn dividend(&mut self, statement: &BrokerStatement, dividend: &Dividend) {
        if dividend.skip_from_cash_flow {
            return
        }

        let date = dividend.date.into();
        let issuer = &dividend.original_issuer;

        self.cash_flow(statement, &CashFlowDetails::new(date, dividend.amount, CashFlowType::Dividend {
            date: dividend.date,
            issuer: issuer.clone(),
        }));

        if !dividend.paid_tax.is_zero() {
            self.cash_flow(statement, &CashFlowDetails::new(date, -dividend.paid_tax, CashFlowType::Tax {
                date: dividend.date,
                issuer: issuer.clone(),
            }));
        };
    }

    fn cash_flow(&mut self, statement: &BrokerStatement, cash_flow: &CashFlowDetails) {
        let date = cash_flow.date;
        let amount = cash_flow.amount;

        match cash_flow.type_ {
            CashFlowType::Dividend {date: dividend_date, ref issuer} => {
                let mut description = statement.instrument_info.get_name(issuer);
                if date.date != dividend_date {
                    description += &format!(" от {}", formatting::format_date(dividend_date));
                };

                self.add(date, Operation::Dividend, amount, if amount.is_positive() {
                    format!("Дивиденд от {}", description)
                } else {
                    format!("Возврат дивиденда от {}", description)
                });
            },
            CashFlowType::Tax {date: dividend_date, ref issuer, ..} => {
                let mut description = statement.instrument_info.get_name(issuer);
                if date.date != dividend_date {
                    description += &format!(" от {}", formatting::format_date(dividend_date));
                };

                self.add(date, Operation::Dividend, amount, if amount.is_positive() {
                    format!("Возврат налога, удержанного с дивиденда от {}", description)
                } else {
                    format!("Налог, удержанный с дивиденда от {}", description)
                });
            },
        }
    }

    fn tax_agent_withholding(&mut self, withholding: &TaxWithholding) {
        let description = format!("Удержание налога за {} год", withholding.year);
        self.add(withholding.date.into(), Operation::Tax, -withholding.amount, description);
    }

    fn add_static(&mut self, time: DateOptTime, operation: Operation, amount: Cash, description: &str) -> &mut CashFlow {
        self.add(time, operation, amount, description.to_owned())
    }

    fn add(&mut self, time: DateOptTime, operation: Operation, amount: Cash, description: String) -> &mut CashFlow {
        self.cash_flows.push(CashFlow{time, operation, amount, sibling_amount: None, description});
        self.cash_flows.last_mut().unwrap()
    }
}

#[derive(Ord, PartialOrd, Eq, PartialEq, Clone, Copy)]
pub enum Operation {
    Deposit,
    Interest,
    Dividend,

    ForexTrade,
    SellTrade,
    BuyTrade,
    Commission,

    Fee,
    Tax,
    Withdrawal,
}

fn cash_flow_comparator(a: &CashFlow, b: &CashFlow) -> Ordering {
    if a.time.date != b.time.date || a.time.time.is_some() && b.time.time.is_some() && a.time != b.time {
        return a.time.cmp(&b.time);
    }

    for (a, b) in [(a, b), (b, a)] {
        let ops = (a.operation, b.operation);
        if let (Operation::Commission, Operation::ForexTrade | Operation::SellTrade | Operation::BuyTrade) = ops {
            return Ordering::Equal;
        }
    }

    a.operation.cmp(&b.operation)
}