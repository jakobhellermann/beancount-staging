# beancount-staging

Tools for reviewing and staging beancount transactions.

## Usage

Show differences between journal and staging:

```bash
beancount-staging -j journal.beancount -s staging.beancount show
```

Interactive TUI review:

```bash
beancount-staging -j journal.beancount -s staging.beancount review
```

Web UI:

```bash
beancount-staging -j journal.beancount -s staging.beancount web
```

## Development

```bash
just cli show         # Show diff
just cli review       # Run TUI
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
