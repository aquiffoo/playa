use std::fs::File;
use std::io::{self, BufReader, BufRead};
use rodio::Source;
use rodio::source::SamplesConverter;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::f32::consts::PI;
use std::{thread, time::Duration};

fn main() {
    print!("{esc}[2J{esc}[1;1H", esc = 27 as char);
    let argv1 = std::env::args()
        .nth(1)
        .expect("🏖️: No file path provided!");
    let (_stream, stream_handler) = rodio::OutputStream::try_default().unwrap();
    let file = File::open(argv1).unwrap();
    let source: SamplesConverter<_, f32> = rodio::Decoder::new(BufReader::new(file))
        .unwrap()
        .convert_samples();
    let sink = rodio::Sink::try_new(&stream_handler).unwrap();
    sink.append(source);

    let audio_data = Arc::new(Mutex::new(Vec::new()));
    let audio_data_clone = Arc::clone(&audio_data);

    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .expect("No input device available");
    let config = device
        .default_input_config()
        .expect("No default input format")
        .config();

    let stream = device.build_input_stream(
        &config,
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            let mut audio_data = audio_data_clone.lock().unwrap();
            audio_data.extend_from_slice(data);
        },
        |err| eprintln!("An error occurred on the input audio stream: {}", err),
        None,
    ).unwrap();

    stream.play().unwrap();

    let running = Arc::new(AtomicBool::new(true));
    let running_viz = Arc::clone(&running);
    let audio_data_viz = Arc::clone(&audio_data);

    let viz_handle = thread::spawn(move || {
        while running_viz.load(Ordering::Relaxed) {
            {
                let mut data = audio_data_viz.lock().unwrap();
                if !data.is_empty() {
                    print!("{esc}[2J{esc}[1;1H", esc = 27 as char);
                    let spectrum = fft(&data);
                    view(&spectrum);
                    data.clear();
                }
            }
            thread::sleep(Duration::from_millis(100));
        }
    });

    let stdin = io::stdin();
    let mut paused = false;
    for line in stdin.lock().lines() {
        let buffer = line.unwrap();
        if buffer.trim().is_empty() {
            if paused {
                sink.play();
                paused = false;
                println!("Now playing track");
                print!("{esc}[2J{esc}[1;1H", esc = 27 as char);
            } else {
                sink.pause();
                paused = true;
                println!("Paused");
                print!("{esc}[2J{esc}[1;1H", esc = 27 as char);
            }
        } else if buffer.trim() == "stop" {
            running.store(false, Ordering::Relaxed);
            break;
        }
    }
    viz_handle.join().unwrap();
}

fn fft(signal: &[f32]) -> Vec<f32> {
    let n = signal.len();
    let mut spectrum = vec![0.0; n];
    for k in 0..n {
        let mut sum = 0.0;
        for t in 0..n {
            let angle = 2.0 * PI * (k as f32) * (t as f32) / (n as f32);
            sum += signal[t] * angle.cos();
        }
        spectrum[k] = sum;
    }
    spectrum
}

fn view(spectrum: &[f32]) {
    for &value in spectrum {
        let bar = "⬜".repeat((value.abs() * 10.0) as usize);
        println!("{}", bar);
    }
}
