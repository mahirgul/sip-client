//! Command-line interface definitions for sip-client.

use std::path::PathBuf;
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "sip-client",
    author = "Mahir Gül <mahirgul@gmail.com>",
    version = "3.0.0",
    about = "Lightweight cross-platform terminal & Win32 SIP client with automated audio playback and RTP streaming",
    long_about = "A simple, clean, lightweight SIP client in 100% Rust. Place calls or answer incoming calls from the terminal or Win32, and automatically stream WAV audio files over RTP (G.711u/a)."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Make an outgoing SIP call and optionally stream an audio WAV file on answer
    Call {
        /// Target SIP destination (e.g., sip:1000@192.168.1.50 or 192.168.1.50:5060)
        destination: String,

        /// Path to WAV audio file to automatically stream over RTP when answered
        #[arg(short = 'p', long = "play")]
        play: Option<PathBuf>,

        /// Continuously loop the audio file during the call
        #[arg(short = 'l', long = "loop")]
        loop_audio: bool,

        /// Automatically hang up (send BYE) once the audio file finishes playing
        #[arg(long = "hangup")]
        hangup: bool,

        /// Caller ID / username (default: sip-client)
        #[arg(short = 'u', long = "user", default_value = "sip-client")]
        user: String,

        /// Authentication username for SIP Digest Auth (if different from caller)
        #[arg(short = 'a', long = "auth-user")]
        auth_user: Option<String>,

        /// Password for SIP Digest Auth
        #[arg(short = 'w', long = "password")]
        password: Option<String>,

        /// SIP proxy or PBX server address (e.g. 192.168.1.1:5060)
        #[arg(short = 's', long = "server")]
        server: Option<String>,

        /// Local SIP UDP bind address (default: 0.0.0.0:0 for dynamic port)
        #[arg(short = 'b', long = "bind", default_value = "0.0.0.0:0")]
        bind: String,

        /// Show native Win32 notification dialog on call status (Windows)
        #[arg(long = "win32")]
        win32: bool,
    },

    /// Listen for incoming SIP calls, auto-answer, and stream audio
    Listen {
        /// Local SIP UDP bind address (default: 0.0.0.0:5060)
        #[arg(short = 'b', long = "bind", default_value = "0.0.0.0:5060")]
        bind: String,

        /// Path to WAV audio file to stream to incoming callers
        #[arg(short = 'p', long = "play")]
        play: Option<PathBuf>,

        /// Loop the audio file continuously for caller
        #[arg(short = 'l', long = "loop")]
        loop_audio: bool,
    },

    /// Generate a sample 8kHz mono test WAV audio file
    GenWav {
        /// Output path for test WAV file (e.g., test.wav)
        #[arg(default_value = "test.wav")]
        output: PathBuf,

        /// Duration of the tone in seconds (default: 5)
        #[arg(short = 'd', long = "duration", default_value_t = 5)]
        duration_secs: u32,
    },
}
