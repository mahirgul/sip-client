use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use anyhow::Result;
use clap::Parser;
use hound::{WavSpec, WavWriter};

mod audio;
mod cli;
mod client;
mod rtp;
mod sip;
mod win32;

use cli::{Cli, Commands};
use client::{SipClient, SipClientConfig};
use win32::show_win32_dialog;

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let cli = Cli::parse();

    let running = Arc::new(AtomicBool::new(true));
    let r_clone = Arc::clone(&running);

    ctrlc::set_handler(move || {
        println!("\n[Signal] Received Ctrl+C. Gracefully shutting down...");
        r_clone.store(false, Ordering::Relaxed);
    })?;

    match cli.command {
        Commands::Call {
            destination,
            play,
            loop_audio,
            hangup,
            user,
            auth_user,
            password,
            server,
            bind,
            win32,
        } => {
            println!("=== sip-client v0.4.0 ===");
            let config = SipClientConfig {
                destination: destination.clone(),
                caller_id: user,
                auth_user,
                password,
                play_wav: play,
                loop_audio,
                auto_hangup: hangup,
                bind_sip: bind,
                server_proxy: server,
            };

            let res = SipClient::make_call(config, running);
            if let Err(ref e) = res {
                eprintln!("[Error]: {e}");
                if win32 {
                    show_win32_dialog("sip-client Error", &format!("Call to {destination} failed: {e}"));
                }
            } else if win32 {
                show_win32_dialog("sip-client", &format!("Call to {destination} completed successfully."));
            }
            res?;
        }

        Commands::Listen {
            bind,
            play,
            loop_audio,
        } => {
            println!("=== sip-client v0.4.0 (Inbound Listener) ===");
            SipClient::listen(&bind, play, loop_audio, running)?;
        }

        Commands::GenWav {
            output,
            duration_secs,
        } => {
            generate_test_wav(&output, duration_secs)?;
            println!("Generated test WAV audio file at {:?}", output);
        }
    }

    Ok(())
}

fn generate_test_wav(path: &Path, duration_secs: u32) -> Result<()> {
    let spec = WavSpec {
        channels: 1,
        sample_rate: 8000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = WavWriter::create(path, spec)?;
    let total_samples = 8000 * duration_secs;
    let freq = 440.0;

    for i in 0..total_samples {
        let t = i as f32 / 8000.0;
        let sample = (t * freq * 2.0 * std::f32::consts::PI).sin();
        let val = (sample * 16000.0) as i16;
        writer.write_sample(val)?;
    }

    writer.finalize()?;
    Ok(())
}
