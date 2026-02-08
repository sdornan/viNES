use std::env;
use std::fs;
use std::process;

use nes_emu::cartridge::Cartridge;
use nes_emu::frontend;

fn main() {
    env_logger::init();

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <rom.nes>", args[0]);
        process::exit(1);
    }

    let rom_path = &args[1];
    let rom_data = fs::read(rom_path).unwrap_or_else(|e| {
        eprintln!("Failed to read ROM file '{}': {}", rom_path, e);
        process::exit(1);
    });

    let cartridge = Cartridge::from_ines(&rom_data).unwrap_or_else(|e| {
        eprintln!("Failed to parse ROM: {}", e);
        process::exit(1);
    });

    if let Err(e) = frontend::run(cartridge) {
        eprintln!("Emulator error: {}", e);
        process::exit(1);
    }
}
