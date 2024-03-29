#!/bin/bash
set -e

cargo -Zbuild-std=core -Zbuild-std-features=panic_immediate_abort build --target thumbv7m-none-eabi -p bootloader --release
cargo build -p app --release $@



rust-objcopy -O binary target/thumbv7m-none-eabi/release/bootloader firmware.bootloader.bin
rust-objcopy -O binary target/thumbv7m-none-eabi/release/app firmware.app.bin

cp firmware.app.bin firmware.bin
dd conv=notrunc if=firmware.bootloader.bin of=firmware.bin
dfu-suffix -v 0483 -d df11 -a firmware.bin
dfu-prefix -a firmware.bin -L
mv firmware.bin firmware.dfu
dfu-util -w -R -a 0 --dfuse-address 0x08000000 -D firmware.dfu
