use core::net;
use std::{collections::VecDeque, error::Error, f32::MIN, io::{self, Write}, option, sync::{Arc, mpsc::{self, Receiver, TryRecvError}}};

use cpal::{Host, Stream, SupportedStreamConfig, traits::{DeviceTrait, HostTrait, StreamTrait}};
use spectrum_analyzer::{FrequencyLimit, samples_fft_to_spectrum, scaling::{divide_by_N, scale_20_times_log10}, windows::hann_window};

// const FREQ_RANGE: f32 = 1000.0;
const MIN_HZ: f32 = 32.0;
const MAX_HZ: f32 = 13000.0;
const WINDOW_SIZE: usize = 512;
const GRAVITY_STEP: f32 = 0.1;
const FFT_SCALE: f32 = 8.2;
const FFT_CUTOFF: f32 = 0.3;
const SMOOTHING_BOUNDS: (usize, usize) = (1, 1);
// const SAMPLE_RATE: u32 = 22000;

pub struct AudioManager {
    receiver: Arc<Receiver<Vec<f32>>>,
    stream: Stream,
    config: SupportedStreamConfig,
    peak: f32,
}

impl AudioManager {
    pub fn new() -> Result<AudioManager, Box<dyn Error>> {
        let host = cpal::default_host();
        let device = AudioManager::get_user_device(host)?;
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

        Ok(AudioManager {
            receiver: Arc::new(rx),
            stream,
            config: supported_config,
            peak: f32::MIN,
        })
    }

    pub fn get_user_device(host: Host) -> Result<cpal::Device, Box<dyn Error>> {
        let devices: Vec<_> = host.input_devices()?.collect();

        if devices.is_empty() {
            return Err("No audio input devices found".into());
        }

        println!("Available audio input sources:");
        for (index, device) in devices.iter().enumerate() {
            println!("  {index}: {} ({})", device.description()?, device.id()?);
        }

        let index = loop {
            print!("Choose an input source [0-{}]: ", devices.len() - 1);
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;

            match input.trim().parse::<usize>() {
                Ok(index) if index < devices.len() => break index,
                _ => println!("Invalid selection. Please try again."),
            }
        };

        devices
            .into_iter()
            .nth(index)
            .ok_or_else(|| format!("Invalid source index: {index}").into())
    }

    pub fn get_volumes(&mut self) -> Result<Vec<(f32, f32)>, Box<dyn Error>> {
        let channels = self.config.channels() as usize;
        let mut buffer = Vec::with_capacity(WINDOW_SIZE);

        while buffer.len() < WINDOW_SIZE {
            let chunk = self.receiver.recv()?;

            for frame in chunk.chunks_exact(channels) {
                let mono = frame.iter().sum::<f32>() / channels as f32;
                buffer.push(mono);

                if buffer.len() == WINDOW_SIZE {
                    break;
                }
            }
        }

        let windowed = hann_window(&buffer);

        let scaling_fn = |value: f32, stats: &_| {
            scale_20_times_log10(divide_by_N(value, stats), stats)
        };

        let spectrum = samples_fft_to_spectrum(
            &windowed,
            self.config.sample_rate(),
            FrequencyLimit::Range(MIN_HZ.into(), MAX_HZ.into()),
            None,
        )?;

        let freqs = spectrum.data().iter().map(
            |(freq, vol)| (freq.val(), vol.val())
        );

        let mut scaled_freqs: Vec<(f32, f32)> = Vec::with_capacity(freqs.len());
        
        for (index, current) in freqs.enumerate() {
            let position = index as f32 / spectrum.data().len() as f32;
            let max_position = scaled_freqs.len() as f32 / spectrum.data().len() as f32;

            let weight = f32::max(position * FFT_SCALE + (1.0 - FFT_CUTOFF), 1.0);
            // let weight = 1.0;
            let magnitude = current.1.abs();

            scaled_freqs.push((current.0, ((magnitude + 1.0).log10() / 3.0) * weight));
        }

        let mut smoothed: Vec<(f32, f32)> = Vec::with_capacity(scaled_freqs.len());
        for i in 0..scaled_freqs.len() {
            let start = i.saturating_sub(SMOOTHING_BOUNDS.0);
            let end = (i + SMOOTHING_BOUNDS.1).min(scaled_freqs.len());
                
            smoothed.push((
                scaled_freqs[start].0,
                scaled_freqs[start..end]
                    .iter()
                    .map(|(_, magnitude)| magnitude)
                    .sum::<f32>()
                / (end - start) as f32
            ));
        }

        let frame_peak = smoothed
            .iter()
            .map(|(_, magnitude)| *magnitude)
            .fold(0.0_f32, f32::max);

        if frame_peak > self.peak {
            self.peak = frame_peak;
        }

        let mut combined_freqs: Vec<(f32, f32)> = Vec::new();

        let mut i: usize = 0;
        for (freq, val) in smoothed.iter() {
            let upper_limit = f32::powf(2.0, (i+1) as f32 + MIN_HZ.log2());
            if *freq < upper_limit {
                combined_freqs.get_mut(i).unwrap().1 += *val / self.peak;
            } else {
                combined_freqs.push((upper_limit, *val / self.peak));
                i += 1;
            }
        }

        self.peak -= GRAVITY_STEP;
        
        Ok(combined_freqs)
    }
}