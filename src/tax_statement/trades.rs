use std::collections::HashMap;
use std::ops::AddAssign;

use chrono::Datelike;
use static_table_derive::StaticTable;

use crate::broker_statement::{BrokerStatement, StockSell, SellDetails, FifoDetails};
use crate::config::PortfolioConfig;
use crate::core::EmptyResult;
use crate::currency::Cash;
use crate::currency::converter::CurrencyConverter;
use crate::formatting::{self, table::Cell};
use crate::localities::Country;
use crate::taxes::TaxPaymentDay;
use crate::types::{Date, Decimal};

use super::statement::TaxStatement;

pub fn process_income(
    country: &Country, portfolio: &PortfolioConfig, broker_statement: &BrokerStatement,
    year: Option<i32>, mut tax_statement: Option<&mut TaxStatement>, converter: &CurrencyConverter,
) -> EmptyResult {
    let mut processor = TradesProcessor {
        portfolio,
        broker_statement,
        year,

        country,
        converter,

        trades_table: TradesTable::new(),
        fifo_table: FifoTable::new(),

        same_dates: true,
        same_currency: true,
        stock_splits: false,
        total_local_profit: HashMap::new(),
    };

    let mut trade_id = 0;

    for trade in &broker_statement.stock_sells {
        let (tax_year, _) = portfolio.tax_payment_day.get(trade.execution_date, true);

        if let Some(year) = year {
            if tax_year != year {
                continue;
            }
        }

        let details = trade.calculate(&country, tax_year, converter)?;
        processor.process_trade(trade_id, trade, &details)?;

        if let Some(ref mut tax_statement) = tax_statement {
            processor.add_income(tax_statement, trade, &details)?;
        }

        trade_id += 1;
    }

    if trade_id != 0 {
        processor.print();
    }

    Ok(())
}

struct TradesProcessor<'a> {
    portfolio: &'a PortfolioConfig,
    broker_statement: &'a BrokerStatement,
    year: Option<i32>,

    country: &'a Country,
    converter: &'a CurrencyConverter,

    trades_table: TradesTable,
    fifo_table: FifoTable,

    same_dates: bool,
    same_currency: bool,
    stock_splits: bool,
    total_local_profit: HashMap<i32, Decimal>,
}

#[derive(StaticTable)]
#[table(name="TradesTable")]
struct TradeRow {
    #[column(name="№")]
    id: usize,
    #[column(name="Дата сделки")]
    conclusion_date: Date,
    #[column(name="Дата расчета")]
    execution_date: Date,
    #[column(name="Ценная бумага")]
    security: String,
    #[column(name="Кол.")]
    quantity: Decimal,
    #[column(name="Цена")]
    price: Cash,
    #[column(name="Курс руб.\nдата сделки")]
    conclusion_currency_rate: Decimal,
    #[column(name="Курс руб.\nдата расчета")]
    execution_currency_rate: Decimal,
    #[column(name="Доход от\nпродажи")]
    revenue: Cash,
    #[column(name="Доход от\nпродажи (руб)")]
    local_revenue: Cash,
    #[column(name="Комиссия")]
    commission: Cash,
    #[column(name="Комиссия\n(руб)")]
    local_commission: Cash,
    #[column(name="Затраты на\nпокупку")]
    purchase_local_cost: Cash,
    #[column(name="Общие\nзатраты")]
    total_local_cost: Cash,
    #[column(name="Прибыль")]
    local_profit: Cash,
    #[column(name="Налог")]
    tax_to_pay: Cash,
    #[column(name="Реальный\nдоход")]
    real_profit_ratio: Cell,
    #[column(name="Реальный\nдоход (руб)")]
    real_local_profit_ratio: Cell,
}

#[derive(StaticTable)]
#[table(name="FifoTable")]
struct FifoRow {
    #[column(name="№")]
    id: Option<usize>,
    #[column(name="Дата сделки")]
    conclusion_date: Date,
    #[column(name="Дата расчета")]
    execution_date: Date,
    #[column(name="Ценная бумага")]
    security: String,
    #[column(name="Кол.")]
    quantity: Decimal,
    #[column(name="Мул.")]
    multiplier: Decimal,
    #[column(name="Цена")]
    price: Cash,
    #[column(name="Курс руб.\nдата сделки")]
    conclusion_currency_rate: Decimal,
    #[column(name="Курс руб.\nдата расчета")]
    execution_currency_rate: Decimal,
    #[column(name="Расходы")]
    cost: Cash,
    #[column(name="Расходы (руб)")]
    local_cost: Cash,
    #[column(name="Комиссия")]
    commission: Cash,
    #[column(name="Комиссия\n(руб)")]
    local_commission: Cash,
    #[column(name="Общие затраты")]
    total_local_cost: Cash,
}

impl<'a> TradesProcessor<'a> {
    fn add_income(&self, tax_statement: &mut TaxStatement, trade: &StockSell, details: &SellDetails) -> EmptyResult {
        let name = self.broker_statement.get_instrument_name(&trade.symbol);
        let description = format!("{}: Продажа {}", self.broker_statement.broker.name, name);

        let precise_currency_rate = self.converter.precise_currency_rate(
            trade.execution_date, details.revenue.currency, self.country.currency)?;

        tax_statement.add_stock_income(
            &description, trade.execution_date, details.revenue.currency, precise_currency_rate,
            details.revenue.amount, details.local_revenue.amount,
            details.total_local_cost.amount
        ).map_err(|e| format!(
            "Unable to add income from selling {} on {} to the tax statement: {}",
            trade.symbol, formatting::format_date(trade.execution_date), e
        ))?;

        Ok(())
    }

    fn process_trade(&mut self, trade_id: usize, trade: &StockSell, details: &SellDetails) -> EmptyResult {
        let security = self.broker_statement.get_instrument_name(&trade.symbol);

        self.same_dates &= trade.execution_date == trade.conclusion_date;
        self.same_currency &= trade.price.currency == self.country.currency &&
            trade.commission.currency == self.country.currency;

        self.total_local_profit.entry(trade.execution_date.year()).or_default()
            .add_assign(details.local_profit.amount);

        let conclusion_currency_rate = self.converter.precise_currency_rate(
            trade.conclusion_date, trade.commission.currency, self.country.currency)?;

        let execution_currency_rate = self.converter.precise_currency_rate(
            trade.execution_date, trade.price.currency, self.country.currency)?;

        self.trades_table.add_row(TradeRow {
            id: trade_id,
            conclusion_date: trade.conclusion_date,
            execution_date: trade.execution_date,
            security: security.to_owned(),
            quantity: trade.quantity,
            price: trade.price,
            conclusion_currency_rate: conclusion_currency_rate,
            execution_currency_rate: execution_currency_rate,
            revenue: details.revenue,
            local_revenue: details.local_revenue,
            commission: trade.commission.round(),
            local_commission: details.local_commission,
            purchase_local_cost: details.purchase_local_cost,
            total_local_cost: details.total_local_cost,
            local_profit: details.local_profit,
            tax_to_pay: details.tax_to_pay,
            real_profit_ratio: Cell::new_ratio(details.real_profit_ratio),
            real_local_profit_ratio: Cell::new_ratio(details.real_local_profit_ratio),
        });

        for (index, buy_trade) in details.fifo.iter().enumerate() {
            self.process_fifo(&security, trade_id, buy_trade, index == 0)?;
        }

        Ok(())
    }

    fn process_fifo(&mut self, security: &str, trade_id: usize, buy_trade: &FifoDetails, first: bool) -> EmptyResult {
        self.same_dates &= buy_trade.execution_date == buy_trade.conclusion_date;
        self.same_currency &= buy_trade.price.currency == self.country.currency &&
            buy_trade.commission.currency == self.country.currency;
        self.stock_splits |= buy_trade.multiplier != dec!(1);

        let conclusion_currency_rate = self.converter.precise_currency_rate(
            buy_trade.conclusion_date, buy_trade.commission.currency, self.country.currency)?;

        let execution_currency_rate = self.converter.precise_currency_rate(
            buy_trade.execution_date, buy_trade.price.currency, self.country.currency)?;

        self.fifo_table.add_row(FifoRow {
            id: if first {
                Some(trade_id)
            } else {
                None
            },
            conclusion_date: buy_trade.conclusion_date,
            execution_date: buy_trade.execution_date,
            security: security.to_owned(),
            quantity: buy_trade.quantity,
            multiplier: buy_trade.multiplier,
            price: buy_trade.price,
            conclusion_currency_rate: conclusion_currency_rate,
            execution_currency_rate: execution_currency_rate,
            cost: buy_trade.cost,
            local_cost: buy_trade.local_cost,
            commission: buy_trade.commission,
            local_commission: buy_trade.local_commission,
            total_local_cost: buy_trade.total_local_cost,
        });

        Ok(())
    }

    fn print(mut self) {
        if self.same_dates {
            self.trades_table.hide_execution_date();
            self.trades_table.rename_conclusion_currency_rate("Курс руб.");
            self.trades_table.hide_execution_currency_rate();

            self.fifo_table.hide_execution_date();
            self.fifo_table.rename_conclusion_currency_rate("Курс руб.");
            self.fifo_table.hide_execution_currency_rate();
        }

        if self.same_currency {
            self.trades_table.hide_conclusion_currency_rate();
            self.trades_table.hide_execution_currency_rate();
            self.trades_table.hide_local_commission();
            self.trades_table.hide_local_revenue();
            self.trades_table.hide_real_local_profit_ratio();

            self.fifo_table.hide_conclusion_currency_rate();
            self.fifo_table.hide_execution_currency_rate();
            self.fifo_table.hide_local_cost();
            self.fifo_table.hide_local_commission();
        }

        if !self.stock_splits {
            self.fifo_table.hide_multiplier()
        }

        let total_local_profit = self.total_local_profit.values().copied().sum();
        let total_tax_to_pay = match self.portfolio.tax_payment_day {
            TaxPaymentDay::Day {..} => Some(self.total_local_profit.iter().map(|(&year, &profit)| {
                self.country.tax_to_pay(year, profit, None)
            }).sum()),

            TaxPaymentDay::OnClose(date) => if self.year.is_none() {
                Some(self.country.tax_to_pay(date.year(), total_local_profit, None))
            } else {
                None
            },
        };

        let mut totals = self.trades_table.add_empty_row();
        totals.set_local_profit(Cash::new(self.country.currency, total_local_profit));

        if let Some(total_tax_to_pay) = total_tax_to_pay {
            totals.set_tax_to_pay(Cash::new(self.country.currency, total_tax_to_pay));
        }

        self.trades_table.print(&format!(
            "Расчет прибыли от продажи ценных бумаг, полученной через {}",
            self.broker_statement.broker.name));

        self.fifo_table.print("Детализация расчета сделок по ФИФО");
    }
}