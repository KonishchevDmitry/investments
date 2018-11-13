## Investments

A work in progress project to organize my investments.

Targeted for russian investors who use [Interactive Brokers](http://interactivebrokers.com) or
[Open Broker](https://open-broker.ru). Considers taxes, commissions, dividends and tax deductions when calculates
portfolio performance.

### Installation

Install [Rust](https://www.rust-lang.org/):

`curl https://sh.rustup.rs -sSf | sh`

Clone the repository:

`git clone https://github.com/KonishchevDmitry/investments`

Compile and install:

`cargo install --path investments`

### Available functionality

```
$ investments analyse --help

Calculates average rate of return from cash investments by comparing portfolio performance
to performance of a bank deposit with exactly the same investments and monthly capitalization.

USAGE:
    investments analyse <PORTFOLIO>

ARGS:
    <PORTFOLIO>
            Portfolio name
```

```
$ investments tax-statement --help

Reads Interactive Brokers statement and alters *.dcX file (Russian tax program named Декларация)
by adding all required information about income from paid dividends.

If tax statement file is not specified only outputs the data which is going to be declared.

USAGE:
    investments tax-statement <PORTFOLIO> <YEAR> [TAX_STATEMENT]

ARGS:
    <PORTFOLIO>
            Portfolio name

    <YEAR>
            Year to generate the statement for

    <TAX_STATEMENT>
            Path to tax statement *.dcX file
```