# Portfolio rebalancing

Investments can instruct you which orders you have to submit to make your portfolio in order with your asset allocation.

All commands below operate on local database to store intermediate results. Local database is required because during
rebalancing you submit buy/sell orders to your broker that modify your portfolio (free assets, open positions) and this
information have to be saved somewhere until at least tomorrow when you'll be able to download a new broker statement
which will include the changes.

First, `sync` command should be executed to read your broker statements and store your current positions to the local
database. But you are free to not use it - for example if you want to rebalance portfolio of an unsupported broker.

Here is how we can emulate `sync` command execution and populate the database manually:
```
$ investments buy ib 100 VTI 4000
$ investments buy ib 30 VXUS 4000
$ investments buy ib 40 BND 4000
$ investments buy ib 60 BNDX 4000
```

With these commands executed and provided example config we'll get the following results for `show` and `rebalance`
commands:

![investments show](images/show-command.png?raw=true "investments show")

![investments rebalance](images/rebalance-command.png?raw=true "investments rebalance")

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
simplest strategy here in case of relatively small price of all stocks - submit all orders except the last one, commit
the current result, execute `investments rebalance` and submit the rest.

You can also tune `min_cash_assets` configuration option - it configures the amount of cash that must remain on the
account after rebalancing. It can serve both a protection against volatility and to instruct rebalancing logic to
proportionally sell the assets to acquire the specified amount.