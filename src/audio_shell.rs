// Reference: https://github.com/sourcebox/audio-midi-shell-rs/blob/master/src/lib.rs 

use rdev::Event;
use tinyaudio::{run_output_device, OutputDevice, OutputDeviceParameters};
use std::sync::{Arc, Mutex};
use std::marker::PhantomData;


/// Shell running the audio processing.
pub struct AudioShell<'a> {
    _output_device: OutputDevice,
    _marker: PhantomData<&'a ()>,
}

impl<'a> AudioShell<'a> {
    /// Initializes the output device and runs the generator in a callback.
    /// It returns a shell object that must be kept alive.
    /// - `sample_rate` is the sampling frequency in Hz.
    /// - `block_size` is the number of samples for the `process` function.
    pub fn spawn(
        sample_rate: u32,
        block_size: usize,
        generator: Arc<Mutex<impl AudioGenerator + Send + 'static>>,
    ) -> Self {
        generator.lock().unwrap().init(block_size);

        let params = OutputDeviceParameters {
            channels_count: 2,
            sample_rate: sample_rate as usize,
            channel_sample_count: block_size,
        };

        let output_device = run_output_device(params, move |data| {
            let mut samples_left = vec![0.0; block_size];
            let mut samples_right = vec![0.0; block_size];
            generator.lock().unwrap().process(&mut samples_left, &mut samples_right);

            for (frame_no, samples) in data.chunks_mut(params.channels_count).enumerate() {
                samples[0] = samples_left[frame_no];
                samples[1] = samples_right[frame_no];
            }
        }).unwrap();

        Self {
            _output_device: output_device, 
            _marker: PhantomData
        }
    }
}

/// Trait to be implemented by structs that are passed as generator to the shell.
pub trait AudioGenerator {
    /// Initializes the generator. Called once inside the shell `run` function.
    fn init(&mut self, _block_size: usize) {}

    /// Generates a block of samples.
    /// `samples_left` and `samples_right` are buffers of the block size passed to the shell `run`
    /// function. They are initialized to `0.0` and must be filled with sample data.
    fn process(&mut self, samples_left: &mut [f32], samples_right: &mut [f32]);

    /// Processes keyboard input.
    fn process_events(&mut self, _event: Event) {}
}
