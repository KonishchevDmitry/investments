# Foreign brokers

<a name="interactive-brokers"></a>
## Interactive Brokers

The program expects Activity Statements in `*.csv` format for broker statements (`Reports -> Statements -> Activity`). [Custom Activity Statement](#ib-custom-activity-statement) is preferred.

<a name="ib-trade-settle-date"></a>
### Trade settle date information

Activity statements don't provide trade settle date information. So by default all calculations will be made in T+0 mode and `simulate-sell` and `tax-statement` commands will complain on this via warning message because it affects correctness of tax calculations.

Trade settle date information may be obtained from Trade Confirmation Report. To do this, create a Trade Confirmation Flex Query in the IB `Reports -> Flex Queries` tab with the following parameters:

<img src="/docs/images/ib-trade-confirmation-parameters.png?raw=true" width="617" height="658" alt="Trade Confirmation Flex Query Parameters" title="Trade Confirmation Flex Query Parameters">

and download the statements for all periods where you have any trades. Investments will catch these statements and use information from them for calculations in T+2 mode.

<a name="ib-dividend-reclassifications"></a>
### Dividend reclassifications

Every year IB has to adjust the 1042 withholding (i.e. withholding on US dividends paid to non-US accounts) to reflect dividend reclassifications. This is typically done in February the following year. As such, the majority of these adjustments are refunds to customers. The typical case is when IB's best information at the time of paying a dividend indicates that the distribution is an ordinary dividend (and therefore subject to withholding), then later at year end, the dividend is reclassified as Return of Capital, proceeds, or capital gains (all of which are not subject to 1042 withholding).

<a name="ib-tax-remapping"></a>
Investments finds such reclassifications and handles them properly, but at this time it matches dividends to taxes using (date, symbol) pair (matching by description is too fragile). As it turns out sometimes dates of reclassified taxes don't match dividend dates. To workaround such cases there is `tax_remapping` configuration option using which you can manually map reclassified tax to date of its origin dividend.

Here is how it works. The typical case is the following:

1. You've got the following error:
```
E: Unable to find origin operations for the following taxes:
* 10.02.2022: AGG
```

2. You look into you statements and find the lines the error originates from:
```
Withholding Tax,Data,USD,2022-02-10,AGG(US4642872265) Cash Dividend USD 0.191959 per Share - US Tax,0.86,
Withholding Tax,Data,USD,2022-02-10,AGG(US4642872265) Cash Dividend USD 0.191959 per Share - US Tax,-0.08,
```
These lines say that we've got `$0.86` tax refund and new `$0.08` tax.

3. Now we need to find the origin dividend. There may be more complex cases but for the typical one we should find `AGG(US4642872265) Cash Dividend USD 0.191959 per Share` string in our statements (includes issuer and dividend amount as somewhat that almost uniquely identifies the dividend) and get the following lines:
```
Dividends,Data,USD,2021-02-05,AGG(US4642872265) Cash Dividend USD 0.191959 per Share (Ordinary Dividend),8.64
Withholding Tax,Data,USD,2021-02-05,AGG(US4642872265) Cash Dividend USD 0.191959 per Share - US Tax,-0.86,
```
If we find more than one dividend, it means that issuer + dividend amount pair is not unique enough ([example](https://github.com/KonishchevDmitry/investments/issues/74#issuecomment-1464879536)) and we have to find the right one by matching their taxes – there must be withheld tax with amount equal to our refund (`$0.86` in our specific case).

4. Having the origin dividend, insert the following lines into configuration file:
```yaml
tax_remapping:
  - {date: 2022.02.10, to_date: 2021.02.05, description: AGG(US4642872265) Cash Dividend USD 0.191959 per Share - US Tax}
```

<a name="ib-cash-flow-info"></a>
<a name="ib-custom-activity-statement"></a>
### Custom activity statement

Default Activity Statement contains only essential information and omits some details. For example [dividend reclassifications](#ib-dividend-reclassifications) don't provide actual dates of cash flows on your account which may be important for [cash-flow](taxes.md#cash-flow) command. For this reason it's recommended to use Custom Activity Statement instead of Default Activity Statement. Plus, it's actually a good idea to keep your statements with max level of detail — who knows when it might be needed.

If Default Activity Statement is used, investments remaps dividend reclassification dates from the past to statement period start date to workaround the issue.

To generate Custom Activity Statement:
* Go to `Reports -> Statements -> Custom Statements`
* Select `Statement Type - Activity`
* Select all sections
* Use the following section configurations:
<img src="/docs/images/ib-custom-activity-statement-parameters.png?raw=true" width="685" height="407" alt="Custom Activity Statement Parameters" title="Custom Activity Statement Parameters">


<a name="firstrade"></a>
## Firstrade

The program expects broker statements in `*.ofx` format (`Accounts -> History -> Download Account History -> Microsoft Money`).

Please take into account the following issues with Firstrade statements:
1. Firstrade doesn't provide information about real dividend amount, so it will be deduced from received amount and expected tax rate.
2. When you generate broker statements, current cash assets and open positions information will always be got for yesterday date. So you effectively aren't able to generate a valid statement with ending date different from yesterday. But you should split the statements for the following reasons:

   2.1. Firstrade allows to generate the statement for past three years only.

   2.2. You should have your statements split by years for [cash-flow](taxes.md#cash-flow) command.

   So, considering this, I recommend to generate new statement for the previous year on each January 1.


# Russian Brokers

When you configure your portfolio as backed by a Russian broker, the following configuration options should be specified depending on your account type:
* `tax_payment_day: on-close` in case of [IIA type A](https://github.com/KonishchevDmitry/investments/files/7531658/iia.pdf) account.
* `tax_exemptions: [tax-free]` in case of [IIA type B](https://github.com/KonishchevDmitry/investments/files/7531658/iia.pdf) account.
* `tax_exemptions: [long-term-ownership]` in case of an ordinary brokerage account where [Long-Term Ownership tax exception](https://github.com/KonishchevDmitry/investments/files/7531659/lto.pdf) is applied.

<a name="stock-splits-in-russian-brokers"></a>
### Stock splits

Please note that Russian brokers process stock splits in a way which resets FIFO and LTO. You can read a long discussion about it here:

* [Инвесторам в фонды (БПИФ и ETF) на Мосбирже. Сплит](https://smart-lab.ru/blog/730297.php)
* [3 февраля 2022 года состоится сплит FXGD, FXTB и FXRU](https://smart-lab.ru/blog/756705.php)
* [Крестовый поход против Открытия и Финекса](https://smart-lab.ru/blog/757057.php)

Taking this into account, investments processes stock splits in a similar way for all Russian brokers, but you [can try to return the tax](https://journal.tinkoff.ru/broker-obnulil-lgotu/) and include the returned amount into the calclucations by adding it to `tax_deductions` configuration option.


<a name="open-broker"></a>
## Открытие Брокер

The program expects broker statements in `*.xml` format.

Dividend records in the statement are identified by some broker-specific instrument ID and there is no any mapping of it to instrument symbol in the statement, so the program will ask you to specify this mapping manually via `instrument_internal_ids` configuration option.


<a name="tinkoff"></a>
## Тинькофф

The program expects broker statements in `*.xlsx` format.

<a name="tinkoff-foreign-income"></a>
Tinkoff broker statements don't contain dividend and tax withheld amounts for dividends from non-Russian issuers - only result amount which has been paid. This information is provided in a separate foreign income statement (Справка о доходах за пределами РФ) which is only available as *.pdf (very unsuitable for parsing) from your account page. But you
can ask support for *.xlsx version of it and place it to the broker statements directory. The program will find it and merge its information with broker statement.

Tinkoff broker statements don't contain any information about corporate actions, so stock splits must be specified manually via `corporate_actions` configuration option.


<a name="bcs"></a>
## БКС

The program expects broker statements in `*.xls` format.

Dividends aren't parsed out from broker statements yet. I use ETFs which don't pay dividends, so I don't have an example of how they are look like in the broker statements.
