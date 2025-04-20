use std::collections::{HashMap, BTreeMap};

use itertools::Itertools;
use log::{self, log_enabled, trace};

use crate::broker_statement::{BrokerStatement, StockSource, StockSellType};
use crate::config::PortfolioConfig;
use crate::core::{EmptyResult, GenericResult};
use crate::currency::Cash;
use crate::currency::converter::CurrencyConverter;
use crate::formatting;
use crate::localities::Country;
use crate::taxes::{NetTax, NetTaxCalculator, NetLtoDeduction, NetLtoDeductionCalculator, TaxCalculator};
use crate::time::{self, Date, DateOptTime};
use crate::types::Decimal;

use super::config::PerformanceMergingConfig;
use super::deposit_emulator::{Transaction, InterestPeriod};
use super::deposit_performance;
use super::inflation::InflationCalc;
use super::instrument_view::InstrumentDepositView;
use super::portfolio_performance_types::{
    PerformanceAnalysisMethod, PortfolioPerformanceAnalysis, InstrumentPerformanceAnalysis, IncomeStructure};

/// Calculates average rate of return from cash investments by comparing portfolio performance to
/// performance of a bank deposit with exactly the same investments and monthly capitalization.
pub struct PortfolioPerformanceAnalyser<'a> {
    today: Date,
    country: &'a Country,
    currency: &'a str,
    converter: &'a CurrencyConverter,
    method: PerformanceAnalysisMethod,
    include_closed_positions: bool,
    performance_merging_config: Option<PerformanceMergingConfig>,

    transactions: Vec<Transaction>,
    income_structure: IncomeStructure,
    instruments: Option<BTreeMap<String, InstrumentDepositView>>,
    net_lto_calc: NetLtoDeductionCalculator,
    tax_calculator: TaxCalculator,
    current_assets: Decimal,
}

impl <'a> PortfolioPerformanceAnalyser<'a> {
    pub fn new(
        country: &'a Country, currency: &'a str, converter: &'a CurrencyConverter,
        method: PerformanceAnalysisMethod, include_closed_positions: bool,
    ) -> PortfolioPerformanceAnalyser<'a> {
        PortfolioPerformanceAnalyser {
            today: time::today(),
            country,
            currency,
            converter,
            method,
            include_closed_positions,
            performance_merging_config: None,

            transactions: Vec::new(),
            income_structure: Default::default(),
            instruments: Some(BTreeMap::new()),
            net_lto_calc: NetLtoDeductionCalculator::new(),
            tax_calculator: TaxCalculator::new(country.clone()),
            current_assets: dec!(0),
        }
    }

    pub fn add(
        &mut self, portfolio: &PortfolioConfig, statement: &BrokerStatement,
        merge_performance: PerformanceMergingConfig,
    ) -> EmptyResult {
        self.performance_merging_config.replace(merge_performance);
        let result = self.add_inner(portfolio, statement);
        self.performance_merging_config.take();
        result
    }

    fn add_inner(&mut self, portfolio: &PortfolioConfig, statement: &BrokerStatement) -> EmptyResult {
        // Assume that the caller has simulated sellout and just check it here
        if !statement.open_positions.is_empty() {
            return Err!(
                "Unable to calculate current assets: The broker statement has open positions");
        }

        trace!("Deposit emulator transactions for {:?}:", portfolio.name);
        self.process_deposits_and_withdrawals(statement)?;
        self.process_positions(statement, portfolio)?;
        self.process_dividends(statement, portfolio)?;
        self.process_interest(statement, portfolio)?;
        self.process_grants(statement)?;
        self.process_fees(statement)?;
        self.process_tax_agent_withholdings(statement)?;
        self.process_tax_deductions(portfolio)?;
        self.process_cash_assets(statement)?;

        for (symbol, deposit_view) in self.instruments.as_mut().unwrap().iter_mut() {
            if deposit_view.name.is_none() {
                deposit_view.name.replace(statement.instrument_info.get_name(symbol));
            }
        }

        Ok(())
    }

    pub fn analyse(mut self) -> GenericResult<(PortfolioPerformanceAnalysis, BTreeMap<i32, NetLtoDeduction>)> {
        let mut instrument_performance = BTreeMap::new();

        self.calculate_open_position_periods()?;

        for (symbol, deposit_view) in self.instruments.take().unwrap() {
            if deposit_view.closed && !self.include_closed_positions {
                continue;
            }

            let analysis = self.analyse_instrument_performance(&symbol, deposit_view)?;
            assert!(instrument_performance.insert(symbol, analysis).is_none());
        }

        let portfolio_performance = self.analyse_portfolio_performance()?;
        self.income_structure.net_profit = portfolio_performance.net_profit();

        Ok((PortfolioPerformanceAnalysis {
            income_structure: self.income_structure,
            instruments: instrument_performance,
            portfolio: portfolio_performance,
        }, self.net_lto_calc.calculate()))
    }

    fn analyse_instrument_performance(
        &mut self, symbol: &str, mut deposit_view: InstrumentDepositView
    ) -> GenericResult<InstrumentPerformanceAnalysis> {
        trace!("Analysing {} performance...", symbol);

        deposit_view.transactions.sort_by_key(|transaction| transaction.date);
        let adjusted_transactions = self.adjust_transactions(&deposit_view.transactions)?;

        let interest = deposit_performance::compare_instrument_to_bank_deposit(
            symbol, self.currency, &adjusted_transactions, &deposit_view.interest_periods, dec!(0))?;

        let name = deposit_view.name.unwrap();
        let days = get_total_activity_duration(&deposit_view.interest_periods);

        let mut investments = dec!(0);
        let mut result = dec!(0);

        for transaction in &deposit_view.transactions {
            if transaction.amount.is_sign_positive() {
                investments += transaction.amount;
            } else {
                result += -transaction.amount;
            }
        }

        Ok(InstrumentPerformanceAnalysis {
            name, days, investments, result, interest,
            inactive: deposit_view.closed,
        })
    }

    fn analyse_portfolio_performance(&mut self) -> GenericResult<InstrumentPerformanceAnalysis> {
        trace!("Analysing portfolio performance...");

        if self.transactions.is_empty() {
            return Err!("The portfolio has no activity yet");
        }

        self.transactions.sort_by_key(|transaction| transaction.date);
        let adjusted_transactions = self.adjust_transactions(&self.transactions)?;

        let activity_periods = [InterestPeriod::new(
            self.transactions.first().unwrap().date, self.today)];

        let interest = deposit_performance::compare_instrument_to_bank_deposit(
            "portfolio", self.currency, &adjusted_transactions, &activity_periods, self.current_assets)?;

        let days = get_total_activity_duration(&activity_periods);
        let investments = self.transactions.iter()
            .map(|transaction| transaction.amount)
            .sum();

        Ok(InstrumentPerformanceAnalysis {
            name: s!("Portfolio"),
            days, investments,
            result: self.current_assets,
            interest,
            inactive: false
        })
    }

    fn calculate_open_position_periods(&mut self) -> EmptyResult {
        trace!("Open positions periods:");

        for (symbol, deposit_view) in self.instruments.as_mut().unwrap() {
            deposit_view.calculate_open_position_periods()?;

            if log_enabled!(log::Level::Trace) {
                let periods = deposit_view.interest_periods.iter()
                    .map(|period| format!(
                        "{} - {}", formatting::format_date(period.start),
                        formatting::format_date(period.end)))
                    .join(", ");

                trace!("* {}: {}", symbol, periods);
            }
        }

        Ok(())
    }

    fn process_deposits_and_withdrawals(&mut self, statement: &BrokerStatement) -> EmptyResult {
        for mut assets in statement.deposits_and_withdrawals.iter().cloned() {
            if assets.cash.is_positive() {
                let commission = statement.broker.get_deposit_commission(self.country, assets)?;

                self.income_structure.commissions += self.converter.convert_to(
                    assets.date, commission, self.currency)?;

                assets.cash += commission;
            }

            let amount = self.converter.convert_to(assets.date, assets.cash, self.currency)?;

            trace!("* {} {}: {}", if amount.is_sign_positive() {
                "Deposit"
            } else {
                "Withdrawal"
            }, formatting::format_date(assets.date), amount.normalize());

            self.transaction(assets.date, amount);
        }

        Ok(())
    }

    fn process_positions(&mut self, statement: &BrokerStatement, portfolio: &PortfolioConfig) -> EmptyResult {
        struct Taxes<'a> {
            stocks: HashMap<&'a str, NetTaxCalculator>,
            portfolio: NetTaxCalculator,
        }

        let mut taxes = self.method.tax_aware().then(|| {
            Taxes {
                stocks: HashMap::new(),
                portfolio: NetTaxCalculator::new(self.country.clone(), portfolio.tax_payment_day()),
            }
        });

        for trade in &statement.stock_buys {
            let multiplier = statement.stock_splits.get_multiplier(
                &trade.symbol, trade.conclusion_time, DateOptTime::new_max_time(self.today));
            let quantity = multiplier * trade.quantity;

            match trade.type_ {
                StockSource::Trade {volume, commission, ..} => {
                    let volume = self.converter.convert_to(
                        trade.execution_date, volume, self.currency)?;

                    let commission = self.converter.convert_to(
                        trade.conclusion_time.date, commission, self.currency)?;
                    self.income_structure.commissions += commission;

                    let deposit_view = self.get_deposit_view(&trade.symbol);
                    deposit_view.trade(&portfolio.name, &trade.symbol, trade.conclusion_time, quantity);
                    deposit_view.transaction(trade.conclusion_time, volume);
                    deposit_view.transaction(trade.conclusion_time, commission);
                },

                StockSource::CorporateAction | StockSource::Grant => {
                    self.get_deposit_view(&trade.symbol).trade(
                        &portfolio.name, &trade.symbol, trade.conclusion_time, quantity);
                },
            };
        }

        for trade in &statement.stock_sells {
            let multiplier = statement.stock_splits.get_multiplier(
                &trade.symbol, trade.conclusion_time, DateOptTime::new_max_time(self.today));
            let quantity = multiplier * trade.quantity;

            match trade.type_ {
                StockSellType::Trade {volume, commission, ..} => {
                    let volume = self.converter.convert_to(
                        trade.execution_date, volume, self.currency)?;

                    let commission = self.converter.convert_to(
                        trade.conclusion_time.date, commission, self.currency)?;
                    self.income_structure.commissions += commission;

                    {
                        let deposit_view = self.get_deposit_view(&trade.symbol);

                        deposit_view.trade(&portfolio.name, &trade.symbol, trade.conclusion_time, -quantity);
                        deposit_view.transaction(trade.conclusion_time, -volume);
                        deposit_view.transaction(trade.conclusion_time, commission);

                        if trade.emulation {
                            deposit_view.closed = false;
                        }
                    }

                    if let Some(taxes) = taxes.as_mut() {
                        let (tax_year, _) = portfolio.tax_payment_day().get(trade.execution_date, true);
                        let instrument = statement.instrument_info.get_or_empty(&trade.symbol);
                        let details = trade.calculate(self.country, &instrument, &portfolio.tax_exemptions, self.converter)?;

                        let mut lto_deductibles = Vec::new();

                        for fifo in &details.fifo {
                            if let Some(lto) = fifo.long_term_ownership_deductible {
                                self.net_lto_calc.add_profit(tax_year, lto.profit, lto.years, trade.emulation);
                                lto_deductibles.push(lto);
                            }
                        }

                        taxes.stocks.entry(&trade.symbol)
                            .or_insert_with(|| NetTaxCalculator::new(
                                self.country.clone(), portfolio.tax_payment_day()))
                            .add_profit(
                                trade.execution_date, details.local_profit, details.taxable_local_profit,
                                &lto_deductibles, trade.emulation);

                        taxes.portfolio.add_profit(
                            trade.execution_date, details.local_profit, details.taxable_local_profit,
                            &lto_deductibles, trade.emulation);
                    }
                },

                StockSellType::CorporateAction => {
                    self.get_deposit_view(&trade.symbol).trade(
                        &portfolio.name, &trade.symbol, trade.conclusion_time, -quantity);
                },
            };
        }

        if let Some(taxes) = taxes {
            for (symbol, symbol_taxes) in taxes.stocks.into_iter() {
                let mut tax_calculator = TaxCalculator::new(self.country.clone());

                for (_, NetTax{tax_payment_date, tax_to_pay, ..}) in symbol_taxes.calculate(&mut tax_calculator).into_iter() {
                    if let Some(amount) = self.map_tax_to_deposit_amount(tax_payment_date, tax_to_pay)? {
                        trace!("* {} selling {} tax: {}",
                            symbol, formatting::format_date(tax_payment_date), amount);

                        self.get_deposit_view(symbol).transaction(tax_payment_date.into(), amount);
                    }
                }
            }

            for (tax_year, NetTax {
                tax_payment_date, tax_to_pay, tax_deduction,
                lto_deduction, lto_loss,
            }) in taxes.portfolio.calculate(&mut self.tax_calculator).into_iter() {
                if let Some(amount) = self.map_tax_to_deposit_amount(tax_payment_date, tax_to_pay)? {
                    trace!("* Stock selling {} tax: {}", formatting::format_date(tax_payment_date), amount);
                    self.transaction(tax_payment_date, amount);
                    self.income_structure.trading_taxes += amount;
                }

                if let Some(amount) = self.map_tax_to_deposit_amount(tax_payment_date, tax_deduction)? {
                    trace!("* {} tax deduction: {}", formatting::format_date(tax_payment_date), amount);
                    self.income_structure.trading_tax_deductions += amount;
                }

                self.net_lto_calc.add_applied_deduction(tax_year, lto_deduction.amount, lto_loss.amount);
            }
        }

        Ok(())
    }

    fn process_dividends(&mut self, statement: &BrokerStatement, portfolio: &PortfolioConfig) -> EmptyResult {
        let tax_aware = self.method.tax_aware();

        for dividend in &statement.dividends {
            let income = self.converter.convert_to(dividend.date, dividend.amount, self.currency)?;
            let paid_tax = self.converter.convert_to(dividend.date, dividend.paid_tax, self.currency)?;

            self.get_deposit_view(&dividend.issuer).transaction(dividend.date.into(), -income);
            self.income_structure.dividends += income;

            if tax_aware {
                self.get_deposit_view(&dividend.issuer).transaction(dividend.date.into(), paid_tax);
                self.income_structure.dividend_taxes += paid_tax;
            }

            if tax_aware {
                let tax = dividend.tax(self.country, self.converter, &mut self.tax_calculator)?;
                let (_, tax_payment_date) = portfolio.tax_payment_day().get(dividend.date, false);

                if let Some(amount) = self.map_tax_to_deposit_amount(tax_payment_date, tax.to_pay)? {
                    trace!("* {} {} dividend {} tax: {}",
                        dividend.original_issuer, formatting::format_date(dividend.date),
                        formatting::format_date(tax_payment_date), amount);

                    self.get_deposit_view(&dividend.issuer).transaction(tax_payment_date.into(), amount);
                    self.transaction(tax_payment_date, amount);
                    self.income_structure.dividend_taxes += amount;
                }
            }
        }

        Ok(())
    }

    fn process_interest(&mut self, statement: &BrokerStatement, portfolio: &PortfolioConfig) -> EmptyResult {
        for interest in &statement.idle_cash_interest {
            self.income_structure.interest += self.converter.convert_to(
                interest.date, interest.amount, self.currency)?;

            if self.method.tax_aware() {
                let tax_to_pay = interest.tax(self.country, self.converter, &mut self.tax_calculator)?;
                let (_, tax_payment_date) = portfolio.tax_payment_day().get(interest.date, false);

                if let Some(amount) = self.map_tax_to_deposit_amount(tax_payment_date, tax_to_pay)? {
                    trace!("* {} idle cash interest {} tax: {}",
                        formatting::format_date(interest.date),
                        formatting::format_date(tax_payment_date), amount);

                    self.transaction(tax_payment_date, amount);
                    self.income_structure.interest_taxes += amount;
                }
            }
        }

        Ok(())
    }

    fn process_grants(&mut self, statement: &BrokerStatement) -> EmptyResult {
        for grant in &statement.cash_grants {
            self.income_structure.grants += self.converter.convert_to(grant.date, grant.amount, self.currency)?;
        }

        Ok(())
    }

    fn process_fees(&mut self, statement: &BrokerStatement) -> EmptyResult {
        for fee in &statement.fees {
            self.income_structure.commissions += self.converter.convert_to(
                fee.date, fee.amount.withholding(), self.currency)?;
        }

        Ok(())
    }

    fn process_tax_agent_withholdings(&mut self, statement: &BrokerStatement) -> EmptyResult {
        for tax in &statement.tax_agent_withholdings {
            let amount = self.converter.convert_to(tax.date, tax.amount.withholding(), self.currency)?;
            trace!("* Tax withholding {}: {}", formatting::format_date(tax.date), amount);
            self.transaction(tax.date, -amount);
        }

        Ok(())
    }

    fn process_tax_deductions(&mut self, portfolio: &PortfolioConfig) -> EmptyResult {
        if !self.method.tax_aware() {
            return Ok(());
        }

        for &(date, amount) in &portfolio.tax_deductions {
            let amount = self.converter.convert(self.country.currency, self.currency, date, amount)?;
            trace!("* Tax deduction {}: {}", formatting::format_date(date), amount);
            self.transaction(date, -amount);
            self.income_structure.additional_tax_deductions += amount;
        }

        Ok(())
    }

    fn process_cash_assets(&mut self, statement: &BrokerStatement) -> EmptyResult {
        self.current_assets += statement.assets.cash.total_assets_real_time(
            self.currency, self.converter)?;
        Ok(())
    }

    fn get_deposit_view(&mut self, symbol: &str) -> &mut InstrumentDepositView {
        let mapped_symbol = self.performance_merging_config.as_ref().unwrap().map(symbol);
        self.instruments.as_mut().unwrap()
            .entry(mapped_symbol.to_owned())
            .or_insert_with(|| InstrumentDepositView::new(mapped_symbol))
    }

    fn transaction(&mut self, date: Date, amount: Decimal) {
        self.transactions.push(Transaction::new(date, amount));
    }

    fn adjust_transactions(&self, transactions: &[Transaction]) -> GenericResult<Vec<Transaction>> {
        let inflation_calc = match self.method {
            PerformanceAnalysisMethod::Virtual | PerformanceAnalysisMethod::Real => None,
            PerformanceAnalysisMethod::InflationAdjusted => Some(
                InflationCalc::new(self.currency, self.today)?
            ),
        };

        Ok(transactions.iter().map(|transaction| {
            let amount = match inflation_calc.as_ref() {
                Some(calc) => calc.adjust(transaction.date, transaction.amount),
                None => transaction.amount,
            };
            Transaction::new(transaction.date, amount)
        }).collect())
    }

    fn map_tax_to_deposit_amount(&self, tax_payment_date: Date, tax_to_pay: Cash) -> GenericResult<Option<Decimal>> {
        // Treat tax payment as an ordinary deposit which we transfer to the account at tax payment
        // day.

        if tax_to_pay.is_zero() {
            return Ok(None);
        }
        assert!(tax_to_pay.is_positive());

        let conversion_date = if tax_payment_date > self.today {
            self.today
        } else {
            tax_payment_date
        };

        Ok(Some(self.converter.convert_to(conversion_date, tax_to_pay, self.currency)?))
    }
}

fn get_total_activity_duration(periods: &[InterestPeriod]) -> u32 {
    periods.iter().map(InterestPeriod::days).sum()
}