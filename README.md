# beancount-staging

## Usage

`beancount-staging` is a standalone tool for [beancount](https://github.com/beancount/beancount) that helps you to bridge automatic imports of transactions into your categorized journal.

Given your existing hand-crafted journal and an automated import of transactions, beancount-staging lets you interactively look through new transactions, assign expense accounts, modify details etc.
When you're done, it will append these transactions to your journal.

```sh
beancount-staging --journal-file journal.beancount --staging-file automated.beancount
```

![demo image](./docs/demo-0.png)
![demo image](./docs/demo-1.png)

In addition to the web frontend, you can also run `beancount-staging diff` to get a command line view of any new transactions.

The tool is inspired by [beancount-import](https://github.com/jbms/beancount-import), which works similarly. My reasons for creating this new tool were

- having an interface optimized for the way I like to use it
- being annoyed with some bugs I ran into with beancount-import
  - not wanting to fix them in python
- wanting something as simple as possible that natively works with beangulp instead of its own importer interface

### Features

- press `a`, `p`, `n` to change account, payee or narration
- autocomplete for accounts
- automatic account suggestions based on previous categorization
- no hidden state, everything is derived from the beancount sources

## Installation

```sh
# via uv/PyPI
uvx beancount-staging # run without installing
uv tool install beancount-staging # install

## building from source

# via nix
nix run github:jakobhellermann/beancount-staging # run without installing
nix profile add github:jakobhellermann/beancount-staging # install

# via cargo
cargo install --git https://github.com/jakobhellermann/beancount-staging
```

## Configuration

Instead of running in the terminal with `-j` and `-s` to specify journal/staging files, you can also have a `beancount-staging.toml` (or `.beancount-staging.toml`:

```toml
[journal]
files = ["docs/examples/journal.beancount"]

[staging]
files = ["docs/examples/staging.beancount"]
```

`beancount-staging` will always commit to the first specified journal file, so with a configuration like the following you can have your dedicated file for imported transactions.

`beancount-staging` doesn't prescribe a way for generating the staging transactions. But a common use case is having a python script using `beangulp` with all your importers. You can specify `staging.command` to any script emitting a beancount file.

```toml
[journal]
files = [
  "src/transactions.beancount", # the output
  "journal.beancount", # can include src/transactions.beancount
]

[staging]
command = ["uv", "run", "bin/import.py", "extract", "data/Assets"]
```

```python
import beangulp
from importers import mybank, mydepot

CONFIG = [
    mybank.Importer(accounts.CHECKING),
    mydepot.Importer(accounts.DEPOT),
    # ...
    beancount_paypal.PaypalImporter(accounts.PAYPAL, ...),
]
HOOKS = []

ingest = beangulp.Ingest(CONFIG, HOOKS)
ingest()
```

## How it works

When you run `beancount-staging`, it will look at all the staging transactions and attempt to find a matching already present transaction in the journal.
If it doesn't find any, it will ask the user to look at the new transaction, if it is unbalanced add the correct `Expense:` account, and commit it into the journal.

### Example

You have an importer that imports your bank statements, and emits beancount transactions like these:

```beancount
; importer output
2020-03-24 * "AMZN MEDIA" "long unreadable description with IDs"
    Assets:MyBank -7.49 EUR
```

The importer doesn't know what expense this transaction corresponds to, so it leaves the second posting blank, or chooses `Expenses:FIXME`.

At this point, running `beancount-staging` will show you this transaction and ask you to provide the correct expense account. When you're done, it appends the resolved transaction at the end of your journal:

```beancount
; journal.beancount
2020-03-24 * "AMZN MEDIA" "Hitchhiker's Guide to the Galaxy Kindle"
    source_desc: "long unreadable description with IDs"
    Assets:MyBank  -7.49 EUR
    Expense:Books   7.49 EUR
```

`beancount-staging` matches staging transaction based on the `date`, `payee` and `narration`, so when you change the latter it will record that using the `source_payee` and `source_desc` metadata.

## CLI Reference

```
Tools for reviewing and staging beancount transactions

Usage: beancount-staging [OPTIONS] [COMMAND]

Commands:
  serve  Start web server for interactive review (default)
  diff   Show differences between journal and staging files and exit

Options:
  -j, --journal-file <JOURNAL_FILE>  Journal file path. Staged transactions will be written into the first file
  -s, --staging-file <STAGING_FILE>  Staging file path
  -c, --config <CONFIG>              Config file or directory path. If a directory is provided, will look for beancount-staging.toml or .beancount-staging.toml in that directory. If not provided, will look in current directory
  -h, --help                         Print help
```

## Development

For the backend and core logic, pretty much just `cargo`.

The frontend lives in [crates/beancount-staging-web/frontend](./crates/beancount-staging-web/frontend), and can be built with `just frontend build` or watched with `just frontend dev`.

Otherwise check the `justfile` the predictor evaluation or `maturin` building.
