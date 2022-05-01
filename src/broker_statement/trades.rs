use crate::core::GenericResult;
use crate::currency::Cash;
use crate::currency::converter::CurrencyConverter;
use crate::formatting;
use crate::localities::Country;
use crate::taxes::{self, IncomeType, LtoDeductibleProfit, TaxExemption};
use crate::time::DateOptTime;
use crate::trades::{self, RealProfit};
use crate::types::{Date, Decimal};

pub struct ForexTrade {
    pub from: Cash,
    pub to: Cash,
    pub commission: Cash,
    pub conclusion_time: DateOptTime,
}

impl ForexTrade {
    pub fn new(time: DateOptTime, from: Cash, to: Cash, commission: Cash) -> ForexTrade {
        ForexTrade {from, to, commission, conclusion_time: time}
    }
}

#[derive(Clone, Copy)]
pub enum StockSource {
    // Ordinary trade
    Trade {
        price: Cash,
        volume: Cash, // May be slightly different from price * quantity due to rounding on broker side
        commission: Cash,
    },

    // Non-trade operation due to a corporate action that doesn't affect cash balance:
    // * Emulated buy to convert position during stock split
    // * Spinoff or stock dividend
    CorporateAction,

    // Stock grants are emulated now via zero cost buys
    Grant,
}

pub struct StockBuy {
    pub symbol: String,
    pub original_symbol: String,

    pub quantity: Decimal,
    pub type_: StockSource,
    cost: PurchaseTotalCost,

    pub conclusion_time: DateOptTime,
    pub execution_date: Date,
    pub out_of_order_execution: bool,

    sold: Decimal,
}

impl StockBuy {
    pub fn new_trade(
        symbol: &str, quantity: Decimal, price: Cash, volume: Cash, commission: Cash,
        conclusion_time: DateOptTime, execution_date: Date,
    ) -> StockBuy {
        let cost = PurchaseTotalCost::new_from_trade(
            conclusion_time.date, execution_date, volume, commission);

        StockBuy {
            symbol: symbol.to_owned(), original_symbol: symbol.to_owned(),
            quantity, type_: StockSource::Trade {price, volume, commission}, cost,
            conclusion_time, execution_date, out_of_order_execution: false,
            sold: dec!(0),
        }
    }

    pub fn new_grant(date: Date, symbol: &str, quantity: Decimal) -> StockBuy {
        StockBuy {
            symbol: symbol.to_owned(), original_symbol: symbol.to_owned(),
            quantity, type_: StockSource::Grant, cost: PurchaseTotalCost::new(),
            out_of_order_execution: true, conclusion_time: date.into(), execution_date: date,
            sold: dec!(0),
        }
    }

    pub fn new_corporate_action(
        symbol: &str, quantity: Decimal, cost: PurchaseTotalCost,
        conclusion_time: DateOptTime, execution_date: Date,
    ) -> StockBuy {
        StockBuy {
            symbol: symbol.to_owned(), original_symbol: symbol.to_owned(),
            quantity, type_: StockSource::CorporateAction, cost, out_of_order_execution: true,
            conclusion_time, execution_date, sold: dec!(0),
        }
    }

    pub fn is_sold(&self) -> bool {
        self.sold == self.quantity
    }

    pub fn get_unsold(&self) -> Decimal {
        self.quantity - self.sold
    }

    pub fn sell(&mut self, quantity: Decimal, multiplier: Decimal) -> StockSellSource {
        assert!(self.get_unsold() >= quantity);
        self.sold += quantity;

        let mut cost = self.cost.clone();
        let type_ = if quantity == self.quantity {
            self.type_
        } else {
            for cost in &mut cost.0 {
                cost.fraction.0 *= quantity;
                cost.fraction.1 *= self.quantity;
            }

            match self.type_ {
                StockSource::Trade {price, commission, ..} => StockSource::Trade {
                    price,
                    volume: price * quantity,
                    commission: commission / self.quantity * quantity,
                },
                StockSource::CorporateAction | StockSource::Grant => self.type_,
            }
        };

        StockSellSource {
            original_symbol: self.original_symbol.clone(),
            quantity, multiplier, type_, cost,
            conclusion_time: self.conclusion_time,
            execution_date: self.execution_date,
        }
    }
}

#[derive(Clone, Copy)]
pub enum StockSellType {
    // Any trade operation:
    // * Ordinary trade
    // * Corporate action which leads to position closing
    Trade {
        // Please note that all these values may be zero
        price: Cash,
        volume: Cash, // May be slightly different from price * quantity due to rounding on broker side
        commission: Cash,
    },

    // Non-trade operation due to a corporate action that doesn't affect cash balance and doesn't
    // lead to any taxes:
    // * Emulated sell to convert position during stock split
    CorporateAction,
}

#[derive(Clone)]
pub struct StockSell {
    pub symbol: String,
    pub original_symbol: String,

    pub quantity: Decimal,
    pub type_: StockSellType,

    pub conclusion_time: DateOptTime,
    pub execution_date: Date,
    pub out_of_order_execution: bool,

    pub emulation: bool,
    sources: Vec<StockSellSource>,
}

impl StockSell {
    pub fn new_trade(
        symbol: &str, quantity: Decimal, price: Cash, volume: Cash, commission: Cash,
        conclusion_time: DateOptTime, execution_date: Date, emulation: bool,
    ) -> StockSell {
        StockSell {
            symbol: symbol.to_owned(), original_symbol: symbol.to_owned(),
            quantity, type_: StockSellType::Trade {price, volume, commission},
            conclusion_time, execution_date, out_of_order_execution: false,
            emulation, sources: Vec::new(),
        }
    }

    pub fn new_corporate_action(
        symbol: &str, quantity: Decimal, conclusion_time: DateOptTime, execution_date: Date,
    ) -> StockSell {
        StockSell {
            symbol: symbol.to_owned(), original_symbol: symbol.to_owned(),
            quantity, type_: StockSellType::CorporateAction,
            conclusion_time, execution_date, out_of_order_execution: true,
            emulation: false, sources: Vec::new(),
        }
    }

    pub fn is_processed(&self) -> bool {
        !self.sources.is_empty()
    }

    pub fn process(&mut self, sources: Vec<StockSellSource>) {
        assert!(!self.is_processed());
        assert_eq!(
            sources.iter()
                .map(|source| source.multiplier * source.quantity)
                .sum::<Decimal>(),
            self.quantity,
        );
        self.sources = sources;
    }

    pub fn calculate(
        &self, country: &Country, tax_year: i32, tax_exemptions: &[TaxExemption],
        converter: &CurrencyConverter,
    ) -> GenericResult<SellDetails> {
        Ok(self.calculate_impl(country, tax_year, tax_exemptions, converter).map_err(|e| format!(
            "Failed to calculate results of {} selling order from {}: {}",
            self.original_symbol, formatting::format_date(self.conclusion_time), e))?)
    }

    fn calculate_impl(
        &self, country: &Country, tax_year: i32, tax_exemptions: &[TaxExemption],
        converter: &CurrencyConverter,
    ) -> GenericResult<SellDetails> {
        let (price, volume, commission) = match self.type_ {
            StockSellType::Trade {price, volume, commission} => (price, volume, commission),
            _ => unreachable!(),
        };

        let currency = price.currency;
        let local_conclusion = |value| converter.convert_to_cash_rounding(
            self.conclusion_time.date, value, country.currency);
        let local_execution = |value| converter.convert_to_cash_rounding(
            self.execution_date, value, country.currency);

        let mut purchase_cost = Cash::zero(currency);
        let mut purchase_local_cost = Cash::zero(country.currency);
        let mut deductible_purchase_local_cost = Cash::zero(country.currency);

        let mut fifo = Vec::new();
        let mut total_quantity = dec!(0);
        let mut tax_free_quantity = dec!(0);

        for source in &self.sources {
            let source_quantity = source.quantity * source.multiplier;

            let mut source_details = FifoDetails::new(source, country, converter)?;
            let source_total_cost = source_details.total_cost(currency, converter)?;
            let source_total_local_cost = source_details.total_cost(country.currency, converter)?;

            for tax_exemption in tax_exemptions {
                match tax_exemption {
                    TaxExemption::LongTermOwnership => {
                        if let Some(years) = taxes::long_term_ownership::is_deductible(source.execution_date, self.execution_date) {
                            let source_local_revenue = local_execution(price * source_quantity)?;
                            let source_local_commission = local_conclusion(
                                commission * source_quantity / self.quantity)?;

                            let source_local_profit = source_local_revenue
                                - source_local_commission
                                - source_total_local_cost;

                            source_details.long_term_ownership_deductible.replace(LtoDeductibleProfit {
                                profit: std::cmp::max(dec!(0), source_local_profit.amount),
                                years,
                            });
                            break;
                        }
                    },
                    TaxExemption::TaxFree => {
                        source_details.tax_exemption_applied = true;
                        break;
                    },
                };
            }

            total_quantity += source_quantity;
            if source_details.tax_exemption_applied {
                tax_free_quantity += source_quantity;
            }

            purchase_cost += source_total_cost;
            purchase_local_cost += source_total_local_cost;
            if !source_details.tax_exemption_applied {
                deductible_purchase_local_cost += source_total_local_cost;
            }

            fifo.push(source_details);
        }

        assert_eq!(total_quantity, self.quantity);
        let taxable_ratio = (total_quantity - tax_free_quantity) / total_quantity;

        let revenue = volume.round();
        let local_revenue = local_execution(revenue)?;
        let taxable_local_revenue = local_execution(revenue * taxable_ratio)?;

        let local_commission = local_conclusion(commission)?;
        let deductible_local_commission = local_conclusion(commission * taxable_ratio)?;

        let total_cost = purchase_cost + converter.convert_to_cash_rounding(
            self.conclusion_time.date, commission, currency)?;
        let total_local_cost = purchase_local_cost + local_commission;
        let deductible_total_local_cost = deductible_purchase_local_cost + deductible_local_commission;

        let profit = revenue - total_cost;
        let local_profit = local_revenue - total_local_cost;
        let taxable_local_profit = taxable_local_revenue - deductible_total_local_cost;

        let tax_without_deduction = country.tax_to_pay(
            IncomeType::Trading, tax_year, local_profit, None);
        let tax_to_pay = country.tax_to_pay(
            IncomeType::Trading, tax_year, taxable_local_profit, None);
        let tax_deduction = tax_without_deduction - tax_to_pay;
        assert!(!tax_deduction.is_negative());

        Ok(SellDetails {
            execution_date: self.execution_date,

            revenue,
            local_revenue,
            local_commission,

            purchase_cost,
            purchase_local_cost,
            total_local_cost,

            profit,
            local_profit,
            taxable_local_profit,

            tax_to_pay,
            tax_deduction,

            fifo,
        })
    }
}

#[derive(Clone)]
pub struct StockSellSource {
    pub original_symbol: String,

    pub quantity: Decimal,
    pub multiplier: Decimal,

    pub type_: StockSource,
    pub cost: PurchaseTotalCost,

    pub conclusion_time: DateOptTime,
    pub execution_date: Date,
}

pub struct SellDetails {
    execution_date: Date,

    pub revenue: Cash,
    pub local_revenue: Cash,
    pub local_commission: Cash,

    // Please note that all of the following values can be zero due to corporate actions or other
    // non-trade operations:
    pub purchase_cost: Cash,
    pub purchase_local_cost: Cash,
    pub total_local_cost: Cash,

    pub profit: Cash,
    pub local_profit: Cash,
    pub taxable_local_profit: Cash,

    pub tax_to_pay: Cash,
    pub tax_deduction: Cash,

    pub fifo: Vec<FifoDetails>,
}

impl SellDetails {
    pub fn tax_exemption_applied(&self) -> bool {
        if self.fifo.iter().any(|trade| trade.tax_exemption_applied) {
            return true;
        }

        assert_eq!(self.taxable_local_profit, self.local_profit);
        assert!(self.tax_deduction.is_zero());
        false
    }

    pub fn real_profit(&self, converter: &CurrencyConverter) -> GenericResult<RealProfit> {
        trades::calculate_real_profit(
            self.execution_date, self.purchase_cost, self.purchase_local_cost,
            self.profit, self.local_profit, self.tax_to_pay, converter)
    }
}

pub struct FifoDetails {
    pub original_symbol: String,

    pub quantity: Decimal,
    pub multiplier: Decimal,

    pub conclusion_time: DateOptTime,
    pub execution_date: Date,

    pub source: StockSourceDetails,
    cost: PurchaseTotalCost,

    pub tax_exemption_applied: bool,
    pub long_term_ownership_deductible: Option<LtoDeductibleProfit>,
}

pub enum StockSourceDetails {
    Trade {
        price: Cash,

        commission: Cash,
        local_commission: Cash,

        cost: Cash,
        local_cost: Cash,
    },
    CorporateAction,
    Grant,
}

impl FifoDetails {
    fn new(source: &StockSellSource, country: &Country, converter: &CurrencyConverter) -> GenericResult<FifoDetails> {
        let details = match source.type_ {
            StockSource::Trade {price, volume, mut commission} => {
                let cost = volume.round();
                let local_cost = converter.convert_to_cash_rounding(
                    source.execution_date, cost, country.currency)?;

                commission = commission.round();
                let local_commission = converter.convert_to_cash_rounding(
                    source.conclusion_time.date, commission, country.currency)?;

                StockSourceDetails::Trade {
                    price,

                    commission,
                    local_commission,

                    cost,
                    local_cost,
                }
            },
            StockSource::CorporateAction => StockSourceDetails::CorporateAction,
            StockSource::Grant => StockSourceDetails::Grant,
        };

        Ok(FifoDetails {
            original_symbol: source.original_symbol.clone(),

            quantity: source.quantity,
            multiplier: source.multiplier,

            conclusion_time: source.conclusion_time,
            execution_date: source.execution_date,

            source: details,
            cost: source.cost.clone(),

            tax_exemption_applied: false,
            long_term_ownership_deductible: None,
        })
    }

    // Please note that all of the following values can be zero due to corporate actions or other
    // non-trade operations:

    pub fn price(&self, currency: &str, converter: &CurrencyConverter) -> GenericResult<Cash> {
        Ok(match self.source {
            StockSourceDetails::Trade {price, ..} if price.currency == currency => price,
            _ => {
                let cost = self.cost(currency, converter)?;
                trades::calculate_price(self.quantity, cost)?
            },
        })
    }

    pub fn cost(&self, currency: &str, converter: &CurrencyConverter) -> GenericResult<Cash> {
        self.cost.calculate(Some(PurchaseCostType::Trade), currency, converter)
    }

    pub fn total_cost(&self, currency: &str, converter: &CurrencyConverter) -> GenericResult<Cash> {
        self.cost.calculate(None, currency, converter)
    }
}

// On stock split we generate fake sell+buy transactions for position conversion, but it gets us
// into currency revaluation issues, so we have to keep original date+amount pairs for proper
// calculation in other currencies.
//
// Please note that it may be zero due to corporate actions or other non-trade operations.
#[derive(Clone)]
pub struct PurchaseTotalCost(Vec<PurchaseCost>);

impl PurchaseTotalCost {
    pub fn new() -> PurchaseTotalCost {
        PurchaseTotalCost(Vec::new())
    }

    fn new_from_trade(conclusion_date: Date, execution_date: Date, volume: Cash, commission: Cash) -> PurchaseTotalCost {
        let mut transactions = Vec::new();

        if !volume.is_zero() {
            transactions.push(PurchaseTransaction::new(
                execution_date, PurchaseCostType::Trade, volume));
        }

        if !commission.is_zero() {
            transactions.push(PurchaseTransaction::new(
                conclusion_date, PurchaseCostType::Commission, commission));
        }

        PurchaseTotalCost(vec![
            PurchaseCost {
                transactions: transactions,
                fraction: Fraction(dec!(1), dec!(1)),
            }
        ])
    }

    pub fn add(&mut self, cost: &PurchaseTotalCost) {
        self.0.extend(cost.0.iter().map(Clone::clone))
    }

    fn calculate(&self, type_: Option<PurchaseCostType>, currency: &str, converter: &CurrencyConverter) -> GenericResult<Cash> {
        let mut total_cost = dec!(0);

        for cost in &self.0 {
            let mut purchase_cost = dec!(0);

            for transaction in &cost.transactions {
                match type_ {
                    Some(type_) if type_ != transaction.type_ => continue,
                    _ => {},
                };

                let transaction_cost = transaction.cost / cost.fraction.1 * cost.fraction.0;
                purchase_cost += converter.convert_to_rounding(
                    transaction.date, transaction_cost, currency)?;
            }

            total_cost += purchase_cost;
        }

        Ok(Cash::new(currency, total_cost.normalize()))
    }
}

#[derive(Clone)]
struct PurchaseCost {
    transactions: Vec<PurchaseTransaction>,
    fraction: Fraction,
}

#[derive(Clone, Copy)]
struct Fraction(Decimal, Decimal);

#[derive(Clone, Copy)]
struct PurchaseTransaction {
    date: Date,
    type_: PurchaseCostType,
    cost: Cash,
}

#[derive(Clone, Copy, PartialEq)]
enum PurchaseCostType {
    Trade,
    Commission,
}

impl PurchaseTransaction {
    fn new(date: Date, type_: PurchaseCostType, cost: Cash) -> PurchaseTransaction {
        PurchaseTransaction {date, type_, cost}
    }
}