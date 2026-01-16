# run

run *args:
    @cargo run -q -- -j docs/examples/journal.beancount -s docs/examples/staging.beancount {{ args }}

web:
    #!/usr/bin/env bash
    set -euo pipefail
    just frontend watch &
    trap "kill $! 2>/dev/null || true" EXIT
    cargo run -q -- -j docs/examples/journal.beancount -s docs/examples/staging.beancount

real:
    #!/usr/bin/env bash
    set -euo pipefail
    just frontend watch &
    trap "kill $! 2>/dev/null || true" EXIT
    cargo run -q -p beancount-staging-cli -- -j ~/finances/journal.beancount -j ~/finances/src/ignored.beancount -s ~/finances/extracted.beancount

# development

frontend script:
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
    cargo nextest run --status-level fail
