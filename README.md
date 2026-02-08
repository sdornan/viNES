# viNES

A vibe coded NES emulator written in Rust.

## Features

- **CPU** — Full 6502 processor emulation with all official opcodes and addressing modes
- **PPU** — Picture Processing Unit with background and sprite rendering
- **APU** — Audio Processing Unit with pulse, triangle, and noise channels
- **Cartridge** — iNES ROM format parsing with Mapper 0 (NROM) support
- **Input** — Keyboard-based controller input via SDL2
- **Save States** — Save and load emulator state with a single keypress

## Building

Requires Rust and SDL2.

```sh
cargo build --release
```

## Usage

```sh
cargo run --release -- <rom.nes>
```

## Controls

| Key         | NES Button |
|-------------|------------|
| Z           | A          |
| X           | B          |
| Enter       | Start      |
| Right Shift | Select     |
| Arrow Keys  | D-Pad      |

### Emulator

| Key    | Action     |
|--------|------------|
| F5     | Save state |
| F9     | Load state |
| Escape | Quit       |
