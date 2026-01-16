# run

run *args:
    just frontend build
    @cargo run -q -- -j docs/examples/journal.beancount -s docs/examples/staging.beancount {{ args }}

real *args:
    just frontend build
    cargo run -q -p beancount-staging-cli -- -j ~/finances/journal.beancount -j ~/finances/src/ignored.beancount -s ~/finances/extracted.beancount {{ args }}

# development

frontend *script:
    @cd crates/beancount-staging-web/frontend && npm --silent run {{ script }}

check:
    # format
    just frontend check
    just frontend fmt:check
    cargo fmt --check

    # lints
    @cd crates/beancount-staging-web/frontend && npm --silent run lint
    cargo clippy

    # tests
    just frontend test
    cargo nextest run --status-level fail
