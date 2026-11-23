use std::{collections::VecDeque, error::Error, process::Command, sync::{Arc, Mutex, atomic::Ordering, mpsc::{self, Receiver}}, thread, time::Duration};

use cpal::{Stream, SupportedStreamConfig, traits::{DeviceTrait, HostTrait, StreamTrait}};
use spectrum_analyzer::{FrequencyLimit, samples_fft_to_spectrum, scaling::divide_by_N, windows::hann_window};

pub struct AudioManager {
    receiver: Arc<Receiver<Vec<f32>>>,
    stream: Stream,
    config: SupportedStreamConfig,
}

impl AudioManager {
    pub fn new() -> Result<AudioManager, Box<dyn Error>> {
        let host = cpal::default_host();
        let device = host
            .input_devices()?
            .find(|d| {
                d.description()
                    .map(|desc| desc.to_string().to_lowercase().contains("monitor"))
                    .unwrap_or(false)
            })
            .or_else(|| host.default_input_device())
            .ok_or("No audio input device found")?;
        println!("Device: {} : {}", device.description()?, device.id()?);

        let mut supported_configs_range = device.supported_input_configs()?;
        let supported_config = supported_configs_range.next()
            .expect("No config :/")
            .with_max_sample_rate();

        let (tx, rx) = mpsc::channel::<Vec<f32>>();

        let stream = device.build_input_stream(
            supported_config.config(),
            move |data: &[f32], _| {
                let _ = tx.send(data.to_vec());
            },
            |err| eprintln!("Stream error: {err}"),
            None,
        )?;
        stream.play()?;

        Ok(AudioManager { receiver: Arc::new(rx), stream, config: supported_config })
    }

    pub fn get_volumes(&self) -> Result<Vec<f32>, Box<dyn Error>> {
        let window_size = 2048;
        let mut buffer = Vec::with_capacity(window_size);

        while buffer.len() < window_size {
            let chunk = self.receiver.recv()?;
            let needed = window_size - buffer.len();
            buffer.extend(chunk.into_iter().take(needed));
        }

        let data: Vec<f32> = self.receiver.recv()?.iter().map(|d| d * 10000.0).collect();
        println!("new");
        // for d in data.iter() {
            // println!("  ---  {}", d);
        // }
        let windowed = hann_window(data.as_slice());

        let spectrum = samples_fft_to_spectrum(
            &windowed,
            self.config.sample_rate(),
            FrequencyLimit::All,
            Some(&divide_by_N),
        )?;

        let (frequency, value) = spectrum.max();

        Ok(vec![frequency.val(), value.val()])
    }
}