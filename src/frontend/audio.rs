use crossbeam::queue::ArrayQueue;
use sdl2::audio::{AudioCallback, AudioDevice, AudioSpecDesired};
use std::sync::Arc;

const SAMPLE_RATE: i32 = 44_100;
const BUFFER_CAPACITY: usize = 4096;

struct NesAudio {
    sample_buffer: Arc<ArrayQueue<f32>>,
}

impl AudioCallback for NesAudio {
    type Channel = f32;

    fn callback(&mut self, out: &mut [f32]) {
        for sample in out.iter_mut() {
            *sample = self.sample_buffer.pop().unwrap_or(0.0);
        }
    }
}

pub fn init(
    sdl_context: &sdl2::Sdl,
) -> Result<(AudioDevice<impl AudioCallback>, Arc<ArrayQueue<f32>>), String> {
    let audio = sdl_context.audio()?;
    let sample_buffer = Arc::new(ArrayQueue::new(BUFFER_CAPACITY));

    let spec = AudioSpecDesired {
        freq: Some(SAMPLE_RATE),
        channels: Some(1),
        samples: Some(1024),
    };

    let device = audio.open_playback(None, &spec, |_obtained| NesAudio {
        sample_buffer: sample_buffer.clone(),
    })?;

    Ok((device, sample_buffer))
}
