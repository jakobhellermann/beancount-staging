cli *args:
    @cargo run -q -p beancount-staging-cli -- --journal docs/examples/journal.beancount --staging docs/examples/staging.beancount {{ args }}

check:
    cargo fmt --check
    cargo clippy
    cargo nextest run --status-level fail
    @cd crates/beancount-staging-web/frontend && pnpm --silent run check
