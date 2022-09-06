# RC8 - Chip-8 Emulator

A CHIP-8 implemented in Rust.

CHIP-8 is an interpreted language originally used in the [COSMAC VIP](https://en.wikipedia.org/wiki/COSMAC_VIP), and made to make it easier to develop games and similar graphical applications. Technically, there was never a "hardware" implementation of CHIP-8, but since it was defined as a virtual machine (with registers, addresses, interrupts, etc), CHIP-8 "interpreters" are commonly refered as "emulators".

It is commonly used as the "hello world" for emulator enthusiasts. If you're curious, take a look at the [technical reference](https://github.com/mattmikolay/chip-8/wiki/CHIP%E2%80%908-Technical-Reference) to learn more.

## Features / Roadmap

- [X] All [instructions](https://github.com/mattmikolay/chip-8/wiki/CHIP%E2%80%908-Instruction-Set) implemented with test cases.
- [X] Proper, "clipped" drawing.
- [ ] 100% pass on the [CHIP-8 test suite](https://github.com/Timendus/chip8-test-suite).
  - Missing `VF` clear quirk on bitwise operations.
  - Waiting for release on `FX0A` instruction.
- [ ] Sound (buzzer) support.
- [ ] Option to set background/foreground.
- [ ] Option to change the display size.
- [ ] Multiple keymaps.
- [ ] Options to pause/continue and reset ROM.
- [ ] Save/Load state.

## Building and running

Before building, make sure you have SDL2 headers available on your system. On most Linux distributions this is included on the `sdl2`, `sdl2-dev`or similarly-named package. You will also need to [install Rust](https://www.rust-lang.org/tools/install).

Once everything is installed, just run `cargo`:

```sh
$ cargo build --release

# This will download all dependencies and compile a 'rc8' release binary.
# You can get it at target/release folder
```

To run with the default options, just provide the ROM name:

```sh
# Run the binary directly
target/release/rc8 some-rom-file.ch8

# If you want, you can run directly on cargo too
$ cargo run -- some-rom-file.ch8
```

To exit the emulator, type `Esc`.

## License

See `LICENSE`.
