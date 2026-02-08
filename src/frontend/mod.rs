pub mod input;
pub mod audio;

use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::PixelFormatEnum;
use std::time::{Duration, Instant};

use crate::cartridge::Cartridge;
use crate::nes::Nes;

const SCALE: u32 = 3;
const WINDOW_WIDTH: u32 = 256 * SCALE;
const WINDOW_HEIGHT: u32 = 240 * SCALE;
const NANOS_PER_FRAME: u64 = 16_639_267; // ~60.0988 FPS (NTSC)

pub fn run(cartridge: Cartridge) -> Result<(), String> {
    let sdl_context = sdl2::init()?;
    let video = sdl_context.video()?;

    let window = video
        .window("viNES — vibe-coded NES emulator in Rust", WINDOW_WIDTH, WINDOW_HEIGHT)
        .position_centered()
        .build()
        .map_err(|e| e.to_string())?;

    let mut canvas = window
        .into_canvas()
        .accelerated()
        .build()
        .map_err(|e| e.to_string())?;

    let texture_creator = canvas.texture_creator();
    let mut texture = texture_creator
        .create_texture_streaming(PixelFormatEnum::RGB24, 256, 240)
        .map_err(|e| e.to_string())?;

    let mut event_pump = sdl_context.event_pump()?;

    // Init audio
    let (_audio_device, sample_buffer) = audio::init(&sdl_context)?;
    _audio_device.resume();

    let mut nes = Nes::new(cartridge, sample_buffer);
    nes.reset();

    let mut next_frame_time = Instant::now();
    let frame_duration = Duration::from_nanos(NANOS_PER_FRAME);
    let mut frame_count = 0u64;

    'running: loop {
        // Handle input — always pump events to keep macOS happy
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'running,
                Event::KeyDown {
                    keycode: Some(key), ..
                } => {
                    if let Some(button) = input::keycode_to_button(key) {
                        nes.bus.controller1.buttons |= button;
                    }
                }
                Event::KeyUp {
                    keycode: Some(key), ..
                } => {
                    if let Some(button) = input::keycode_to_button(key) {
                        nes.bus.controller1.buttons &= !button;
                    }
                }
                _ => {}
            }
        }

        // Only run emulation + render when it's time for the next frame
        let now = Instant::now();
        if now >= next_frame_time {
            nes.step_frame();
            frame_count += 1;

            texture
                .update(None, &nes.bus.ppu.frame.data, 256 * 3)
                .map_err(|e| e.to_string())?;
            canvas.copy(&texture, None, None)?;
            canvas.present();

            if frame_count % 60 == 0 {
                canvas.window_mut().set_title(
                    &format!("NES Emulator — frame {}", frame_count)
                ).map_err(|e| e.to_string())?;
            }

            // Schedule next frame; skip ahead if we fell behind
            next_frame_time += frame_duration;
            if now > next_frame_time {
                next_frame_time = now + frame_duration;
            }
        } else {
            // Yield CPU while waiting — short sleep to stay responsive
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    Ok(())
}
