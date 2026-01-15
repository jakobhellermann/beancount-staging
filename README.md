# beancount-staging

Tools for reviewing and staging beancount transactions.

## Usage

Compare journal and staging files:

```bash
cargo run -p beancount-staging-cli -- -j journal.beancount -s staging.beancount show
```

Interactive TUI review:

```bash
cargo run -p beancount-staging-cli -- -j journal.beancount -s staging.beancount review
```

Web UI:

```bash
cargo run -p beancount-staging-web -- -j journal.beancount -s staging.beancount
```

## Development

```bash
just cli show          # Run CLI show command
just cli review        # Run TUI
just web              # Run web server
just check            # Format, lint, test
```

Frontend development:

```bash
cd crates/beancount-staging-web/frontend
pnpm install
pnpm run build
pnpm run check        # Type check
```
