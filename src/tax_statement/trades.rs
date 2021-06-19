use std::collections::{BTreeMap, VecDeque};

use chrono::Datelike;
use log::warn;

use static_table_derive::StaticTable;

use crate::brokers::Broker;
use crate::broker_statement::{
    BrokerStatement, StockSell, StockSellType, SellDetails, FifoDetails, StockSourceDetails, Fee};
use crate::config::PortfolioConfig;
use crate::core::{EmptyResult, GenericResult};
use crate::currency::{Cash, MultiCurrencyCashAccount};
use crate::currency::converter::CurrencyConverter;
use crate::formatting::{self, table::Cell};
use crate::localities::{Country, Jurisdiction};
use crate::taxes::{IncomeType, TaxPaymentDaySpec};
use crate::taxes::long_term_ownership::LtoDeductionCalculator;
use crate::time::Date;
use crate::trades;
use crate::types::Decimal;

use super::statement::TaxStatement;

pub fn process_income(
    country: &Country, portfolio: &PortfolioConfig, broker_statement: &BrokerStatement,
    year: Option<i32>, tax_statement: Option<&mut TaxStatement>, converter: &CurrencyConverter,
) -> GenericResult<Cash> {
    let mut processor = TradesProcessor {
        portfolio,
        broker_statement,
        tax_year: year,

        country,
        converter,

        trades_table: TradesTable::new(),
        fifo_table: FifoTable::new(),
        lto_table: LtoTable::new(),

        same_dates: true,
        same_currency: true,
        non_trade_sources: false,
        stock_splits: false,
        tax_exemptions: false,
        long_term_ownership: false,

        tax_year_stat: BTreeMap::new(),
    };

    processor.process_trades(tax_statement)?;

    let totals = processor.process_totals()?;
    if !processor.trades_table.is_empty() {
        processor.print(&totals);
    }

    Ok(totals.tax_to_pay)
}

struct TradesProcessor<'a> {
    portfolio: &'a PortfolioConfig,
    broker_statement: &'a BrokerStatement,
    tax_year: Option<i32>,

    country: &'a Country,
    converter: &'a CurrencyConverter,

    trades_table: TradesTable,
    fifo_table: FifoTable,
    lto_table: LtoTable,

    same_dates: bool,
    same_currency: bool,
    non_trade_sources: bool,
    stock_splits: bool,
    tax_exemptions: bool,
    long_term_ownership: bool,

    tax_year_stat: BTreeMap<i32, TaxYearStat>,
}

impl<'a> TradesProcessor<'a> {
    fn pre_process_fees(&mut self) -> GenericResult<VecDeque<PreprocessedFee>> {
        let broker = self.broker_statement.broker.type_;
        let mut fees = VecDeque::new();

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

            let tax_year = self.get_tax_year(fee.date);
            if self.needs_processing(tax_year) {
                fees.push_back(self.pre_process_fee(fee)?);
            }
        }

        Ok(fees)
    }

    fn pre_process_fee(&mut self, fee: &Fee) -> GenericResult<PreprocessedFee> {
        self.same_currency &= fee.amount.currency == self.country.currency;

        let amount = fee.amount.round();
        let local_amount = self.converter.convert_to_cash_rounding(
            fee.date, fee.amount, self.country.currency)?;

        let tax_year = self.tax_year_stat(fee.date);
        tax_year.profit.withdraw(amount);
        tax_year.local_profit -= local_amount;
        tax_year.taxable_local_profit -= local_amount;
        tax_year.deductible_fees.replace(tax_year.deductible_fees.unwrap_or_default() + local_amount.amount);

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
        let mut fees = self.pre_process_fees()?;
        let jurisdiction = self.broker_statement.broker.type_.jurisdiction();

        let mut trade_id = 0;

        for trade in &self.broker_statement.stock_sells {
            match trade.type_ {
                StockSellType::Trade {..} => (),
                StockSellType::CorporateAction => continue,
            };

            let tax_year = self.get_tax_year(trade.execution_date);
            if !self.needs_processing(tax_year) {
                continue;
            }

            while let Some(fee) = fees.front() {
                if fee.date >= trade.conclusion_time.date {
                    break;
                }
                self.post_process_fee(fees.pop_front().unwrap());
            }

            let details = trade.calculate(self.country, tax_year, &self.portfolio.tax_exemptions, self.converter)?;
            self.process_trade(trade_id, trade, &details)?;
            trade_id += 1;

            if let Some(ref mut statement) = tax_statement {
                if jurisdiction == Jurisdiction::Usa {
                    let tax_year_stat = self.tax_year_stat.get_mut(&tax_year).unwrap();
                    let additional_fees = tax_year_stat.deductible_fees.take().unwrap_or_default();
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
        let security = self.broker_statement.get_instrument_name(&trade.original_symbol);
        let (price, commission) = match trade.type_ {
            StockSellType::Trade {price, commission, ..} => (price, commission),
            _ => unreachable!(),
        };

        self.same_dates &= trade.execution_date == trade.conclusion_time.date;
        self.same_currency &= price.currency == self.country.currency &&
            commission.currency == self.country.currency;
        self.tax_exemptions |= details.tax_exemption_applied();

        let conclusion_currency_rate = if commission.currency != self.country.currency {
            Some(self.converter.precise_currency_rate(
                trade.conclusion_time.date, commission.currency, self.country.currency)?)
        } else {
            None
        };

        let execution_currency_rate = if price.currency != self.country.currency {
            Some(self.converter.precise_currency_rate(
                trade.execution_date, price.currency, self.country.currency)?)
        } else {
            None
        };

        let real = details.real_profit(self.converter)?;

        {
            let tax_year = self.tax_year_stat(trade.execution_date);

            tax_year.purchase_cost.deposit(details.purchase_cost);
            tax_year.purchase_local_cost += details.purchase_local_cost;

            tax_year.profit.deposit(details.profit);
            tax_year.local_profit += details.local_profit;
            tax_year.taxable_local_profit += details.taxable_local_profit;
        }

        self.trades_table.add_row(TradeRow {
            id: trade_id,
            conclusion_date: trade.conclusion_time.date,
            execution_date: trade.execution_date,
            security: security,
            quantity: trade.quantity,

            price,
            conclusion_currency_rate: conclusion_currency_rate,
            execution_currency_rate: execution_currency_rate,

            revenue: details.revenue,
            local_revenue: details.local_revenue,

            commission: commission.round(),
            local_commission: details.local_commission,

            purchase_local_cost: details.purchase_local_cost,
            total_local_cost: details.total_local_cost,

            local_profit: details.local_profit,
            taxable_local_profit: details.taxable_local_profit,

            tax_to_pay: details.tax_to_pay,
            tax_deduction: details.tax_deduction,

            // FIXME(konishchev): More columns?
            real_profit_ratio: real.profit_ratio.map(Cell::new_ratio),
            real_local_profit_ratio: real.local_profit_ratio.map(Cell::new_ratio),
        });

        for (index, buy_trade) in details.fifo.iter().enumerate() {
            self.process_fifo(trade_id, buy_trade, trade.execution_date, index == 0)?;
        }

        Ok(())
    }

    fn process_fifo(
        &mut self, trade_id: usize, trade: &FifoDetails, sell_execution_date: Date, first: bool,
    ) -> EmptyResult {
        let security = self.broker_statement.get_instrument_name(&trade.original_symbol);
        self.stock_splits |= trade.multiplier != dec!(1);

        let mut execution_date_cell = None;

        let mut price_cell = None;
        let mut execution_currency_rate_cell = None;

        let mut commission_cell = None;
        let mut local_commission_cell = None;
        let mut conclusion_currency_rate_cell = None;

        let mut cost_cell = None;
        let mut local_cost_cell = None;

        let source = match trade.source {
            StockSourceDetails::Trade {price, commission, local_commission, cost, local_cost, ..} => {
                self.same_dates &= trade.execution_date == trade.conclusion_time.date;
                self.same_currency &=
                    price.currency == self.country.currency &&
                    commission.currency == self.country.currency;

                execution_date_cell.replace(trade.execution_date);
                price_cell.replace(price);

                commission_cell.replace(commission);
                local_commission_cell.replace(local_commission);

                cost_cell.replace(cost);
                local_cost_cell.replace(local_cost);

                if commission.currency != self.country.currency {
                    conclusion_currency_rate_cell.replace(self.converter.precise_currency_rate(
                        trade.conclusion_time.date, commission.currency, self.country.currency)?);
                };

                if price.currency != self.country.currency {
                    execution_currency_rate_cell.replace(self.converter.precise_currency_rate(
                        trade.execution_date, price.currency, self.country.currency)?);
                };

                "Покупка"
            },
            StockSourceDetails::CorporateAction => {
                self.non_trade_sources = true;
                "Корп. действие"
            },
        };

        if let Some(ref deductible) = trade.long_term_ownership_deductible {
            let tax_year_stat = self.tax_year_stat(sell_execution_date);
            tax_year_stat.lto_calculator.as_mut().unwrap().add(deductible.profit, deductible.years);
            self.long_term_ownership = true;
        }

        self.fifo_table.add_row(FifoRow {
            id: if first {
                Some(trade_id)
            } else {
                None
            },
            conclusion_date: trade.conclusion_time.date,
            execution_date: execution_date_cell,
            security: security,
            quantity: trade.quantity,
            multiplier: trade.multiplier,

            price: price_cell,
            conclusion_currency_rate: conclusion_currency_rate_cell,
            execution_currency_rate: execution_currency_rate_cell,

            cost: cost_cell,
            local_cost: local_cost_cell,

            commission: commission_cell,
            local_commission: local_commission_cell,

            total_local_cost: trade.total_cost(self.country.currency, self.converter)?,
            source: source.to_owned(),

            long_term_ownership: trade.long_term_ownership_deductible.is_some(),
            tax_free: trade.tax_exemption_applied,
        });

        Ok(())
    }

    fn add_income(
        &self, tax_statement: &mut TaxStatement, trade: &StockSell, details: &SellDetails,
        additional_fees: Decimal,
    ) -> EmptyResult {
        assert_eq!(details.taxable_local_profit, details.local_profit);
        assert!(details.fifo.iter().all(|trade| trade.long_term_ownership_deductible.is_none()));

        let name = self.broker_statement.get_instrument_name(&trade.original_symbol);
        let description = format!("{}: Продажа {}", self.broker_statement.broker.name, name);

        let cost = details.total_local_cost.amount + additional_fees;
        let precise_currency_rate = self.converter.precise_currency_rate(
            trade.execution_date, details.revenue.currency, self.country.currency)?;

        tax_statement.add_stock_income(
            &description, trade.execution_date, details.revenue.currency, precise_currency_rate,
            details.revenue.amount, details.local_revenue.amount, cost,
        ).map_err(|e| format!(
            "Unable to add income from selling {} on {} to the tax statement: {}",
            trade.original_symbol, formatting::format_date(trade.execution_date), e
        ))?;

        Ok(())
    }

    fn process_totals(&mut self) -> GenericResult<Totals> {
        let local_currency = self.country.currency;
        let tax_payment_day = self.portfolio.tax_payment_day();

        for (&year, stat) in &mut self.tax_year_stat {
            let (lto_deduction, lto_limit) = stat.lto_calculator.take().unwrap().calculate();
            if !lto_deduction.is_zero() {
                stat.taxable_local_profit.amount -= lto_deduction;
                self.lto_table.add_row(LtoRow {
                    year,
                    deduction: Cash::new(local_currency, lto_deduction),
                    limit: Cash::new(local_currency, lto_limit),
                });
            }
        }

        let mut total_local_profit = Cash::zero(local_currency);
        let mut total_taxable_local_profit = Cash::zero(local_currency);

        let mut total_tax_without_deduction = Cash::zero(local_currency);
        let mut total_tax_to_pay = Cash::zero(local_currency);

        for (&year, stat) in &self.tax_year_stat {
            let single_tax_year = match tax_payment_day.spec {
                TaxPaymentDaySpec::Day {..} => if let Some(tax_year) = self.tax_year {
                    assert_eq!(year, tax_year);
                    true
                } else {
                    false
                },
                TaxPaymentDaySpec::OnClose(close_date) => {
                    assert_eq!(year, close_date.year());
                    true
                },
            };

            total_local_profit += stat.local_profit;
            total_taxable_local_profit += stat.taxable_local_profit;

            total_tax_without_deduction += self.country.tax_to_pay(
                IncomeType::Trading, year, stat.local_profit, None);

            let tax_to_pay = self.country.tax_to_pay(
                IncomeType::Trading, year, stat.taxable_local_profit, None);
            total_tax_to_pay += tax_to_pay;

            if single_tax_year {
                // FIXME(konishchev): Support
                trades::calculate_real_profit(
                    tax_payment_day.get_for(year, true),
                    stat.purchase_cost.clone(), stat.purchase_local_cost,
                    stat.profit.clone(), stat.local_profit, tax_to_pay, self.converter)?;
            }
        }

        let total_tax_deduction = total_tax_without_deduction - total_tax_to_pay;
        assert!(!total_tax_deduction.is_negative());

        Ok(Totals {
            local_profit: total_local_profit,
            taxable_local_profit: total_taxable_local_profit,

            tax_to_pay: total_tax_to_pay,
            tax_deduction: total_tax_deduction,
        })
    }

    fn print(mut self, totals: &Totals) {
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
        if !self.tax_exemptions && !self.long_term_ownership {
            self.trades_table.hide_taxable_local_profit();
            self.trades_table.hide_tax_deduction();
        }
        if !self.tax_exemptions {
            self.fifo_table.hide_tax_free();
        }
        if !self.long_term_ownership {
            self.fifo_table.hide_long_term_ownership();
        }
        if self.tax_year.is_some() {
            self.lto_table.hide_year();
        }

        // FIXME(konishchev): More totals?
        let mut totals_row = self.trades_table.add_empty_row();
        totals_row.set_local_profit(totals.local_profit);
        totals_row.set_taxable_local_profit(totals.taxable_local_profit);
        totals_row.set_tax_to_pay(totals.tax_to_pay);
        totals_row.set_tax_deduction(totals.tax_deduction);

        self.trades_table.print(&format!(
            "Расчет прибыли от продажи ценных бумаг, полученной через {}",
            self.broker_statement.broker.name));

        if !self.fifo_table.is_empty() {
            self.fifo_table.print("Детализация расчета сделок по ФИФО");
        }

        if !self.lto_table.is_empty() {
            self.lto_table.print("Льгота на долгосрочное владение ценными бумагами");
        }
    }

    fn tax_year_stat(&mut self, date: Date) -> &mut TaxYearStat {
        let local_currency = self.country.currency;

        let tax_year = self.get_tax_year(date);
        assert!(self.needs_processing(tax_year), "An attempt to process {} tax year", tax_year);

        self.tax_year_stat.entry(tax_year).or_insert_with(|| {
            let zero = Cash::zero(local_currency);
            TaxYearStat {
                purchase_cost: MultiCurrencyCashAccount::new(),
                purchase_local_cost: zero,

                profit: MultiCurrencyCashAccount::new(),
                local_profit: zero,
                taxable_local_profit: zero,

                deductible_fees: None,
                lto_calculator: Some(LtoDeductionCalculator::new()),
            }
        })
    }

    fn get_tax_year(&self, date: Date) -> i32 {
        self.portfolio.tax_payment_day().get(date, true).0
    }

    fn needs_processing(&self, tax_year: i32) -> bool {
        match self.tax_year {
            Some(year) => tax_year == year,
            None => true,
        }
    }
}

struct PreprocessedFee {
    date: Date,
    amount: Cash,
    local_amount: Cash,
    description: String,
}

struct TaxYearStat {
    purchase_cost: MultiCurrencyCashAccount,
    purchase_local_cost: Cash,

    profit: MultiCurrencyCashAccount,
    local_profit: Cash,
    taxable_local_profit: Cash,

    deductible_fees: Option<Decimal>,
    lto_calculator: Option<LtoDeductionCalculator>,
}

struct Totals {
    local_profit: Cash,
    taxable_local_profit: Cash,

    tax_to_pay: Cash,
    tax_deduction: Cash,
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
    price: Option<Cash>,
    #[column(name="Курс руб.\nдата сделки")]
    conclusion_currency_rate: Option<Decimal>,
    #[column(name="Курс руб.\nдата расчета")]
    execution_currency_rate: Option<Decimal>,
    #[column(name="Расходы")]
    cost: Option<Cash>,
    #[column(name="Расходы (руб)")]
    local_cost: Option<Cash>,
    #[column(name="Комиссия")]
    commission: Option<Cash>,
    #[column(name="Комиссия\n(руб)")]
    local_commission: Option<Cash>,
    #[column(name="Общие затраты")]
    total_local_cost: Cash,
    #[column(name="Источник", align="center")]
    source: String,
    #[column(name="ЛДВ", align="center")]
    long_term_ownership: bool,
    #[column(name="Льгота", align="center")]
    tax_free: bool,
}

#[derive(StaticTable)]
#[table(name="LtoTable")]
struct LtoRow {
    #[column(name="Год")]
    year: i32,
    #[column(name="Вычет")]
    deduction: Cash,
    #[column(name="Лимит")]
    limit: Cash,
}