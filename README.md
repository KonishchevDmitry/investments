[![Test status](https://github.com/KonishchevDmitry/investments/actions/workflows/test.yml/badge.svg)](https://github.com/KonishchevDmitry/investments/actions/workflows/test.yml)

# Investments

Helps you with managing your investments:
* **Portfolio rebalancing:** instructs you which orders you have to submit to make your portfolio in order with your asset allocation.
* **Stock selling simulation:** calculates revenue, profit, taxes and real profit percent which considers taxes into account.
* **Automatic tax statement generation:** reads broker statements and alters *.dcX file (created by Russian tax program named Декларация) by adding all required information about income from stock selling, paid dividends and idle cash interest.
* **Analysis:** calculates average rate of return from cash investments by comparing portfolio performance to performance of a bank deposit in USD and RUB currency with exactly the same investments and monthly capitalization. Considers taxes, commissions, dividends, tax deductions and optionally inflation when calculates portfolio performance.
* **Bank deposits control:** view opened bank deposits all in one place and get notified about upcoming deposit closures.

Targeted for Russian investors who use [Firstrade](https://www.firstrade.com/), [Interactive Brokers](https://interactivebrokers.com/), [БКС](https://broker.ru/), [Сбер](https://sberbank.ru/) or [Тинькофф](https://www.tinkoff.ru/).

# Installation

See [installation instructions](docs/install.md).

# Configuration

Create `~/.investments/config.yaml` configuration file. See [example](docs/config-example.yaml) which contains typical configuration for each broker, tax exemptions that are applicable to the account and more. Don't forget to obtain API token for FCS API and Finnhub (see [stock and forex quotes providers](docs/quotes.md) for details).

# Usage

## Stocks

Investments is designed to work with your broker statements — there is no need to enter all trades and transactions manually, but it requires you to have all broker statements starting from account opening day. It may be either one broker statement or many — it doesn't matter, but what matters is that the first statement must be with zero starting assets and statements' periods mustn't overlap or have missing days in between.

For now the following brokers are supported:
* Firstrade ([details](https://github.com/KonishchevDmitry/investments/blob/master/docs/brokers.md#firstrade))
* Interactive Brokers ([details](https://github.com/KonishchevDmitry/investments/blob/master/docs/brokers.md#interactive-brokers))
* БКС ([details](https://github.com/KonishchevDmitry/investments/blob/master/docs/brokers.md#bcs))
* Сбер ([details](https://github.com/KonishchevDmitry/investments/blob/master/docs/brokers.md#sber))
* Тинькофф ([details](https://github.com/KonishchevDmitry/investments/blob/master/docs/brokers.md#tinkoff))

Investments keeps some data in local database located at `~/.investments/db.sqlite` and supports a number of commands which can be grouped as:
* Analyse commands ([analyse](#analyse), [cash-flow](docs/taxes.md#cash-flow), [metrics](#metrics),
  [simulate-sell](#simulate-sell), [tax-statement](docs/taxes.md#tax-statement)) that read your broker statements and produce some results. These commands use the database only for quotes caching.
* `sync` command that reads your broker statements and stores your current positions to the local database.
* Portfolio rebalancing commands ([show, rebalance, cash, buy, sell](docs/rebalancing.md)) that work only with local database.

<a name="analyse"></a>
### Performance analysis

`investments analyse` command calculates average rate of return from cash investments by comparing portfolio performance to performance of a bank deposit in USD and RUB currency with exactly the same investments and monthly capitalization. Considers taxes, commissions, dividends, tax deductions and optionally inflation when calculates portfolio performance.

<img src="/docs/images/analyse-command.png?raw=true" width="80%" height="80%" alt="investments analyse" title="investments analyse">

### Portfolio rebalancing

See [instructions for portfolio rebalancing](docs/rebalancing.md).

![investments rebalance](/docs/images/rebalance-command.png?raw=true "investments rebalance")

### Tax statement generation

See [instructions for tax statement generation and recommendations for interacting with Russian Federal Tax Service](docs/taxes.md).

![investments tax-statement](/docs/images/tax-statement-command.png?raw=true "investments tax-statement")

<a name="simulate-sell"></a>
### Sell simulation

`investments simulate-sell` command simulates closing of the specified positions by current market price and allows you to estimate your profits, taxes and tax exemption applicability.

![investments simulate-sell](/docs/images/simulate-sell-command.png?raw=true "investments simulate-sell")

<a name="metrics"></a>
### Prometheus metrics

`investments metrics` command allows you to export analysis results in [Prometheus](https://prometheus.io/) format to be collected by [Node exporter's Textfile Collector](https://github.com/prometheus/node_exporter#textfile-collector).

Here is an example of [Grafana](https://grafana.com/) dashboard which displays aggregated statistics and investment results for multiple portfolios opened in different brokers:

[![Investments Grafana dashboard](https://user-images.githubusercontent.com/217795/105888583-320e1080-601e-11eb-8a47-97774479e0f7.gif)](https://youtu.be/fMUxBDY3AUg)

## Deposits

You can also view opened bank deposits all in one place and get notified about upcoming deposit closures. Register your opened deposits in the configuration file and then execute:

```
$ investments deposits

                            Open deposits

 Open date   Close date    Name     Amount   Interest  Current amount
 19.06.2019  19.03.2020  Тинькофф  465,000₽         7     473,343.49₽
 21.06.2019  21.06.2020  Тинькофф  200,000₽       7.5     203,763.08₽
                                   665,000₽               677,106.57₽
```

This command has a cron mode (`investments deposits --cron`) which you can use in combination with `notify_deposit_closing_days` configuration option. For example, if you create a cron job and configure it to send the command output to your email, then on 11.06.2020 having `notify_deposit_closing_days: 10` you get an email with the following contents:

```
The following deposits are about to close:
* 21.06.2020 Тинькофф: 200,000₽ -> 215,570.51₽

The following deposits are closed:
* 19.03.2020 Тинькофф: 465,000₽ -> 490,013.27₽
```


# Unsupported features

The program is focused on passive investing use cases and supports only those cases which I saw in my broker statements or statements sent to me by other people, which I assured to be handled properly and wrote regression tests for. For example, the following aren't supported yet:
* [Bonds](https://github.com/KonishchevDmitry/investments/issues/43)
* [Margin trading](https://github.com/KonishchevDmitry/investments/issues/8)
* [Futures and options](https://github.com/KonishchevDmitry/investments/issues/48)


# Denial of responsibility

Any automation is imperfect and the author is a software developer, not a tax lawyer, so always be critical to all program's calculation results.

The project is developed as a pet project, mainly for my personal use. The code is written in a way that if it finds something unusual in broker statement it returns an error and doesn't try to pass through the error to avoid the case when it will get you to misleading results, so there may be many cases that it's not able to handle yet and I can't guarantee that I'll find a free time to support your specific case.


# Contacts

[Issues](https://github.com/KonishchevDmitry/investments/issues) and
[Discussions](https://github.com/KonishchevDmitry/investments/discussions) are the preferred way for requests and questions. Please use [email](mailto:konishchev@gmail.com) only for privacy reasons.
