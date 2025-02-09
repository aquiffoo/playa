use std::fs::File;
use std::io::{self, BufReader, BufRead, Write};
use rodio::Source;
use rodio::source::SamplesConverter;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::io::Read;
use std::f32::consts::PI;
use std::{thread, time::Duration};

fn clear() {
    print!("{esc}[2J{esc}[1;1H", esc = 27 as char);
}

fn main() {
    clear();
    let argv1 = std::env::args()
        .nth(1)
        .expect("🏖️: No file path provided!");
    let (_stream, stream_handler) = rodio::OutputStream::try_default().unwrap();
    let file = File::open(argv1).unwrap();
    let reader = BufReader::new(file);
    let decoder = rodio::Decoder::new(reader).unwrap();
    let track_length = decoder.total_duration();
    let source: SamplesConverter<_, f32> = decoder.convert_samples();
    let sink = Arc::new(rodio::Sink::try_new(&stream_handler).unwrap());
    sink.append(source);

    let audio_data = Arc::new(Mutex::new(Vec::new()));
    let audio_data_clone = Arc::clone(&audio_data);

    let host = cpal::default_host();
    let device = host.default_input_device().expect("No input device available");
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
    let paused_flag = Arc::new(AtomicBool::new(false));
    let speed = Arc::new(Mutex::new(1.0));

    let running_viz = Arc::clone(&running);
    let paused_viz = Arc::clone(&paused_flag);
    let audio_data_viz = Arc::clone(&audio_data);
    let sink_viz = Arc::clone(&sink);
    let track_length_for_viz = track_length;
    let speed_viz = Arc::clone(&speed);

    let viz_handle = thread::spawn(move || {
        clear();
        println!("🏖️'s Bindings:\nEnter: Play/Pause\nstop: Stops playing\nw/s: Vol+/Vol-\na/d: Rewind/Forward 5sec\n0.5/1/1.5/2: Speed");
        while running_viz.load(Ordering::Relaxed) {
            {
                let mut data = audio_data_viz.lock().unwrap();
                if !data.is_empty() {
                    data.clear();
                }
            }
            {
                let current = sink_viz.get_pos();
                let speed_current = *speed_viz.lock().unwrap();
                let progress_str = if let Some(total) = track_length_for_viz {
                    let total_secs = total.as_secs_f32();
                    let effective_total = if speed_current > 0.0 { total_secs / speed_current } else { total_secs };
                    let progress = if effective_total > 0.0 {
                        (sink_viz.get_pos().as_secs_f32() / effective_total * 20.0).round() as usize
                    } else {
                        0
                    };
                    let status = if paused_viz.load(Ordering::Relaxed) { "⏸ PAUSED" } else { "▶ PLAYING" };
                    let status_with_speed = format!("{} [{}x]", status, speed_current);
                    format!(
                        "{} [{}{}] {}s / {}s",
                        status_with_speed,
                        "=".repeat(progress),
                        " ".repeat(20 - progress),
                        sink_viz.get_pos().as_secs(),
                        total.as_secs()
                    )
                } else {
                    let progress = (sink_viz.get_pos().as_secs() as usize % 21).min(20);
                    let status = if paused_viz.load(Ordering::Relaxed) { "⏸ PAUSED" } else { "▶ PLAYING" };
                    let status_with_speed = format!("{} [{}x]", status, speed_current);
                    format!(
                        "{} [{}{}] {}s",
                        status_with_speed,
                        "=".repeat(progress),
                        " ".repeat(20 - progress),
                        sink_viz.get_pos().as_secs()
                    )
                };
                print!("\r{}", progress_str);
                io::stdout().flush().unwrap();
            }
            thread::sleep(Duration::from_millis(250));
        }
        println!();
    });

    let sink_cmd = Arc::clone(&sink);
    let running_cmd = Arc::clone(&running);
    let paused_cmd = Arc::clone(&paused_flag);
    let speed_cmd = Arc::clone(&speed);
    let cmd_handle = thread::spawn(move || {
        let stdin = io::stdin();
        let mut stdin_lock = stdin.lock();
        let mut bytes_iter = stdin_lock.bytes();
        let mut command_buffer = String::new();
        while running_cmd.load(Ordering::Relaxed) {
            if let Some(Ok(byte)) = bytes_iter.next() {
                if byte == 0x1B {
                    let mut seq = vec![byte];
                    if let Some(Ok(b)) = bytes_iter.next() {
                        seq.push(b);
                    }
                    if let Some(Ok(b)) = bytes_iter.next() {
                        seq.push(b);
                    }
                    let seq_str = String::from_utf8(seq).unwrap();
                    match seq_str.as_str() {
                        "\x1B[A" => {
                            let current_volume = sink_cmd.volume();
                            sink_cmd.set_volume((current_volume + 0.1).min(1.0));
                            clear();
                        }
                        "\x1B[B" => {
                            let current_volume = sink_cmd.volume();
                            sink_cmd.set_volume((current_volume - 0.1).max(0.0));
                            clear();
                        }
                        "\x1B[D" => {
                            let current_time = sink_cmd.get_pos();
                            let new_time = current_time
                                .checked_sub(Duration::from_secs(5))
                                .unwrap_or(Duration::new(0, 0));
                            sink_cmd.try_seek(new_time).unwrap();
                            clear();
                        }
                        "\x1B[C" => {
                            let current_time = sink_cmd.get_pos();
                            sink_cmd.try_seek(current_time + Duration::from_secs(5)).unwrap();
                            clear();
                        }
                        _ => {}
                    }
                } else if byte == b'\r' || byte == b'\n' {
                    if command_buffer.is_empty() {
                        if paused_cmd.load(Ordering::Relaxed) {
                            sink_cmd.play();
                            paused_cmd.store(false, Ordering::Relaxed);
                            clear();
                            println!("\nResuming playback");
                        } else {
                            sink_cmd.pause();
                            paused_cmd.store(true, Ordering::Relaxed);
                            clear();
                            println!("\nPlayback Paused");
                        }
                    } else {
                        let cmd = command_buffer.trim();
                        if cmd == "stop" {
                            running_cmd.store(false, Ordering::Relaxed);
                            clear();
                            break;
                        }
                        command_buffer.clear();
                    }
                } else {
                    let ch = byte as char;
                    match ch {
                        'w' | 's' | 'a' | 'd' => {
                            match ch {
                                'w' => {
                                    let current_volume = sink_cmd.volume();
                                    sink_cmd.set_volume((current_volume + 0.1).min(1.0));
                                }
                                's' => {
                                    let current_volume = sink_cmd.volume();
                                    sink_cmd.set_volume((current_volume - 0.1).max(0.0));
                                }
                                'a' => {
                                    let current_time = sink_cmd.get_pos();
                                    let new_time = current_time
                                        .checked_sub(Duration::from_secs(5))
                                        .unwrap_or(Duration::new(0, 0));
                                    sink_cmd.try_seek(new_time).unwrap();
                                }
                                'd' => {
                                    let current_time = sink_cmd.get_pos();
                                    sink_cmd.try_seek(current_time + Duration::from_secs(5)).unwrap();
                                }
                                _ => {}
                            }
                            clear();
                        }
                        _ => {
                            command_buffer.push(ch);
                            if command_buffer == "stop" {
                                running_cmd.store(false, Ordering::Relaxed);
                                clear();
                                break;
                            } else if command_buffer == "0.5" {
                                sink_cmd.set_speed(0.5);
                                *speed_cmd.lock().unwrap() = 0.5;
                                clear();
                                command_buffer.clear();
                            } else if command_buffer == "1" {
                                sink_cmd.set_speed(1.0);
                                *speed_cmd.lock().unwrap() = 1.0;
                                clear();
                                command_buffer.clear();
                            } else if command_buffer == "1.5" {
                                sink_cmd.set_speed(1.5);
                                *speed_cmd.lock().unwrap() = 1.5;
                                clear();
                                command_buffer.clear();
                            } else if command_buffer == "2" {
                                sink_cmd.set_speed(2.0);
                                *speed_cmd.lock().unwrap() = 2.0;
                                clear();
                                command_buffer.clear();
                            } else if command_buffer.len() > 4 {
                                command_buffer.clear();
                            }
                        }
                    }
                }
            }
        }
        running_cmd.store(false, Ordering::Relaxed);
    });
    
    while running.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_millis(100));
        if let Some(total) = track_length {
            if sink.get_pos() >= total {
                running.store(false, Ordering::Relaxed);
                println!("\n🏖️ Done!");
                break;
            }
        } else if sink.empty() {
            running.store(false, Ordering::Relaxed);
            println!("\n🏖️ Done!");
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
