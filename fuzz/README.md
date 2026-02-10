# syntax fuzzing

## prerequisites

- `cargo install cargo-fuzz`
- nightly toolchain available (`rustup toolchain install nightly`)

## targets

- `syntax_update`
- `syntax_interpolate`
- `syntax_edits`

## quick run (address sanitizer)

```bash
RUST_BACKTRACE=1 ASAN_OPTIONS=abort_on_error=1:detect_leaks=0 \
  cargo +nightly fuzz run syntax_update --sanitizer address -- -max_total_time=60

RUST_BACKTRACE=1 ASAN_OPTIONS=abort_on_error=1:detect_leaks=0 \
  cargo +nightly fuzz run syntax_interpolate --sanitizer address -- -max_total_time=60

RUST_BACKTRACE=1 ASAN_OPTIONS=abort_on_error=1:detect_leaks=0 \
  cargo +nightly fuzz run syntax_edits --sanitizer address -- -max_total_time=60
```

## deterministic corpus replay

```bash
cargo +nightly fuzz run syntax_update -- -runs=1 -seed=1337 fuzz/corpus/syntax_update/seed_0
cargo +nightly fuzz run syntax_interpolate -- -runs=1 -seed=1337 fuzz/corpus/syntax_interpolate/seed_0
cargo +nightly fuzz run syntax_edits -- -runs=1 -seed=1337 fuzz/corpus/syntax_edits/seed_0
```

## crash artifact replay

```bash
cargo +nightly fuzz run syntax_update fuzz/artifacts/syntax_update/<artifact>
cargo +nightly fuzz run syntax_interpolate fuzz/artifacts/syntax_interpolate/<artifact>
cargo +nightly fuzz run syntax_edits fuzz/artifacts/syntax_edits/<artifact>
```
