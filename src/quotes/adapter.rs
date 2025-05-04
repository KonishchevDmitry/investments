use crate::core::GenericResult;
use crate::time::{Date, Period};

use super::{QuotesProvider, SupportedExchange, QuotesMap, HistoricalQuotes};

pub struct QuotesProviderAdapter<P: QuotesProvider> {
    provider: P,
    historical_only: bool,
    historical_until: Option<Date>,
}

impl<P: QuotesProvider> QuotesProviderAdapter<P> {
    pub fn new(provider: P) -> Self {
        QuotesProviderAdapter{
            provider,
            historical_only: false,
            historical_until: None,
        }
    }

    pub fn historical_only(mut self) -> Self {
        self.historical_only = true;
        self
    }

    pub fn historical_until(mut self, date: Date) -> Self {
        self.historical_until.replace(date);
        self
    }
}

impl<P: QuotesProvider> QuotesProvider for QuotesProviderAdapter<P> {
    fn name(&self) -> &'static str {
        self.provider.name()
    }

    fn high_precision(&self) -> bool {
        self.provider.high_precision()
    }

    fn supports_forex(&self) -> bool {
        !self.historical_only && self.provider.supports_forex()
    }

    fn supports_stocks(&self) -> SupportedExchange {
        if self.historical_only {
            return SupportedExchange::None;
        }
        self.provider.supports_stocks()
    }

    fn supports_historical_stocks(&self) -> SupportedExchange {
        self.provider.supports_historical_stocks()
    }

    fn get_quotes(&self, symbols: &[&str]) -> GenericResult<QuotesMap> {
        self.provider.get_quotes(symbols)
    }

    fn get_historical_quotes(&self, symbol: &str, period: Period) -> GenericResult<Option<HistoricalQuotes>> {
        if let Some(until) = self.historical_until {
            if period.first_date() >= until {
                return Ok(None);
            }
        }
        self.provider.get_historical_quotes(symbol, period)
    }
}