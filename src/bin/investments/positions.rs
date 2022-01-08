use clap::{Arg, ArgMatches};

use investments::core::GenericResult;
use investments::types::Decimal;
use investments::util::{self, DecimalRestrictions};

pub struct PositionsParser {
    name: &'static str,
    help: String,

    allow_all: bool,
    required: bool,
}

impl PositionsParser {
    const ARG_NAME: &'static str = "POSITIONS";

    pub fn new(name: &'static str, allow_all: bool, required: bool) -> PositionsParser {
        let help = format!("{} in {} $symbol format (may be specified multiple times)", name, if allow_all {
            "$quantity|all"
        } else {
            "$quantity"
        });

        PositionsParser {name, help, allow_all, required}
    }

    pub fn arg(&self) -> Arg {
        Arg::new(PositionsParser::ARG_NAME)
            .help(self.help.as_str())
            // FIXME(konishchev): HERE
            .multiple_occurrences(true)
            .multiple_values(true)
            .takes_value(true)
            .value_names(&["SHARES", "SYMBOL"])
            .required(self.required)
    }

    pub fn parse(&self, matches: &ArgMatches) -> GenericResult<Vec<(String, Option<Decimal>)>> {
        let mut positions = Vec::new();

        let mut args = match matches.values_of(PositionsParser::ARG_NAME) {
            Some(args) => args,
            None => return Ok(positions),
        };

        while let Some(quantity) = args.next() {
            let quantity = if self.allow_all && quantity == "all" {
                None
            } else {
                Some(util::parse_decimal(
                    quantity, DecimalRestrictions::StrictlyPositive
                ).map_err(|_| format!(
                    "{} specification: Invalid quantity: {:?}", self.name, quantity)
                )?)
            };

            let symbol = args.next().ok_or(format!(
                "{} specification: Even number of arguments is expected", self.name))?;

            positions.push((symbol.to_owned(), quantity));
        }

        Ok(positions)
    }
}