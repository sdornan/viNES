use std::fs;
use std::sync::Arc;
use crossbeam::queue::ArrayQueue;
use nes_emu::cartridge::Cartridge;
use nes_emu::cpu::Cpu;
use nes_emu::bus::Bus;

fn main() {
    let rom_data = fs::read("nestest.nes").expect("Could not read nestest.nes - place it in the project root");
    let cartridge = Cartridge::from_ines(&rom_data).expect("Failed to parse nestest.nes");

    let sample_buffer = Arc::new(ArrayQueue::new(4096));
    let mut cpu = Cpu::new();
    let mut bus = Bus::new(cartridge, sample_buffer);

    // nestest automated mode starts at $C000
    cpu.pc = 0xC000;
    cpu.status = nes_emu::cpu::CpuFlags::from_bits_truncate(0x24);
    cpu.sp = 0xFD;
    cpu.cycles = 7;

    let max_steps = 8991;
    for i in 0..max_steps {
        let trace = cpu.trace(&mut bus);

        // Print first 20 lines and any that diverge
        if i < 20 {
            println!("{}", trace);
        }

        cpu.step(&mut bus);

        // Check nestest error codes at $02 and $03
        let err1 = bus.cpu_read(0x0002);
        let err2 = bus.cpu_read(0x0003);
        if (err1 != 0 || err2 != 0) && i > 0 {
            println!("\n--- NESTEST FAILURE at step {} ---", i);
            println!("Last trace: {}", trace);
            println!("Error code $02 = 0x{:02X}, $03 = 0x{:02X}", err1, err2);
            println!("These indicate which test group failed.");
            std::process::exit(1);
        }
    }

    let err1 = bus.cpu_read(0x0002);
    let err2 = bus.cpu_read(0x0003);
    println!("\n--- NESTEST COMPLETE ---");
    println!("Ran {} steps", max_steps);
    println!("$02 = 0x{:02X} (official opcodes result)", err1);
    println!("$03 = 0x{:02X} (unofficial opcodes result)", err2);
    if err1 == 0 && err2 == 0 {
        println!("ALL TESTS PASSED!");
    } else {
        println!("SOME TESTS FAILED");
        std::process::exit(1);
    }
}
