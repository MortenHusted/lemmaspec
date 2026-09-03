# Contributing

Bug reports, design discussions, and focused pull requests are welcome.

Before opening a pull request:

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
cargo package --locked
```

Changes to an example's committed HTML view must be regenerated with the same
source revision:

```sh
# Exit 1 is expected because the example is deliberately incomplete.
cargo run --quiet -- render examples/persistence_adapter_readiness.lemmaspec || test "$?" -eq 1
```

Keep artifacts self-contained and deterministic. Facts should be observations
or explicit inputs, not guesses added to make an expectation pass. Treat an
unsatisfied expectation as useful evidence when it represents the real state.

By contributing, you agree that your contribution is licensed under the MIT
License in [LICENSE](LICENSE).
