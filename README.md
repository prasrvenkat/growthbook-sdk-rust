# growthbook-sdk-rust
GrowthBook Rust SDK (unreleased, unofficial)
Unreleased mainly to avoid taking name on crates.io in case growthbook team wants the name later.

1. Add library to Cargo.toml
2. Depending on framework initialize repository once and reuse. For example in Axum this can be done in state.
3. Create growthbook instance in request handlers passing context dynamically, fetching features from repository in state.

