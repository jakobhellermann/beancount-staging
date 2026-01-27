# run

run *args:
    just frontend build
    @cargo run -q -- -j docs/examples/journal.beancount -s docs/examples/staging.beancount {{ args }}

real *args:
    just frontend build
    cargo run -q -p beancount-staging-cli -- -j ~/finances/src/transactions.beancount -j ~/finances/journal.beancount -j ~/finances/src/ignored.beancount -s ~/finances/extracted.beancount {{ args }}

predict-eval *args:
    cargo run -r -p beancount-staging-predictor --example evaluate -- -j ~/finances/src/transactions.beancount -j ~/finances/journal.beancount -j ~/finances/src/ignored.beancount {{ args }}

predict-plot:
    #!/usr/bin/env bash
    set -euo pipefail
    cd crates/beancount-staging-predictor
    echo "Generating learning curve data
    cargo run --release --example plot_prediction -- \
        -j ~/finances/src/transactions.beancount \
        -j ~/finances/journal.beancount \
        -j ~/finances/src/ignored.beancount \
        2>/dev/null > learning_curve.csv
    echo "Plotting learning curve..."
    uv run --with matplotlib --with pandas python plot_learning_curve.py learning_curve.csv --exclude dt_raw payee_freq dt_shuffled

# development

frontend *script:
    @cd crates/beancount-staging-web/frontend && npm --silent run {{ script }}

maturin *args:
    @cd crates/beancount-staging-py && CARGO_TARGET_DIR=target maturin {{ args }}

maturin-run:
    just maturin develop && uv run python crates/beancount-staging-py/example/run.py

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
    cargo nextest run --status-level fail --workspace
