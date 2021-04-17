# Investments

Helps you with managing your investments:
* **Portfolio rebalancing:** instructs you which orders you have to submit to make your portfolio in order with your
  asset allocation.
* **Stock selling simulation:** calculates revenue, profit, taxes and real profit percent which considers taxes into
  account.
* **Automatic tax statement generation:** reads broker statements and alters *.dcX file (created by Russian tax program
  named Декларация) by adding all required information about income from stock selling, paid dividends and idle cash
  interest.
* **Analysis:** calculates average rate of return from cash investments by comparing portfolio performance to
  performance of a bank deposit in USD and RUB currency with exactly the same investments and monthly capitalization.
  Considers taxes, commissions, dividends and tax deductions when calculates portfolio performance.
* **Bank deposits control:** view opened bank deposits all in one place and get notified about upcoming deposit closures.

Targeted for Russian investors who use [Interactive Brokers](https://interactivebrokers.com/),
[Тинькофф](https://www.tinkoff.ru/), [Firstrade](https://www.firstrade.com/), [Открытие Брокер](https://open-broker.ru/)
or [БКС](https://broker.ru/).

# Installation

1. Install Rust - https://www.rust-lang.org/tools/install
2. Install or upgrade the package:
```
cargo install investments
```
If it fails to compile and you installed Rust a long time ago, try `rustup update` to update Rust to the latest version.

If you want to install the package from sources, use:
```
git clone https://github.com/KonishchevDmitry/investments.git
cd investments
cargo install --path . --force
```

# Configuration

Create `~/.investments/config.yaml` configuration file ([example](config-example.yaml)). Don't forget to obtain API
token for Finnhub and Twelve Data (see the comments in example config).

# Usage

## Stocks

Investments is designed to work with your broker statements - there is no need to enter all trades and transactions
manually, but it requires you to have all broker statements starting from account opening day. It may be either one
broker statement or many - it doesn't matter, but what matters is that the first statement must be with zero starting
assets and statements' periods mustn't overlap or have missing days in between.

For now the following broker statements are supported:
* Interactive Brokers (*.csv)
* Тинькофф (*.xlsx)
* Firstrade (experimental support) (*.ofx)
* Открытие Брокер (IIA, basic support) (*.xml)
* БКС (basic support) (*.xls)

Investments keeps some data in local database located at `~/.investments/db.sqlite` and supports a number of commands
which can be grouped as:
* Analyse commands (`analyse`, `simulate-sell`, `tax-statement`) that read your broker statements and produce some
  results.
* `sync` command that reads your broker statements and stores your current positions to the local database.
* Portfolio rebalancing commands (`show`, `rebalance`, `cash`, `buy`, `sell`) that work only with local database.

Local database is required for portfolio rebalancing because during rebalancing you submit buy/sell orders to your
broker that modify your portfolio (free assets, open positions) and this information have to be saved somewhere until at
least tomorrow when you'll be able to download a new broker statement which will include the changes.

Here is an example of main commands output:

<img src="/images/analyse.png?raw=true" width="70%" height="70%" alt="investments analyse" title="investments analyse">

![investments simulate-sell](/images/simulate-sell.png?raw=true "investments simulate-sell")

![investments tax-statement](/images/tax-statement.png?raw=true "investments tax-statement")

The screenshots are blurred for privacy reasons since they require a real broker statement, but I can emulate `sync`
command by executing the following commands with a random fake data to provide a full example of `show` and `rebalance`
commands:
```
$ investments buy ib 100 VTI 4000
$ investments buy ib 30 VXUS 4000
$ investments buy ib 40 BND 4000
$ investments buy ib 60 BNDX 4000
```

With these commands executed and provided example config we'll get the following results for `show` and `rebalance`
commands:

![investments show](/images/show.png?raw=true "investments show")

![investments rebalance](/images/rebalance.png?raw=true "investments rebalance")

Rebalancing actions in this case are assumed to be the following:
1. View the instructions: `investments rebalance`
2. Buy 50 VXUS using broker's terminal, got `$current_assets` left on your account
3. Commit the results: `investments buy ib 50 VXUS $current_assets`
4. View the instructions: `investments rebalance`
5. Buy 12 BNDX using broker's terminal, got `$current_assets` left on your account
6. Commit the results: `investments buy ib 12 BNDX $current_assets`
7. View the instructions: `investments rebalance`
8. Buy 9 BND using broker's terminal, got `$current_assets` left on your account
9. Commit the results: `investments buy ib 9 BND $current_assets`
10. View the result: `investments show`

This iterative trading is not required - you can look at the results of `investments rebalance` and submit all orders at
once, but it leaves a chance to spend more than you supposed to in case of highly volatile market. In practice, the
simplest strategy here in case of relatively small price of all stocks - submit all orders except the last (one / two /
few), commit the current result, execute `investments rebalance` and submit the rest.

## Prometheus metrics

`investments metrics` command allows you to export analysis results in [Prometheus](https://prometheus.io/) format to be
collected by [Node exporter's Textfile Collector](https://github.com/prometheus/node_exporter#textfile-collector).

Here is an example of [Grafana](https://grafana.com/) dashboard which displays aggregated statistics and investment
results for multiple portfolios opened in different brokers:

[![Investments Grafana dashboard](https://user-images.githubusercontent.com/217795/105888583-320e1080-601e-11eb-8a47-97774479e0f7.gif)](https://youtu.be/fMUxBDY3AUg)

## Broker specific

### Interactive Brokers

The program expects Activity Statements in \*.csv format for broker statements (`Reports -> Statements -> Activity`).

#### Recommendations
      
Download broker statements periodically and run the tool against them to be sure that it's still able to parse them and
won't fail when you'll need it.

Generate tax statement in the beginning of March. Interactive Brokers sometimes adds corrections to their mid-February
statements and if you generate tax statement earlier, it may contain inaccurate data.

#### T+2 trading mode

Activity statements don't provide trade settle date. So by default all calculations will be made in T+0 mode and
`simulate-sell` and `tax-statement` commands will complain on this via warning message because it affects correctness of
tax calculations.

Trade settle date may be obtained from Trade Confirmation Report. To do this, create a Trade Confirmation Flex Query in
the IB `Reports -> Flex Queries` tab with the following parameters:

![Trade Confirmation Flex Query Parameters](/images/trade-confirmation-parameters.png?raw=true "Trade Confirmation Flex Query Parameters")

and download the statements for all periods where you have any trades. Investments will catch these statements and use
information from them for calculations in T+2 mode.

<a name="dividend-reclassifications"></a>
#### Dividend reclassifications

Every year IB has to adjust the 1042 withholding (i.e. withholding on US dividends paid to non-US accounts) to reflect
dividend reclassifications. This is typically done in February the following year. As such, the majority of these
adjustments are refunds to customers. The typical case is when IB's best information at the time of paying a dividend
indicates that the distribution is an ordinary dividend (and therefore subject to withholding), then later at year end,
the dividend is reclassified as Return of Capital, proceeds, or capital gains (all of which are not subject to 1042
withholding).

So withholding in previous year's statements should be reviewed against February statement's withholding adjustments.
Investments finds such reclassifications and handles them properly, but at this time it matches dividends on taxes using
(date, symbol) pair, because matching by description turned out to be too fragile. As it turns out sometimes dates
of reclassified taxes don't match dividend dates. To workaround such cases there is `tax_remapping` configuration option
using which you can manually map reclassified tax to date of its origin dividend.


### БКС and Открытие Брокер

Dividends aren't parsed out from broker statements yet. I use FinEx ETF which don't pay dividends, so I don't have an
example of how they are look like in the broker statements.

## Deposits

Deposits are controlled via `deposits` command. You register your opened deposits in the configuration file and then
execute `investments deposits` to view them all in one place:

```
$ investments deposits

                            Open deposits

 Open date   Close date    Name     Amount   Interest  Current amount
 19.06.2019  19.03.2020  Тинькофф  465,000₽         7     473,343.49₽
 21.06.2019  21.06.2020  Тинькофф  200,000₽       7.5     203,763.08₽
                                   665,000₽               677,106.57₽
```

This command has a cron mode (`investments deposits --cron`) which you can use in combination with
`notify_deposit_closing_days` configuration option. For example, if you create a cron job and configure it to send the
command output to your email, then on 11.06.2020 having `notify_deposit_closing_days: 10` you get an email with the
following contents:

```
The following deposits are about to close:
* 21.06.2020 Тинькофф: 200,000₽ -> 215,570.51₽

The following deposits are closed:
* 19.03.2020 Тинькофф: 465,000₽ -> 490,013.27₽
```


# Unsupported features

The program supports only those cases which I saw in my broker statements or statements sent to me by other people,
which I assured to be handled properly and wrote regression tests for. For example, the following aren't supported yet:
* Bonds
* Futures
* Options
* Margin accounts


# Denial of responsibility

The project is developed as a pet project, mainly for my personal use. The code is written in a way that if it finds
something unusual in broker statement it returns an error and doesn't try to pass through the error to avoid the case
when it will get you to misleading results, so there are many cases that it's not able to handle yet and I can't
guarantee that I'll find a free time to support your specific case.


# Contacts

[Issues](https://github.com/KonishchevDmitry/investments/issues) and
[Discussions](https://github.com/KonishchevDmitry/investments/discussions) are the preferred way for requests and
questions. Please use [email](mailto:konishchev@gmail.com) only for privacy reasons.
