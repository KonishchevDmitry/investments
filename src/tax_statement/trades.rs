use std::collections::{HashMap, VecDeque};
use std::ops::AddAssign;

use chrono::Datelike;
use log::warn;

use static_table_derive::StaticTable;

use crate::brokers::Broker;
use crate::broker_statement::{BrokerStatement, StockBuyType, StockSell, SellDetails, FifoDetails, Fee};
use crate::config::PortfolioConfig;
use crate::core::{EmptyResult, GenericResult};
use crate::currency::Cash;
use crate::currency::converter::CurrencyConverter;
use crate::formatting::{self, table::Cell};
use crate::localities::{Country, Jurisdiction};
use crate::taxes::{IncomeType, TaxPaymentDaySpec};
use crate::types::{Date, Decimal};

use super::statement::TaxStatement;

pub fn process_income(
    country: &Country, portfolio: &PortfolioConfig, broker_statement: &BrokerStatement,
    year: Option<i32>, tax_statement: Option<&mut TaxStatement>, converter: &CurrencyConverter,
) -> GenericResult<Option<Cash>> {
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
        non_trade_sources: false,
        stock_splits: false,
        tax_exemptions: false,

        total_local_profit: Cash::new(country.currency, dec!(0)),
        total_taxable_local_profit_by_year: HashMap::new(),
        total_taxable_local_profit: Cash::new(country.currency, dec!(0)),
        total_tax_deduction: Cash::new(country.currency, dec!(0)),
    };

    processor.process_trades(tax_statement)?;

    let total_tax_to_pay = processor.process_totals();
    processor.print(total_tax_to_pay);

    Ok(total_tax_to_pay)
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
    non_trade_sources: bool,
    stock_splits: bool,
    tax_exemptions: bool,

    total_local_profit: Cash,
    total_taxable_local_profit_by_year: HashMap<i32, Cash>,
    total_taxable_local_profit: Cash,
    total_tax_deduction: Cash,
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
    conclusion_currency_rate: Option<Decimal>,
    #[column(name="Курс руб.\nдата расчета")]
    execution_currency_rate: Option<Decimal>,
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
    #[column(name="Налогообл.\nприбыль")]
    taxable_local_profit: Cash,
    #[column(name="Налог")]
    tax_to_pay: Cash,
    #[column(name="Вычет")]
    tax_deduction: Cash,
    #[column(name="Реальный\nдоход")]
    real_profit_ratio: Option<Cell>,
    #[column(name="Реальный\nдоход (руб)")]
    real_local_profit_ratio: Option<Cell>,
}

#[derive(StaticTable)]
#[table(name="FifoTable")]
struct FifoRow {
    #[column(name="№")]
    id: Option<usize>,
    #[column(name="Дата сделки")]
    conclusion_date: Date,
    #[column(name="Дата расчета")]
    execution_date: Option<Date>,
    #[column(name="Ценная бумага")]
    security: String,
    #[column(name="Кол.")]
    quantity: Decimal,
    #[column(name="Мул.")]
    multiplier: Decimal,
    #[column(name="Цена")]
    price: Cash,
    #[column(name="Курс руб.\nдата сделки")]
    conclusion_currency_rate: Option<Decimal>,
    #[column(name="Курс руб.\nдата расчета")]
    execution_currency_rate: Option<Decimal>,
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
    #[column(name="Источник", align="center")]
    source: String,
    #[column(name="Льгота", align="center")]
    tax_free: Option<String>,
}

impl<'a> TradesProcessor<'a> {
    fn pre_process_fees(&mut self) -> GenericResult<(VecDeque<PreprocessedFee>, HashMap<i32, Decimal>)> {
        let broker = self.broker_statement.broker.type_;

        let mut fees = VecDeque::new();
        let mut fees_by_year = HashMap::<i32, Decimal>::new();

        let mut index = 0;
        let count = self.broker_statement.fees.len();

        while index < count {
            let fee = &self.broker_statement.fees[index];
            index += 1;

            if broker == Broker::InteractiveBrokers {
                // IB generates a lot of fee + reversal pairs for Snapshot Quotes, so detect them
                // here and skip to make the statement less noisy.

                if index < count && fee.amount.is_positive() {
                    let next_fee = &self.broker_statement.fees[index];
                    if next_fee.date == fee.date && next_fee.amount == -fee.amount {
                        index += 1;
                        continue;
                    }
                }
            }

            if let Some(year) = self.year {
                let (tax_year, _) = self.portfolio.tax_payment_day().get(fee.date, true);
                if tax_year != year {
                    continue;
                }
            }

            let fee = self.pre_process_fee(fee)?;
            fees_by_year.entry(fee.date.year()).or_default()
                .add_assign(fee.local_amount.amount);
            fees.push_back(fee);
        }

        Ok((fees, fees_by_year))
    }

    fn pre_process_fee(&mut self, fee: &Fee) -> GenericResult<PreprocessedFee> {
        self.same_currency &= fee.amount.currency == self.country.currency;

        let amount = fee.amount.round();
        let local_amount = self.converter.convert_to_cash_rounding(
            fee.date, fee.amount, self.country.currency)?;

        self.total_local_profit.sub_assign(local_amount).unwrap();
        self.total_taxable_local_profit.sub_assign(local_amount).unwrap();
        self.total_taxable_local_profit_by_year.entry(fee.date.year())
            .and_modify(|total| total.sub_assign(local_amount).unwrap())
            .or_insert(-local_amount);

        Ok(PreprocessedFee {
            date: fee.date,
            amount, local_amount,
            description: fee.local_description().to_owned(),
        })
    }

    fn post_process_fee(&mut self, fee: PreprocessedFee) {
        let mut row = self.trades_table.add_empty_row();
        row.set_conclusion_date(fee.date);
        row.set_security(fee.description);

        if fee.amount.is_negative() {
            row.set_revenue(-fee.amount);
            row.set_local_revenue(-fee.local_amount);
        } else {
            row.set_commission(fee.amount);
            row.set_local_commission(fee.local_amount);
        }

        row.set_local_profit(-fee.local_amount);
        row.set_taxable_local_profit(-fee.local_amount);
    }

    fn process_trades(&mut self, mut tax_statement: Option<&'a mut TaxStatement>) -> EmptyResult {
        let (mut fees, mut fees_by_year) = self.pre_process_fees()?;
        let jurisdiction = self.broker_statement.broker.type_.jurisdiction();

        // FIXME(konishchev): HERE
        for (trade_id, trade) in self.broker_statement.stock_sells.iter().enumerate() {
            let (tax_year, _) = self.portfolio.tax_payment_day().get(trade.execution_date, true);

            if let Some(year) = self.year {
                if tax_year != year {
                    continue;
                }
            }

            while let Some(fee) = fees.front() {
                if fee.date >= trade.conclusion_date {
                    break;
                }
                self.post_process_fee(fees.pop_front().unwrap());
            }

            let details = trade.calculate(self.country, tax_year, &self.portfolio.tax_exemptions, self.converter)?;
            self.process_trade(trade_id, trade, &details)?;

            if let Some(ref mut statement) = tax_statement {
                if jurisdiction == Jurisdiction::Usa {
                    let additional_fees = fees_by_year.remove(&tax_year).unwrap_or_default();
                    self.add_income(statement, trade, &details, additional_fees)?;
                } else {
                    warn!(concat!(
                        "Tax statement generation for income from trading is supported only for brokers with USA jurisdiction. ",
                        "Don't adding it to the tax statement."
                    ));
                    tax_statement = None;
                }
            }
        }

        for fee in fees {
            self.post_process_fee(fee);
        }

        Ok(())
    }

    fn process_trade(&mut self, trade_id: usize, trade: &StockSell, details: &SellDetails) -> EmptyResult {
        let security = self.broker_statement.get_instrument_name(&trade.symbol);

        self.same_dates &= trade.execution_date == trade.conclusion_date;
        self.same_currency &= trade.price.currency == self.country.currency &&
            trade.commission.currency == self.country.currency;
        self.tax_exemptions |= details.tax_exemption_applied();

        self.total_local_profit.add_assign(details.local_profit).unwrap();
        self.total_taxable_local_profit.add_assign(details.taxable_local_profit).unwrap();
        self.total_taxable_local_profit_by_year.entry(trade.execution_date.year())
            .and_modify(|total| total.add_assign(details.taxable_local_profit).unwrap())
            .or_insert(details.taxable_local_profit);
        self.total_tax_deduction.add_assign(details.tax_deduction).unwrap();

        let conclusion_currency_rate = if trade.commission.currency != self.country.currency {
            Some(self.converter.precise_currency_rate(
                trade.conclusion_date, trade.commission.currency, self.country.currency)?)
        } else {
            None
        };

        let execution_currency_rate = if trade.price.currency != self.country.currency {
            Some(self.converter.precise_currency_rate(
                trade.execution_date, trade.price.currency, self.country.currency)?)
        } else {
            None
        };

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
            taxable_local_profit: details.taxable_local_profit,

            tax_to_pay: details.tax_to_pay,
            tax_deduction: details.tax_deduction,

            real_profit_ratio: details.real_profit_ratio.map(Cell::new_ratio),
            real_local_profit_ratio: details.real_local_profit_ratio.map(Cell::new_ratio),
        });

        for (index, buy_trade) in details.fifo.iter().enumerate() {
            self.process_fifo(&security, trade_id, buy_trade, index == 0)?;
        }

        Ok(())
    }

    fn process_fifo(&mut self, security: &str, trade_id: usize, buy_trade: &FifoDetails, first: bool) -> EmptyResult {
        self.stock_splits |= buy_trade.multiplier != dec!(1);

        let mut execution_date = None;
        let mut conclusion_currency_rate = None;
        let mut execution_currency_rate = None;

        let source = match buy_trade.type_ {
            StockBuyType::Trade => {
                self.same_dates &= buy_trade.execution_date == buy_trade.conclusion_date;
                self.same_currency &=
                    buy_trade.price.currency == self.country.currency &&
                    buy_trade.commission.currency == self.country.currency;

                execution_date.replace(buy_trade.execution_date);

                if buy_trade.commission.currency != self.country.currency {
                    conclusion_currency_rate.replace(self.converter.precise_currency_rate(
                        buy_trade.conclusion_date, buy_trade.commission.currency, self.country.currency)?);
                };

                if buy_trade.price.currency != self.country.currency {
                    execution_currency_rate.replace(self.converter.precise_currency_rate(
                        buy_trade.execution_date, buy_trade.price.currency, self.country.currency)?);
                };

                "Покупка"
            },
            StockBuyType::CorporateAction => {
                self.non_trade_sources = true;
                "Корп. действие"
            },
        };

        self.fifo_table.add_row(FifoRow {
            id: if first {
                Some(trade_id)
            } else {
                None
            },
            conclusion_date: buy_trade.conclusion_date,
            execution_date,
            security: security.to_owned(),
            quantity: buy_trade.quantity,
            multiplier: buy_trade.multiplier,

            // FIXME(konishchev): HERE: All below
            price: buy_trade.price,
            conclusion_currency_rate,
            execution_currency_rate,

            // FIXME(konishchev): HERE
            cost: buy_trade.cost(buy_trade.price.currency, self.converter)?,
            local_cost: buy_trade.cost(self.country.currency, self.converter)?,

            commission: buy_trade.commission,
            local_commission: buy_trade.local_commission,

            // FIXME(konishchev): HERE
            total_local_cost: buy_trade.total_cost(self.country.currency, self.converter)?,
            source: source.to_owned(),
            tax_free: if buy_trade.tax_exemption_applied {
                Some("✔".to_owned())
            } else {
                None
            },
        });

        Ok(())
    }

    fn add_income(
        &self, tax_statement: &mut TaxStatement, trade: &StockSell, details: &SellDetails,
        additional_fees: Decimal,
    ) -> EmptyResult {
        assert_eq!(details.taxable_local_profit, details.local_profit);

        let name = self.broker_statement.get_instrument_name(&trade.symbol);
        let description = format!("{}: Продажа {}", self.broker_statement.broker.name, name);

        let cost = details.total_local_cost.amount + additional_fees;
        let precise_currency_rate = self.converter.precise_currency_rate(
            trade.execution_date, details.revenue.currency, self.country.currency)?;

        tax_statement.add_stock_income(
            &description, trade.execution_date, details.revenue.currency, precise_currency_rate,
            details.revenue.amount, details.local_revenue.amount, cost,
        ).map_err(|e| format!(
            "Unable to add income from selling {} on {} to the tax statement: {}",
            trade.symbol, formatting::format_date(trade.execution_date), e
        ))?;

        Ok(())
    }

    fn process_totals(&mut self) -> Option<Cash> {
        match self.portfolio.tax_payment_day().spec {
            TaxPaymentDaySpec::Day {..} => Some(self.total_taxable_local_profit_by_year.iter().map(|(&year, profit)| {
                self.country.tax_to_pay(IncomeType::Trading, year, profit.amount, None)
            }).sum()),

            TaxPaymentDaySpec::OnClose(date) => if self.year.is_none() {
                Some(self.country.tax_to_pay(IncomeType::Trading, date.year(), self.total_taxable_local_profit.amount, None))
            } else {
                None
            },
        }.map(|amount| Cash::new(self.country.currency, amount))
    }

    fn print(mut self, total_tax_to_pay: Option<Cash>) {
        if self.trades_table.is_empty() {
            return;
        }

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
        if !self.non_trade_sources {
            self.fifo_table.hide_source();
        }
        if !self.stock_splits {
            self.fifo_table.hide_multiplier();
        }
        if !self.tax_exemptions {
            self.trades_table.hide_taxable_local_profit();
            self.trades_table.hide_tax_deduction();
            self.fifo_table.hide_tax_free();
        }

        let mut totals = self.trades_table.add_empty_row();
        totals.set_local_profit(self.total_local_profit);
        totals.set_taxable_local_profit(self.total_taxable_local_profit);
        if let Some(total_tax_to_pay) = total_tax_to_pay {
            totals.set_tax_to_pay(total_tax_to_pay);
        }
        totals.set_tax_deduction(self.total_tax_deduction);

        self.trades_table.print(&format!(
            "Расчет прибыли от продажи ценных бумаг, полученной через {}",
            self.broker_statement.broker.name));

        if !self.fifo_table.is_empty() {
            self.fifo_table.print("Детализация расчета сделок по ФИФО");
        }
    }
}

struct PreprocessedFee {
    date: Date,
    amount: Cash,
    local_amount: Cash,
    description: String,
}