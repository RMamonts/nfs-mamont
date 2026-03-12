# Fuzzing

This directory contains fuzz targets for the project using [`cargo-fuzz`](https://github.com/rust-fuzz/cargo-fuzz), a
fuzzing harness for Rust.

## Prerequisites

Install `cargo-fuzz`:

```bash
cargo install cargo-fuzz
```

## Available Targets

```bash
cargo +nightly fuzz list
```

* `round` - round-trip tests for functions from `parser::nfsv3` and `parser::mount`
* `parser` - continuous test of `parser::parser_struct` for inner structure consistency
* `serializer` - continuous test of `serializer::serializer_struct` for inner structure consistency

## Run target

```bash
cargo +nightly fuzz run <target_name> -- -only_ascii=1 -max_len=4096 -max_total_time=300
```


