use core::GenericResult;
use types::Date;
use util;

pub fn parse_date(date: &str) -> GenericResult<Date> {
    util::parse_date(date, "%Y-%m-%dT00:00:00")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_parsing() {
        assert_eq!(parse_date("2017-12-31T00:00:00").unwrap(), date!(31, 12, 2017));
    }
}