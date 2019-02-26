use crate::core::GenericResult;
use crate::currency::Cash;
use crate::currency::converter::CurrencyConverter;
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
        // FIXME: We need to use exactly the same rounding logic as in tax statement

        let mut cost = self.commission;
        let mut local_cost = converter.convert_to(self.conclusion_date, cost, country.currency)?;

        let revenue = self.price * self.quantity;
        let local_revenue = converter.convert_to(self.execution_date, revenue, country.currency)?;

        let mut fifo = Vec::new();

        for source in &self.sources {
            let mut purchase_cost = source.price * source.quantity;
            let mut purchase_local_cost = converter.convert_to(
                source.execution_date, purchase_cost, country.currency)?;

            purchase_cost.add_assign(source.commission).map_err(|e| format!(
                "Trade and commission have different currency: {}", e))?;

            purchase_local_cost += converter.convert_to(
                source.conclusion_date, source.commission, country.currency)?;

            cost.add_assign(purchase_cost).map_err(|e| format!(
                "Sell and buy trade have different currency: {}", e))?;

            local_cost += purchase_local_cost;

            fifo.push(FifoDetails {
                quantity: source.quantity,
                price: source.price,

                purchase_cost,
                purchase_local_cost: Cash::new(country.currency, purchase_local_cost),
            });
        }

        let profit = revenue.sub(cost).map_err(|e| format!(
            "Sell and buy trade have different currency: {}", e))?;

        let local_profit = local_revenue - local_cost;
        let tax_to_pay = country.tax_to_pay(local_profit, None);

        Ok(SellDetails {
            cost,
            local_cost: Cash::new(country.currency, local_cost),

            revenue,
            local_revenue: Cash::new(country.currency, local_revenue),

            profit,
            local_profit: Cash::new(country.currency, local_profit),
            tax_to_pay: Cash::new(country.currency, tax_to_pay),

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

pub struct SellDetails {
    pub cost: Cash,
    pub local_cost: Cash,

    pub revenue: Cash,
    pub local_revenue: Cash,

    pub profit: Cash,
    pub local_profit: Cash,
    pub tax_to_pay: Cash,

    pub fifo: Vec<FifoDetails>,
}

pub struct FifoDetails {
    pub quantity: u32,
    pub price: Cash,

    pub purchase_cost: Cash,
    pub purchase_local_cost: Cash,
}