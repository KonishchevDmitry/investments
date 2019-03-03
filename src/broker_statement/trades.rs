use crate::core::GenericResult;
use crate::currency::{self, Cash};
use crate::currency::converter::CurrencyConverter;
use crate::formatting;
use crate::localities::Country;
use crate::types::{Date, Decimal};

#[derive(Debug)]
pub struct StockBuy {
    pub symbol: String,
    pub quantity: u32,
    pub price: Cash,
    pub commission: Cash,

    pub conclusion_date: Date,
    pub execution_date: Date,

    sold: u32,
}

impl StockBuy {
    pub fn new(
        symbol: &str, quantity: u32, price: Cash, commission: Cash,
        conclusion_date: Date, execution_date: Date,
    ) -> StockBuy {
        StockBuy {
            symbol: symbol.to_owned(), quantity, price, commission,
            conclusion_date, execution_date, sold: 0,
        }
    }

    pub fn is_sold(&self) -> bool {
        self.sold == self.quantity
    }

    pub fn get_unsold(&self) -> u32 {
        self.quantity - self.sold
    }

    pub fn sell(&mut self, quantity: u32) {
        assert!(self.get_unsold() >= quantity);
        self.sold += quantity;
    }
}

#[derive(Clone, Debug)]
pub struct StockSell {
    pub symbol: String,
    pub quantity: u32,
    pub price: Cash,
    pub commission: Cash,

    pub conclusion_date: Date,
    pub execution_date: Date,

    pub emulation: bool,
    sources: Vec<StockSellSource>,
}

impl StockSell {
    pub fn new(
        symbol: &str, quantity: u32, price: Cash, commission: Cash,
        conclusion_date: Date, execution_date: Date, emulation: bool,
    ) -> StockSell {
        StockSell {
            symbol: symbol.to_owned(), quantity, price, commission,
            conclusion_date, execution_date, emulation, sources: Vec::new(),
        }
    }

    pub fn is_processed(&self) -> bool {
        !self.sources.is_empty()
    }

    pub fn process(&mut self, sources: Vec<StockSellSource>) {
        assert!(!self.is_processed());
        assert_eq!(sources.iter().map(|source| source.quantity).sum::<u32>(), self.quantity);
        self.sources = sources;
    }

    pub fn calculate(&self, country: &Country, converter: &CurrencyConverter) -> GenericResult<SellDetails> {
        Ok(self.calculate_impl(country, converter).map_err(|e| format!(
            "Failed calculate results of {} selling order from {}: {}",
            self.symbol, formatting::format_date(self.conclusion_date), e))?)
    }

    fn calculate_impl(&self, country: &Country, converter: &CurrencyConverter) -> GenericResult<SellDetails> {
        // FIXME: We need to use exactly the same rounding logic as in tax statement

        let revenue = (self.price * self.quantity).round();
        let local_revenue = converter.convert_to_cash(
            self.execution_date, revenue, country.currency)?.round();

        let local_commission = converter.convert_to_cash(
            self.conclusion_date, self.commission, country.currency)?.round();

        let mut total_cost = self.commission;
        let mut total_local_cost = local_commission;

        let mut purchase_cost = Cash::new(total_cost.currency, dec!(0));
        let mut purchase_local_cost = Cash::new(total_local_cost.currency, dec!(0));

        let mut fifo = Vec::new();

        for source in &self.sources {
            let fifo_details = source.calculate(country, converter)?;

            purchase_cost.add_assign(fifo_details.total_cost).map_err(|e| format!(
                "Sell and buy trade have different currency: {}", e))?;
            purchase_local_cost.add_assign(fifo_details.total_local_cost).unwrap();

            fifo.push(fifo_details);
        }

        total_cost.add_assign(purchase_cost).map_err(|e| format!(
            "Sell and buy trade have different currency: {}", e))?;
        total_local_cost.add_assign(purchase_local_cost).unwrap();

        let profit = revenue.sub(total_cost).map_err(|e| format!(
            "Sell and buy trade have different currency: {}", e))?;

        let local_profit = local_revenue.sub(total_local_cost).unwrap();
        let tax_to_pay = Cash::new(country.currency, country.tax_to_pay(local_profit.amount, None));

        let real_profit = profit.sub_convert(self.execution_date, tax_to_pay, converter)?;
        let real_profit_ratio = real_profit.div(purchase_cost).unwrap();

        let real_local_profit = local_profit.sub(tax_to_pay).unwrap();
        let real_local_profit_ratio = real_local_profit.div(purchase_local_cost).unwrap();

        Ok(SellDetails {
            revenue,
            local_revenue: local_revenue,
            local_commission: local_commission,

            purchase_cost: purchase_cost,
            purchase_local_cost: purchase_local_cost,

            total_cost: total_cost,
            total_local_cost: total_local_cost,

            profit,
            local_profit: local_profit,
            tax_to_pay: tax_to_pay,

            real_profit_ratio,
            real_local_profit_ratio,

            fifo: fifo,
        })
    }

    // FIXME: Deprecate
    pub fn tax_to_pay(&self, country: &Country, converter: &CurrencyConverter) -> GenericResult<Decimal> {
        Ok(self.calculate(country, converter)?.tax_to_pay.amount)
    }
}

#[derive(Clone, Debug)]
pub struct StockSellSource {
    pub quantity: u32,
    pub price: Cash,
    pub commission: Cash,

    pub conclusion_date: Date,
    pub execution_date: Date,
}

impl StockSellSource {
    fn calculate(&self, country: &Country, converter: &CurrencyConverter) -> GenericResult<FifoDetails> {
        let cost = (self.price * self.quantity).round();
        let local_cost = currency::round(converter.convert_to(
            self.execution_date, cost, country.currency)?);

        let local_commission = currency::round(converter.convert_to(
            self.conclusion_date, self.commission, country.currency)?);

        let mut total_cost = cost;
        let mut total_local_cost = local_cost;

        total_cost.add_assign(self.commission).map_err(|e| format!(
            "Trade and commission have different currency: {}", e))?;
        total_local_cost += local_commission;

        Ok(FifoDetails {
            quantity: self.quantity,
            price: self.price,

            commission: self.commission,
            local_commission: Cash::new(country.currency, local_commission),

            conclusion_date: self.conclusion_date,
            execution_date: self.execution_date,

            cost,
            local_cost: Cash::new(country.currency, local_cost),

            total_cost,
            total_local_cost: Cash::new(country.currency, total_local_cost),
        })
    }
}

pub struct SellDetails {
    pub revenue: Cash,
    pub local_revenue: Cash,
    pub local_commission: Cash,

    pub purchase_cost: Cash,
    pub purchase_local_cost: Cash,

    pub total_cost: Cash,
    pub total_local_cost: Cash,

    pub profit: Cash,
    pub local_profit: Cash,
    pub tax_to_pay: Cash,

    pub real_profit_ratio: Decimal,
    pub real_local_profit_ratio: Decimal,

    pub fifo: Vec<FifoDetails>,
}

pub struct FifoDetails {
    pub quantity: u32,
    pub price: Cash,

    pub commission: Cash,
    pub local_commission: Cash,

    pub conclusion_date: Date,
    pub execution_date: Date,

    pub cost: Cash,
    pub local_cost: Cash,

    pub total_cost: Cash,
    pub total_local_cost: Cash,
}