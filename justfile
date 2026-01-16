# run in scenario

cli *args:
    @cargo run -q -p beancount-staging-cli -- --journal docs/examples/journal.beancount --staging docs/examples/staging.beancount {{ args }}

real:
    @cargo run -p beancount-staging-cli -- -j ~/finances/src/transactions.beancount -j ~/finances/src/ignored.beancount -j ~/finances/src/balance.beancount --staging ~/finances/extracted.beancount web

real-all *args:
    @cargo run -p beancount-staging-cli -- -j ~/finances/journal.beancount -j ~/finances/src/ignored.beancount --staging ~/finances/extracted.beancount {{ args }}

web:
    @cd crates/beancount-staging-web/frontend && pnpm --silent run build --log-level=warning
    @cargo run -q -p beancount-staging-cli -- --journal docs/examples/journal.beancount --staging docs/examples/staging.beancount web

# development

frontend-watch:
    @cd crates/beancount-staging-web/frontend && pnpm --silent run watch

check:
    # format
    @cd crates/beancount-staging-web/frontend && pnpm --silent run check
    @cd crates/beancount-staging-web/frontend && pnpm --silent run fmt:check
    cargo fmt --check

    # lints
    @cd crates/beancount-staging-web/frontend && pnpm --silent run lint
    cargo clippy

    # tests
    cargo nextest run --status-level fail
