use std::fs::File;
use std::io::Write;

use retest::Plan;

use investments::core::EmptyResult;

#[test]
fn generate_regression_tests() {
    let mut t = Tests::new();
    let last_tax_year = 2026;

    // cli
    t.with_args("No command", &[]).exit_code(2);

    t.add("Help short", "-h");
    t.add("Help long", "--help");

    for command in [
        "sync", "show", "rebalance", "cash", "buy", "sell",
        "analyse", "backtest", "simulate-sell", "tax-statement", "cash-flow",
        "deposits", "metrics", "completion",
    ] {
        t.add(&format!("Help {command} short"), &format!("{command} -h"));
        t.add(&format!("Help {command} long"), &format!("{command} --help"));
    }

    // deposits
    t.add("Deposits", "deposits");
    t.add("Deposits cron mode", "deposits --cron --date 01.01.2100");

    // show
    t.add("Show", "show ib");
    t.add("Show flat", "show ib --flat");

    // analyse
    t.add("Analyse", "analyse --all");
    t.add("Analyse virtual", "analyse --all --method virtual");
    t.add("Analyse inflation-adjusted", "analyse --all --method inflation-adjusted");
    t.add("Analyse delisted", "analyse tbank-delisting --all").config("other");

    // backtest
    t.add("Backtest", "backtest");
    t.add("Backtest virtual", "backtest --method virtual");
    t.add("Backtest inflation-adjusted", "backtest --method inflation-adjusted");
    t.add("Backtest portfolio", "backtest ib");

    // simulate-sell
    t.add("Simulate sell partial", "simulate-sell ib all VTI 50 BND 50 BND");
    t.add("Simulate sell OTC trade", "simulate-sell tbank-delisting").config("other");
    t.add("Simulate sell in other currency", "simulate-sell tbank --base-currency USD");
    t.add("Simulate sell after stock split", "simulate-sell ib-stock-split all AAPL").config("other");
    t.add("Simulate sell after reverse stock split", "simulate-sell ib-reverse-stock-split all AAPL all VISL").config("other");
    t.add("Simulate sell stock grant", "simulate-sell ib-external-exchanges all IBKR").config("other");
    t.add("Simulate sell zero cost position", "simulate-sell ib-complex 5 VTRS 125 VTRS").config("other");
    t.add("Simulate sell with mixed currency", "simulate-sell tbank-mixed-currency-trade all EQMX all RSHA").config("other");

    // tax-statement
    t.add("IB complex tax statement", "tax-statement ib-complex").config("other");
    t.add("IB external exchanges tax statement", "tax-statement ib-external-exchanges").config("other");
    t.add("IB fractional shares split tax statement", "tax-statement ib-fractional-shares-split").config("other");
    t.add("IB liquidation tax statement", "tax-statement ib-liquidation").config("other");
    t.add("IB reverse stock split tax statement", "tax-statement ib-reverse-stock-split").config("other");
    t.add("IB reverse stock split with reverse order tax statement", "tax-statement ib-reverse-stock-split-reverse-order").config("other");
    t.add("IB simple with LSE tax statement", "tax-statement ib-simple-with-lse").config("other");
    t.add("IB spinoff with selling tax statement", "tax-statement ib-spinoff-with-selling").config("other");
    t.add("IB stock split tax statement", "tax-statement ib-stock-split").config("other");
    t.add("IB symbol with space tax statement", "tax-statement ib-symbol-with-space").config("other");
    t.add("IB tax remapping tax statement", "tax-statement ib-tax-remapping").config("other");
    t.add("IB trading tax statement", "tax-statement ib-trading").config("other");
    t.add("IB with enabled Stock Yield Enhancement Program (not received yet) tax statement", "tax-statement ib-stock-yield-enhancement-program-not-received-yet").config("other");
    t.add("Open MOEX dividends tax statement", "tax-statement open-dividends-moex").config("other");
    t.add("Open SPB dividends tax statement", "tax-statement open-dividends-spb").config("other");
    t.add("TBank complex tax statement", "tax-statement tbank-complex").config("other");
    t.add("TBank delisting tax statement", "tax-statement tbank-delisting").config("other");
    t.add("TBank complex full tax statement", "tax-statement tbank-complex-full").config("other");

    // Not all calculations are seen in tax-statement output. For example, dividend jurisdiction
    // appear only in the tax statement, so it worth to test also third party statements here.
    t.tax_statement("IB complex", 2020).config("other");
    t.tax_statement("IB external exchanges", 2021).config("other");
    t.tax_statement("Open dividends MOEX", 2021).config("other");
    t.tax_statement("Open dividends SPB", 2021).config("other");
    t.tax_statement("TBank complex full", 2020).config("other");

    // cash-flow
    t.add("IB margin RUB cash flow", "cash-flow ib-margin-rub").config("other");
    t.add("IB stock split cash flow", "cash-flow ib-stock-split").config("other");
    t.add("IB external exchanges cash flow", "cash-flow ib-external-exchanges").config("other");
    t.add("IB reverse stock split cash flow", "cash-flow ib-reverse-stock-split").config("other");
    t.add("IB reverse stock split with reverse order cash flow", "cash-flow ib-reverse-stock-split-reverse-order").config("other");
    t.add("IB simple with LSE cash flow", "cash-flow ib-simple-with-lse").config("other");
    t.add("IB tax remapping cash flow", "cash-flow ib-tax-remapping").config("other");
    t.add("IB trading cash flow", "cash-flow ib-trading").config("other");
    t.add("IB with enabled Stock Yield Enhancement Program (not received yet) cash flow", "cash-flow ib-stock-yield-enhancement-program-not-received-yet").config("other");
    t.add("Open non-unified account cash-flow", "cash-flow open-iia-a").config("other");
    t.add("Open inactive with forex trades cash flow", "cash-flow open-inactive-with-forex").config("other");
    t.add("Open MOEX dividends cash flow", "cash-flow open-dividends-moex").config("other");
    t.add("Open SPB dividends cash flow", "cash-flow open-dividends-spb").config("other");
    t.add("Sber daily cash flow", "cash-flow sber-daily").config("other");
    t.add("TBank complex cash flow", "cash-flow tbank-complex").config("other");
    t.add("TBank complex full cash flow", "cash-flow tbank-complex-full").config("other");

    // other
    t.add("Metrics", "metrics $OUT_PATH/metrics.prom");
    t.add("Completion", "completion $OUT_PATH/completion.bash");

    // IIA

    for (account_type, open_year) in [("A", 2017), ("B", 2021)] {
        let name = format!("IIA-{account_type}");
        let id = format!("open-iia-{}", account_type.to_lowercase());

        t.add(&format!("{name} analyse"), &format!("analyse {id} --all")).config("other");
        t.add(&format!("{name} simulate sell"), &format!("simulate-sell {id}")).config("other");

        t.add(&format!("{name} tax statement"), &format!("tax-statement {id}")).config("other");
        for year in open_year..=last_tax_year {
            t.with_args(
                &format!("{name} tax statement {year}"),
                &["tax-statement", &id, &year.to_string()],
            ).config("other");
        }
    }

    // Personal accounts

    let accounts = [
        ("BCS",       Some((2019, None, false))),
        ("Firstrade", Some((2020, Some(2022), true))),
        ("IB",        Some((2018, None, true))),
        ("TBank",     Some((2019, None, true))),

        ("BCS IIA",      None),
        ("Investpalata", None),
        ("Kate",         None),
        ("Kate IIA",     None),
        ("Sber",         None),
        ("Sber IIA",     None),
        ("TBank IIA",    None),
        ("VTB",          None),
    ];

    for (name, year_spec) in accounts {
        let id = &name_to_id(name);

        t.with_args(&format!("Rebalance {name}"), &["rebalance", id]);
        t.with_args(&format!("Simulate sell {name}"), &["simulate-sell", id]);

        if let Some((first_tax_year, last_tax_year_spec, full)) = year_spec {
            let last_tax_year = last_tax_year_spec.unwrap_or(last_tax_year);

            for tax_year in first_tax_year..=last_tax_year {
                let tax_year_string = &tax_year.to_string();

                t.with_args(
                    &format!("{name} tax statement {tax_year}"),
                    &["tax-statement", id, tax_year_string],
                );

                if full {
                    t.tax_statement(name, tax_year);
                    t.with_args(
                        &format!("{name} cash flow {tax_year}"),
                        &["cash-flow", id, tax_year_string],
                    );
                }
            }

            if !full {
                t.with_args(&format!("{name} cash flow"), &["cash-flow", id]);
            }
        } else {
            t.with_args(&format!("{name} tax statement"), &["tax-statement", id]);
            t.with_args(&format!("{name} cash flow"), &["cash-flow", id]);
        }
    }

    t.write().unwrap()
}

struct Tests {
    tests: Vec<Test>,
}

impl Tests {
    fn new() -> Tests {
        Tests { tests: Vec::new() }
    }

    fn add<'a>(&'a mut self, name: &str, command: &str) -> &'a mut Test {
        let args = command.split(' ').filter(|arg| !arg.is_empty()).collect::<Vec<_>>();
        self.with_args(name, &args)
    }

    fn with_args<'a>(&'a mut self, name: &str, args: &[&str]) -> &'a mut Test {
        self.tests.push(Test::new(name, "tests/investments", args));
        self.tests.last_mut().unwrap()
    }

    fn tax_statement(&mut self, name: &str, year: i32) -> &mut Test {
        let id = &name_to_id(name);
        let path = format!("$OUT_PATH/{}-tax-statement-{}.de{}", id, year, year % 10);

        self.tests.push(Test::new(
            &format!("{name} tax statement generation {year}"),
            "tests/test-tax-statement", &[id, &year.to_string(), &path],
        ));

        self.tests.last_mut().unwrap()
    }

    fn write(self) -> EmptyResult {
        let mut plan = Plan::new()
            .expected_path("testdata/rt_expected")
            .actual_path("testdata/rt_actual")
            .build();

        for test in self.tests {
            plan.push(test.build());
        }

        let mut file = File::create("tests/rt.rt")?;
        file.write_all(plan.rt().as_bytes())?;
        file.flush()?;

        Ok(())
    }
}

struct Test {
    name: String,
    app: String,
    config: String,
    args: Vec<String>,
    exit_code: i32,
}

impl Test {
    fn new(name: &str, app: &str, args: &[&str]) -> Test {
        Test {
            name: name.to_owned(),
            app: app.to_owned(),
            config: "main".to_owned(),
            args: args.iter().map(|&arg| arg.to_owned()).collect(),
            exit_code: 0,
        }
    }

    fn config(&mut self, name: &str) -> &mut Test {
        name.clone_into(&mut self.config);
        self
    }

    fn exit_code(&mut self, exit_code: i32) -> &mut Test {
        self.exit_code = exit_code;
        self
    }

    fn build(self) -> retest::Test {
        let mut stdout = true;
        for arg in &self.args {
            if arg.starts_with("$OUT_PATH/") {
                stdout = false;
                break;
            }
        }

        let mut args: Vec<&str> = vec![&self.config];
        args.extend(self.args.iter().map(|arg| arg.as_str()));

        let mut test = retest::Test::new(self.app)
            .name(&self.name)
            .args(&args)
            .exit_code(self.exit_code)
            .build();

        if stdout {
            test.stdout(name_to_id(&self.name));
        }

        test
    }
}

fn name_to_id(name: &str) -> String {
    name.chars().fold(String::with_capacity(name.len()), |mut id, char| {
        let char = if " ()".contains(char) {
            '-'
        } else {
            char.to_ascii_lowercase()
        };

        if char != '-' || !matches!(id.chars().last(), Some('-')) {
            id.push(char)
        }

        id
    })
}