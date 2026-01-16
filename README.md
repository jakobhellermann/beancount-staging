# beancount-staging

## Usage

`beancount-staging` is a standalone tool for [beancount](https://github.com/beancount/beancount) which lets helps you to bridge automatic imports of transactions into your categorized journal.

Given your existing journal and an automated import of transactions, beancount-staging starts a website in which you can interactively assign expense accounts, modify details, add tags and links.

When you're done, it will append these transactions to your journal.

```sh
beancount-staging --journal-file journal.beancount --staging-file automated.beancount
```

![demo image](./docs/demo.png)

## Installation

```sh
# via uv
uvx beancount-staging # run without installing
uv tool install beancount-staging # install

# via cargo
cargo install --git https://github.com/jakobhellermann/beancount-staging

# via nix
nix run github:jakobhellermann/beancount-staging # run without installing
nix profile add github:jakobhellermann/beancount-staging # install
```

## CLI References

```
Tool for reviewing and staging beancount transactions

Usage: beancount-staging --journal-file <JOURNAL_FILE> --staging-file <STAGING_FILE> [COMMAND]

Commands:
  serve  Start web server for interactive review (default)
  diff   Show differences between journal and staging files and exit

Options:
  -j, --journal-file <JOURNAL_FILE>  Journal file path. Staged transactions will be written into the first file
  -s, --staging-file <STAGING_FILE>  Staging file path
  -h, --help                         Print help
```

## Use case scenarios

TODO

## Development

TODO
