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
    let (_stream, stream_handler) = rodio::OutputStream::try_default()
        .unwrap();
    let file = File::open(argv1)
        .unwrap();
    let reader = BufReader::new(file);
    let decoder = rodio::Decoder::new(reader)
        .unwrap();
    let track_length = decoder
        .total_duration();
    let source: SamplesConverter<_, f32> = decoder
        .convert_samples();
    let sink = Arc::new(
        rodio::Sink::try_new(&stream_handler)
            .unwrap()
    );
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
    let elapsed_time = Arc::new(Mutex::new(Duration::new(0, 0)));
    let sink_viz = Arc::clone(&sink);
    let track_length_for_viz = track_length;

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
            {
                if let Some(total) = track_length_for_viz {
                    let current = sink_viz.get_pos();
                    let progress = if total.as_secs_f32() > 0.0 {
                        (current.as_secs_f32() / total.as_secs_f32() * 20.0).round() as usize
                    } else {
                        0
                    };
                    let bar = format!("[{}{}]", "=".repeat(progress), " ".repeat(20 - progress));
                    println!("{}", bar);
                } else {
                    println!("Elapsed: {} sec", sink_viz.get_pos().as_secs());
                }
            }
            thread::sleep(Duration::from_millis(100));
        }
    });

    let sink_cmd = Arc::clone(&sink);
    let running_cmd = Arc::clone(&running);
    let cmd_handle = thread::spawn(move || {
        let stdin = io::stdin();
        let mut paused = false;
        for line in stdin.lock().lines() {
            let buffer = line.unwrap();
            match buffer.trim() {
                "" => {
                    if paused {
                        sink_cmd.play();
                        paused = false;
                        println!("Now playing track");
                        print!("{esc}[2J{esc}[1;1H", esc = 27 as char);
                    } else {
                        sink_cmd.pause();
                        paused = true;
                        println!("Paused");
                        print!("{esc}[2J{esc}[1;1H", esc = 27 as char);
                    }
                }
                "stop" => {
                    running_cmd.store(false, Ordering::Relaxed);
                    break;
                }
                "ArrowUp" | "w" => {
                    let current_volume = sink_cmd.volume();
                    sink_cmd.set_volume((current_volume + 0.1).min(1.0));
                }
                "ArrowDown" | "s" => {
                    let current_volume = sink_cmd.volume();
                    sink_cmd.set_volume((current_volume - 0.1).max(0.0));
                }
                "ArrowLeft" | "a" => {
                    let current_time = sink_cmd.get_pos();
                    let new_time = current_time
                        .checked_sub(Duration::from_secs(5))
                        .unwrap_or(Duration::new(0, 0));
                    sink_cmd.try_seek(new_time).unwrap();
                }
                "ArrowRight" | "d" => {
                    let current_time = sink_cmd.get_pos();
                    sink_cmd.try_seek(current_time + Duration::from_secs(5)).unwrap();
                }
                "0.5" => {
                    sink_cmd.set_speed(0.5);
                }
                "1" => {
                    sink_cmd.set_speed(1.0);
                }
                "1.5" => {
                    sink_cmd.set_speed(1.5);
                }
                "2" => {
                    sink_cmd.set_speed(2.0);
                }
                _ => {}
            }
        }
        running_cmd.store(false, Ordering::Relaxed)
    });
    
    while running.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_millis(100));
        if let Some(total) = track_length {
            if sink.get_pos() >= total {
                running.store(false, Ordering::Relaxed);
                println!("🏖️ Done!");
                break;
            }
        } else if sink.empty() {
            running.store(false, Ordering::Relaxed);
            println!("🏖️ Done!");
            break;
        }
    }

    let _ = cmd_handle.join();
    let _ = viz_handle.join();
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
        let bar = "=".repeat((value.abs() * 10.0) as usize);
        println!("{}", bar);
    }
}
