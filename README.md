# Shutte speed tester

## Adapting to a different board

* If needed, change the target in both `rust-toolchain.toml` and each Cargo.toml's `forced-target`.
* Tweak [config/src/lib.rs]
* Tweak interrupt names in lines in [app/src/main.rs] marked with `// HWCONFIG`.

## Prerequisites

```shell
cargo install cargo-binutils
rustup component add llvm-tools
```

## Building

Build & flash in DFU mode: `./flash.sh`
