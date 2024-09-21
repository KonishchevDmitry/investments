mod cash_flows;
mod corporate_actions;
mod dividends;
mod fees;
mod grants;
mod interest;
mod merging;
mod partial;
mod payments;
mod reader;
mod taxes;
mod trades;
mod validators;

mod bcs;
mod firstrade;
mod ib;
mod open;
mod sber;
mod tinkoff;

use std::cmp::Ordering;
use std::collections::{HashMap, BTreeMap, BTreeSet, hash_map::Entry};

use itertools::Itertools;
use log::{debug, warn};

use crate::brokers::{BrokerInfo, Broker};
use crate::commissions::CommissionCalc;
use crate::core::{EmptyResult, GenericResult};
use crate::currency::{Cash, CashAssets, MultiCurrencyCashAccount};
use crate::currency::converter::CurrencyConverter;
use crate::exchanges::{Exchange, Exchanges, TradingMode};
use crate::formatting;
use crate::instruments::{InstrumentInternalIds, InstrumentInfo};
use crate::quotes::{Quotes, QuoteQuery};
use crate::taxes::{TaxRemapping, TaxExemption, long_term_ownership};
use crate::time::{self, Date, DateOptTime, Period};
use crate::types::{Decimal, TradeType};
use crate::util;

use self::dividends::{DividendAccruals, process_dividend_accruals};
use self::partial::PartialBrokerStatement;
use self::reader::BrokerStatementReader;
use self::taxes::{TaxId, TaxAccruals, TaxAgentWithholdings};
use self::validators::{DateValidator, sort_and_validate_trades};

pub use self::cash_flows::{CashFlow, CashFlowType};
pub use self::corporate_actions::{CorporateAction, StockSplitController, process_corporate_actions};
pub use self::dividends::Dividend;
pub use self::fees::Fee;
pub use self::grants::{StockGrant, process_grants};
pub use self::interest::IdleCashInterest;
pub use self::merging::StatementsMergingStrategy;
pub use self::payments::Withholding;
pub use self::reader::ReadingStrictness;
pub use self::taxes::TaxAgentWithholding;
pub use self::trades::{ForexTrade, StockBuy, StockSource, StockSell, StockSellType, StockSourceDetails, SellDetails, FifoDetails};

pub struct BrokerStatement {
    pub broker: BrokerInfo,
    pub period: Period,

    pub assets: NetAssets,
    pub historical_assets: BTreeMap<Date, NetAssets>,

    pub fees: Vec<Fee>,
    pub cash_flows: Vec<CashFlow>,
    pub deposits_and_withdrawals: Vec<CashAssets>,
    pub idle_cash_interest: Vec<IdleCashInterest>,
    pub tax_agent_withholdings: TaxAgentWithholdings,

    pub exchanges: Exchanges,
    pub forex_trades: Vec<ForexTrade>,
    pub stock_buys: Vec<StockBuy>,
    pub stock_sells: Vec<StockSell>,
    pub dividends: Vec<Dividend>,

    stock_grants: Vec<StockGrant>,
    corporate_actions: Vec<CorporateAction>,
    pub stock_splits: StockSplitController,

    pub open_positions: HashMap<String, Decimal>,
    pub instrument_info: InstrumentInfo,
}

impl BrokerStatement {
    pub fn read(
        broker: BrokerInfo, statement_dir_path: &str, symbol_remapping: &HashMap<String, String>,
        instrument_internal_ids: &InstrumentInternalIds, instrument_names: &HashMap<String, String>,
        tax_remapping: TaxRemapping, tax_exemptions: &[TaxExemption], corporate_actions: &[CorporateAction],
        strictness: ReadingStrictness,
    ) -> GenericResult<BrokerStatement> {
        let broker_jurisdiction = broker.type_.jurisdiction();

        let mut statements = reader::read(broker.type_, statement_dir_path, tax_remapping, strictness)?;
        statements.sort_by_key(|statement| statement.period.unwrap());

        let mut last_period = statements.first().unwrap().period.unwrap();
        for statement in &statements[1..] {
            let period = statement.period.unwrap();
            if period.first_date() <= last_period.last_date() {
                return Err!(
                    "Overlapping broker statement periods: {} and {}",
                    last_period.format(), period.format());
            }
            last_period = period;
        }

        let last_index = statements.len() - 1;
        let mut statement = BrokerStatement::new_empty_from(broker, statements.first().unwrap())?;
        statement.instrument_info.set_internal_ids(instrument_internal_ids.clone());

        let mut dividend_accruals = HashMap::new();
        let mut tax_accruals = HashMap::new();

        for (index, mut partial) in statements.into_iter().enumerate() {
            for (dividend_id, accruals) in partial.dividend_accruals.drain() {
                dividend_accruals.entry(dividend_id)
                    .and_modify(|existing: &mut DividendAccruals| existing.merge(&accruals))
                    .or_insert(accruals);
            }

            for (tax_id, accruals) in partial.tax_accruals.drain() {
                tax_accruals.entry(tax_id)
                    .and_modify(|existing: &mut TaxAccruals| existing.merge(&accruals))
                    .or_insert(accruals);
            }

            statement.merge(partial, last_period.last_date(), index == 0, index == last_index).map_err(|e| format!(
                "Failed to merge broker statements: {}", e))?;
        }

        for (dividend_id, accruals) in dividend_accruals {
            let instrument = statement.instrument_info.get_or_add_by_id(&dividend_id.issuer)?;
            let taxation_type = instrument.get_taxation_type(dividend_id.date, broker_jurisdiction)?;

            let (dividend, cash_flows) = process_dividend_accruals(
                dividend_id, &instrument.symbol, taxation_type, accruals, &mut tax_accruals, true)?;

            if let Some(dividend) = dividend {
                statement.dividends.push(dividend);
            }

            statement.cash_flows.extend(cash_flows.into_iter());
        }

        if !tax_accruals.is_empty() {
            let taxes = tax_accruals.keys()
                .map(|tax: &TaxId| format!(
                    "* {date}: {issuer}", date=formatting::format_date(tax.date),
                    issuer=tax.issuer))
                .collect::<Vec<_>>()
                .join("\n");

            let mut hint = String::new();
            if statement.broker.type_ == Broker::InteractiveBrokers {
                // https://github.com/KonishchevDmitry/investments/blob/master/docs/brokers.md#ib-tax-remapping
                let url = "https://bit.ly/investments-ib-tax-remapping";
                hint = format!("\n\nProbably manual tax remapping rules are required (see {})", url);
            }

            return Err!("Unable to find origin operations for the following taxes:\n{}{}", taxes, hint);
        }

        process_grants(&mut statement, strictness.contains(ReadingStrictness::GRANTS))?;

        for (symbol, new_symbol) in symbol_remapping.iter() {
            statement.rename_symbol(symbol, new_symbol, None, true).map_err(|e| format!(
                "Failed to remap {} to {}: {}", symbol, new_symbol, e))?;
        }

        for (symbol, new_symbol) in statement.instrument_info.suggest_remapping() {
            statement.rename_symbol(&symbol, &new_symbol, None, false).map_err(|e| format!(
                "Failed to apply automatically generated remapping rule {} -> {}: {}", symbol, new_symbol, e))?;
        }

        statement.corporate_actions.extend(corporate_actions.iter().cloned());

        for (symbol, name) in instrument_names {
            statement.instrument_info.get_or_add(symbol).set_name(name);
        }

        statement.validate(strictness)?;

        process_corporate_actions(&mut statement)?;
        statement.process_trades(None)?;

        statement.check_otc_instruments(strictness);
        statement.validate_tax_exemptions(tax_exemptions, strictness)?;

        Ok(statement)
    }

    fn new_empty_from(broker: BrokerInfo, statement: &PartialBrokerStatement) -> GenericResult<BrokerStatement> {
        let period = statement.get_period()?;

        if statement.get_has_starting_assets()? {
            return Err!(concat!(
                "The first broker statement ({}) has a non-zero starting assets. ",
                "Make sure that broker statements directory contains statements for all periods ",
                "starting from account opening",
            ), period.format());
        }

        Ok(BrokerStatement {
            broker, period,

            assets: NetAssets::default(),
            historical_assets: BTreeMap::new(),

            fees: Vec::new(),
            cash_flows: Vec::new(),
            deposits_and_withdrawals: Vec::new(),
            idle_cash_interest: Vec::new(),
            tax_agent_withholdings: TaxAgentWithholdings::new(),

            exchanges: Exchanges::new_empty(),
            forex_trades: Vec::new(),
            stock_buys: Vec::new(),
            stock_sells: Vec::new(),
            dividends: Vec::new(),

            stock_grants: Vec::new(),
            corporate_actions: Vec::new(),
            stock_splits: StockSplitController::default(),

            open_positions: HashMap::new(),
            instrument_info: InstrumentInfo::new(),
        })
    }

    pub fn check_date(&self) {
        let days = (time::today() - self.period.last_date()).num_days();
        let months = Decimal::from(days) / dec!(30);

        if months >= dec!(1) {
            warn!("{} broker statement is {} months old and may be outdated.",
                  self.broker.brief_name, util::round(months, 1));
        }
    }

    pub fn check_period_against_tax_year(&self, year: i32) -> GenericResult<Period> {
        let tax_period_start = date!(year, 1, 1);
        let tax_period_end = date!(year, 12, 31);

        if tax_period_end < self.period.first_date() || self.period.last_date() < tax_period_start {
            return Err!(concat!(
                "Period of the specified broker statement ({}) ",
                "doesn't overlap with the requested tax year ({})"),
                self.period.format(), year);
        }

        if self.period.last_date() < tax_period_end {
            warn!(concat!(
                "Period of the specified broker statement ({}) ",
                "doesn't fully overlap with the requested tax year ({})."
            ), self.period.format(), year);
        }

        Period::new(
            std::cmp::max(tax_period_start, self.period.first_date()),
            std::cmp::min(tax_period_end, self.period.last_date()),
        )
    }

    pub fn get_instrument_supposed_trading_mode(&self, symbol: &str) -> TradingMode {
        let exchanges = self.get_instrument_supposed_exchanges(symbol);
        exchanges.get_prioritized().first().unwrap().trading_mode()
    }

    pub fn batch_quotes(&self, quotes: &Quotes) -> EmptyResult {
        quotes.batch_all(self.open_positions.keys().map(|symbol| {
            self.get_quote_query(symbol)
        }))
    }

    pub fn get_quote_query(&self, symbol: &str) -> QuoteQuery {
        let exchanges = self.get_instrument_supposed_exchanges(symbol);
        QuoteQuery::Stock(symbol.to_owned(), exchanges.get_prioritized())
    }

    pub fn net_value(
        &self, converter: &CurrencyConverter, quotes: &Quotes, currency: &str, realtime: bool,
    ) -> GenericResult<Cash> {
        let mut net_value = self.assets.cash.clone();

        match self.assets.other {
            Some(other) if !realtime => {
                net_value.deposit(other);
            },
            _ => {
                self.batch_quotes(quotes)?;

                for (symbol, &quantity) in &self.open_positions {
                    let price = quotes.get(self.get_quote_query(symbol))?;
                    net_value.deposit(price * quantity);
                }
            },
        }

        Ok(Cash::new(currency, net_value.total_assets_real_time(currency, converter)?))
    }

    pub fn emulate_sell(
        &mut self, symbol: &str, quantity: Decimal, price: Cash,
        commission_calc: &mut CommissionCalc,
    ) -> EmptyResult {
        let trading_mode = self.get_instrument_supposed_trading_mode(symbol);

        let conclusion_time = crate::exchanges::today_trade_conclusion_time();
        let mut execution_date = trading_mode.execution_date(conclusion_time);

        for trade in self.stock_sells.iter().rev() {
            if trade.execution_date > execution_date {
                execution_date = trade.execution_date;
            }

            if trade.symbol == symbol {
                break
            }
        }

        let volume = price * quantity;
        let commission = commission_calc.add_trade(
            conclusion_time.date, TradeType::Sell, quantity, price)?;

        let stock_sell = StockSell::new_trade(
            symbol, quantity, price, volume, commission,
            conclusion_time, execution_date, true);

        if let Entry::Occupied(mut open_position) = self.open_positions.entry(symbol.to_owned()) {
            let available = open_position.get_mut();

            match quantity.cmp(available) {
                Ordering::Equal => {
                    open_position.remove();
                },
                Ordering::Less => {
                    *available = (*available - quantity).normalize();
                },
                Ordering::Greater => {
                    return Err!("The portfolio has not enough open positions for {}", symbol);
                },
            }
        } else {
            return Err!("The portfolio has no open {} position", symbol);
        }

        self.assets.cash.deposit(volume);
        self.assets.cash.withdraw(commission);
        self.stock_sells.push(stock_sell);

        Ok(())
    }

    pub fn emulate_commissions(&mut self, commission_calc: CommissionCalc) -> GenericResult<MultiCurrencyCashAccount> {
        let mut total = MultiCurrencyCashAccount::new();

        for commissions in commission_calc.calculate()?.values() {
            for commission in commissions.iter() {
                self.assets.cash.withdraw(commission);
                total.deposit(commission);
            }
        }

        Ok(total)
    }

    pub fn process_trades(&mut self, until: Option<DateOptTime>) -> EmptyResult {
        let mut unsold_buys: HashMap<String, Vec<usize>> = HashMap::new();

        for (index, stock_buy) in self.stock_buys.iter().enumerate().rev() {
            if let Some(time) = until {
                if stock_buy.conclusion_time >= time {
                    continue;
                }
            }

            if stock_buy.is_sold() {
                continue;
            }

            let symbol_buys = match unsold_buys.get_mut(&stock_buy.symbol) {
                Some(symbol_buys) => symbol_buys,
                None => unsold_buys.entry(stock_buy.symbol.clone()).or_default(),
            };

            symbol_buys.push(index);
        }

        for stock_sell in &mut self.stock_sells {
            if let Some(time) = until {
                if stock_sell.conclusion_time >= time {
                    continue;
                }
            }

            if stock_sell.is_processed() {
                continue;
            }

            let mut remaining_quantity = stock_sell.quantity;
            let mut sources = Vec::new();

            let symbol_buys = unsold_buys.get_mut(&stock_sell.symbol).ok_or_else(|| format!(
                "Error while processing {} position closing: There are no open positions for it",
                stock_sell.original_symbol
            ))?;

            while !remaining_quantity.is_zero() {
                let index = symbol_buys.last().copied().ok_or_else(|| format!(
                    "Error while processing {} position closing: There are no open positions for it",
                    stock_sell.original_symbol
                ))?;

                let stock_buy = &mut self.stock_buys[index];
                let multiplier = self.stock_splits.get_multiplier(
                    &stock_sell.symbol, stock_buy.conclusion_time, stock_sell.conclusion_time);

                let unsold_quantity = multiplier * stock_buy.get_unsold();
                let sell_quantity = std::cmp::min(remaining_quantity, unsold_quantity);
                assert!(sell_quantity > dec!(0));

                let source_quantity = (sell_quantity / multiplier).normalize();
                assert_eq!(source_quantity * multiplier, sell_quantity);

                sources.push(stock_buy.sell(source_quantity, multiplier));
                remaining_quantity -= sell_quantity;

                if stock_buy.is_sold() {
                    symbol_buys.pop();
                }
            }

            stock_sell.process(sources);
        }

        if until.is_none() {
            self.validate_open_positions()?;
        }

        Ok(())
    }

    fn get_instrument_supposed_exchanges(&self, symbol: &str) -> &Exchanges {
        match self.instrument_info.get(symbol) {
            Some(instrument) if !instrument.exchanges.is_empty() => &instrument.exchanges,
            _ => &self.exchanges,
        }
    }

    fn merge(
        &mut self, statement: PartialBrokerStatement, last_date: Date, first: bool, last: bool,
    ) -> EmptyResult {
        if !first {
            let period = statement.get_period()?;
            self.broker.statements_merging_strategy.validate(self.period, period, last_date)?;
            self.period = Period::new(self.period.first_date(), period.last_date()).unwrap();
        }

        if let partial::NetAssets{cash: Some(cash), other} = statement.assets {
            let assets = NetAssets{cash, other};
            self.assets = assets.clone();
            assert!(self.historical_assets.insert(self.period.last_date(), assets).is_none());
        } else if last {
            return Err!("Unable to find any information about current cash assets");
        }

        self.fees.extend(statement.fees);
        self.cash_flows.extend(statement.cash_flows);
        self.deposits_and_withdrawals.extend(statement.deposits_and_withdrawals);
        self.idle_cash_interest.extend(statement.idle_cash_interest);
        self.tax_agent_withholdings.merge(statement.tax_agent_withholdings);

        self.exchanges.merge(statement.exchanges);
        self.forex_trades.extend(statement.forex_trades);
        self.stock_buys.extend(statement.stock_buys);
        self.stock_sells.extend(statement.stock_sells);
        self.stock_grants.extend(statement.stock_grants);

        self.corporate_actions.extend(statement.corporate_actions);
        self.open_positions = statement.open_positions;
        self.instrument_info.merge(statement.instrument_info);

        Ok(())
    }

    fn rename_symbol(&mut self, symbol: &str, new_symbol: &str, time: Option<DateOptTime>, check_existence: bool) -> EmptyResult {
        // For now don't introduce any enums here:
        // * When date is set - it's always a corporate action.
        // * In other case it's a manual remapping.
        let remapping = if let Some(time) = time {
            debug!("Renaming {symbol} -> {new_symbol} due to corporate action from {}...", formatting::format_date(time.date));
            false
        } else {
            debug!("Remapping {symbol} -> {new_symbol}...");
            true
        };

        let mut found = false;
        let mut rename = |operation_time: DateOptTime, operation_symbol: &mut String, operation_original_symbol: &mut String| {
            if let Some(time) = time {
                if operation_time > time {
                    return;
                }
            }

            if *operation_symbol == symbol {
                new_symbol.clone_into(operation_symbol);
                found = true;
            }

            if remapping {
                if *operation_original_symbol == symbol {
                    new_symbol.clone_into(operation_original_symbol);
                    found = true;
                }
            }
        };

        if remapping {
            if let Some(quantity) = self.open_positions.remove(symbol) {
                match self.open_positions.entry(new_symbol.to_owned()) {
                    Entry::Occupied(_) => {
                        self.open_positions.insert(symbol.to_owned(), quantity);
                        return Err!("The portfolio already has {new_symbol} symbol");
                    },
                    Entry::Vacant(entry) => {
                        entry.insert(quantity);
                    },
                }
            }

            self.instrument_info.remap(symbol, new_symbol)?;
        } else {
            self.stock_splits.rename(symbol, new_symbol)?;
        }

        for trade in &mut self.stock_buys {
            rename(trade.conclusion_time, &mut trade.symbol, &mut trade.original_symbol);
        }

        for trade in &mut self.stock_sells {
            rename(trade.conclusion_time, &mut trade.symbol, &mut trade.original_symbol);
        }

        for dividend in &mut self.dividends {
            rename(dividend.date.into(), &mut dividend.issuer, &mut dividend.original_issuer);
        }

        if remapping {
            for cash_flow in &mut self.cash_flows {
                if let Some(original_symbol) = cash_flow.mut_symbol() {
                    if *original_symbol == symbol {
                        new_symbol.clone_into(original_symbol);
                    }
                }
            }
        }

        if check_existence && !found {
            return Err!("Unable to find any operation with it in the broker statement");
        }

        Ok(())
    }

    fn validate(&mut self, strictness: ReadingStrictness) -> EmptyResult {
        let validator = DateValidator::new(self.period);

        validator.sort_and_validate(
            "a deposit of withdrawal", &mut self.deposits_and_withdrawals,
            |cash_flow| cash_flow.date)?;

        self.sort_and_alter_fees(self.period.last_date());
        validator.validate("a fee", &self.fees, |fee| fee.date)?;

        if
            strictness.contains(ReadingStrictness::REPO_TRADES) &&
            self.cash_flows.iter().any(|cash_flow| matches!(cash_flow.type_, CashFlowType::Repo{..}))
        {
            warn!(concat!(
                "Broker statement contains repo trades which aren't supported yet. ",
                "All repo trades will be ignored during the calculations."
            ));
        }

        self.cash_flows.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));
        validator.validate("a cash flow", &self.cash_flows, |cash_flow| cash_flow.date)?;

        validator.sort_and_validate(
            "an idle cash interest", &mut self.idle_cash_interest, |interest| interest.date)?;

        self.tax_agent_withholdings.sort_and_validate(&validator)?;

        validator.sort_and_validate(
            "a forex trade", &mut self.forex_trades, |trade| trade.conclusion_time)?;

        self.sort_and_validate_stock_buys()?;
        self.sort_and_validate_stock_sells()?;
        validator.sort_and_validate("a stock grant", &mut self.stock_grants, |grant| grant.date)?;

        self.dividends.sort_by(|a, b| (a.date, &a.issuer).cmp(&(b.date, &b.original_issuer)));
        validator.validate("a dividend", &self.dividends, |dividend| dividend.date)?;

        validator.sort_and_validate(
            "a corporate action", &mut self.corporate_actions, |action| action.time)?;

        Ok(())
    }

    fn sort_and_alter_fees(&mut self, max_date: Date) {
        if self.broker.allow_future_fees {
            for fee in &mut self.fees {
                if fee.date > max_date && self.exchanges.get_prioritized().iter().any(|exchange| {
                    exchange.is_valid_execution_date(max_date, fee.date)
                }) {
                    fee.date = max_date;
                }
            }
        }

        self.fees.sort_by_key(|fee| fee.date);
    }

    fn sort_and_validate_stock_buys(&mut self) -> EmptyResult {
        let date_validator = DateValidator::new(self.period);
        sort_and_validate_trades("buy", &mut self.stock_buys)?;
        date_validator.validate("a stock buy", &self.stock_buys, |trade| trade.conclusion_time)
    }

    fn sort_and_validate_stock_sells(&mut self) -> EmptyResult {
        let date_validator = DateValidator::new(self.period);
        sort_and_validate_trades("sell", &mut self.stock_sells)?;
        date_validator.validate("a stock sell", &self.stock_sells, |trade| trade.conclusion_time)
    }

    fn check_otc_instruments(&mut self, strictness: ReadingStrictness) {
        if !strictness.contains(ReadingStrictness::OTC_INSTRUMENTS) {
            return;
        }

        // We can't balance losses and profits between securities traded on organized securities market and securities
        // that aren't traded on organized securities market (see Article 220.1 of the Tax Code of the Russian
        // Federation or https://www.nalog.gov.ru/rn77/taxation/taxes/ndfl/nalog_vichet/nv_ubit/ details), but we don't
        // know for sure whether the stock marked as OTC in broker statement traded or not. So for now just show the
        // warning about all OTC stocks.
        //
        // Instrument info may be non-deduplicated due to its representation issues in broker statement, so use trades
        // here as a source of all instrument symbols.

        let otc_stocks = self.stock_buys.iter().map(|trade| &trade.symbol)
            .chain(self.stock_sells.iter().map(|trade| &trade.symbol))
            .collect::<BTreeSet<_>>().into_iter()
            .filter(|symbol| {
                self.instrument_info.get(symbol)
                    .map(|instrument| instrument.exchanges.get_prioritized().contains(&Exchange::Otc))
                    .unwrap_or(false)
            })
            .join(", ");

        if !otc_stocks.is_empty() {
            warn!(concat!(
                "Broker statement contains the following OTC stocks: {}. ",
                "Tax calculations or losses and profits balancing for OTC trades may be incorrect, so be critical to them."
            ), otc_stocks);
        }
    }

    fn validate_tax_exemptions(&mut self, tax_exemptions: &[TaxExemption], strictness: ReadingStrictness) -> EmptyResult {
        if !strictness.contains(ReadingStrictness::TAX_EXEMPTIONS) || !tax_exemptions.contains(&TaxExemption::LongTermOwnership) {
            return Ok(());
        }

        let mut unknown = BTreeSet::new();

        for trade in &self.stock_sells {
            let instrument = self.instrument_info.get_or_empty(&trade.symbol);
            if long_term_ownership::is_applicable(&instrument.isin, trade.execution_date).is_none() {
                unknown.insert(&trade.symbol);
            }
        }

        for trade in &self.stock_buys {
            if trade.is_sold() {
                continue;
            }

            let instrument = self.instrument_info.get_or_empty(&trade.symbol);
            let execution_date = self.get_instrument_supposed_trading_mode(&trade.symbol).execution_date(time::today());

            if long_term_ownership::is_applicable(&instrument.isin, execution_date).is_none() {
                unknown.insert(&trade.symbol);
            }
        }

        if !unknown.is_empty() {
            warn!(concat!(
                "Unable to determine long-term ownership tax exemption applicability for the following stocks: {}. ",
                "Assuming them non-applicable."
            ), unknown.iter().join(", "));
        }

        Ok(())
    }

    fn validate_open_positions(&self) -> EmptyResult {
        let mut open_positions: HashMap<&str, Decimal> = HashMap::new();

        for stock_buy in &self.stock_buys {
            if stock_buy.is_sold() {
                continue;
            }

            let multiplier = self.stock_splits.get_multiplier(
                &stock_buy.symbol, stock_buy.conclusion_time,
                DateOptTime::new_max_time(self.period.last_date()));

            let quantity = multiplier * stock_buy.get_unsold();

            open_positions.entry(&stock_buy.symbol)
                .and_modify(|position| *position += quantity)
                .or_insert(quantity);
        }

        let symbols: BTreeSet<&str> = self.open_positions.keys().map(String::as_str)
            .chain(open_positions.keys().copied())
            .collect();

        for &symbol in &symbols {
            let calculated = open_positions.get(symbol);
            let actual = self.open_positions.get(symbol);

            if calculated != actual {
                let calculated = calculated.copied().unwrap_or_default();
                let actual = actual.copied().unwrap_or_default();

                return Err!(concat!(
                    "Calculated open positions don't match declared ones in the statement: ",
                    "{}: {} vs {}"
                ), symbol, calculated, actual);
            }
        }

        Ok(())
    }
}

#[derive(Clone, Default)]
pub struct NetAssets {
    pub cash: MultiCurrencyCashAccount,
    pub other: Option<Cash>, // Supported only for some brokers
}