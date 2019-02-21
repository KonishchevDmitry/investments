[![Build Status](https://travis-ci.com/KonishchevDmitry/investments.svg?branch=master)](https://travis-ci.com/KonishchevDmitry/investments)

## Investments

Helps you with managing your investments:
* **Portfolio rebalancing:** instructs you which orders you have to submit to make your portfolio in order with your asset
  allocation.
* **Automatic tax statement generation:** reads broker statements and alters *.dcX file (created by Russian tax program named
  Декларация) by adding all required information about income from paid dividends.
* **Analysis:** calculates average rate of return from cash investments by comparing portfolio performance to performance of
  a bank deposit with exactly the same investments and monthly capitalization. Considers taxes, commissions, dividends
  and tax deductions when calculates portfolio performance.

Targeted for Russian investors who use [Interactive Brokers](http://interactivebrokers.com) or
[Открытие Брокер](https://open-broker.ru).

### Installation

1. Install Rust - https://www.rust-lang.org/tools/install
2. Compile the project:
```
$ git clone https://github.com/KonishchevDmitry/investments.git
$ cd investments
$ cargo install --path .
```

Use the following command to recompile the project after update to a new version:
```
cargo install --force --path .
```

### Configuration and usage

Create `~/.investments/config.yaml` configuration file ([example](config-example.yaml)). Don't forget to obtain API
token for Alpha Vantage (see the comments in example config).

Investments is designed to work with your broker statements - there is no need to enter all trades and transactions
manually, but it requires you to have all broker statements starting from account opening day. It may be either one
broker statement or many - it doesn't matter, but what matters is that the first statement must be with zero starting
assets and statements' periods mustn't overlap or have missing days in between.

For now only the following broker statements are supported:
* Interactive Brokers (*.csv)
* Открытие Брокер (ИИС) (*.xml)

Investments keeps some data in local database located at `~/.investments/db.sqlite` and supports a number of commands
which can be grouped as:
* Analyse commands (`analyse`, `tax-statement`) that read your broker statements and produce some results.
* `sync` command that reads your broker statements and stores your current positions to the local database.
* Portfolio rebalancing commands (`show`, `rebalance`, `cash`, `buy`, `sell`) that work only with local database.

Local database is required for portfolio rebalancing because during rebalancing you submit buy/sell orders to your
broker that modify your portfolio (free assets, open positions) and this information have to be saved somewhere until at
least tomorrow when you'll be able to download a new broker statement which will include the changes.

I don't provide an example of `analyse` or `tax-statement` result here for privacy reasons since they require a real
broker statement, but I can emulate `sync` command by executing the following commands with a random fake data to
provide an example of `show` and `rebalance` commands:
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


### Denial of responsibility

The project is developed as a pet project, mainly for my personal use. The code is written in a way that if it finds
something unusual in broker statement it returns an error and doesn't try to pass through the error to avoid the case
when it will get you to misleading results, so there are many cases that it's not able to handle yet and I can't
guarantee that I'll find a free time to support your specific case.