## Introduction

A toy payment app

## Setup & info 

Tested with:
* rust 1.58
* Dependencies version from Cargo.lock

Note:
* Require rust std
* No unsafe code

## Build

* `cargo build`
* `cargo build --release`

## Run

* `cargo run -- resources/sample_1.csv > output.csv`
* `RUST_LOG=debug cargo run -- resources/sample_1_with_errors.csv`

Notes:
* Return:
  * 0 on success
  * 1 if no csv path is provided on cli
  * 2 if csv cannot be read
  * 3 if csv is not valid
  * 4 if an error occurs when processing transaction(s)

## Unit tests

* `cargo test`
* `cargo test accounts::tests::accounts_output_ok -- --nocapture`

## Code quality

* Clippy
  * Only 1 remaining error (never loop) -> clippy mistake? (unit tested properly)
  * 2 warnings about 'redundant_closure' -> clippy mistake? (help not working)
  * 3 warnings about 'functions only used in unit tests'
* Code formatting
  * done (`cargo fmt`)
* Doc
  * `cargo doc && xdg-open target/doc/homework_toy_pay/index.html`

## Futures plans

* Float values are not serialized in CSV with the required precision
* Optimize serde deserialization (https://docs.rs/csv/latest/csv/tutorial/index.html#performance)
* Use Dashmap (https://docs.rs/dashmap/latest/dashmap/struct.DashMap.html) instead of regular HashMap to handle multithreading + Perf?