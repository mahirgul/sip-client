//! RFC 3550 Real-time Transport Protocol (RTP) packetizer and streaming engine.

use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::audio::AudioSource;

pub struct RtpPacket {
    pub payload_type: u8,
    pub sequence_number: u16,
    pub timestamp: u32,
    pub ssrc: u32,
    pub payload: Vec<u8>,
}

impl RtpPacket {
    pub fn new(payload_type: u8, sequence_number: u16, timestamp: u32, ssrc: u32, payload: Vec<u8>) -> Self {
        Self {
            payload_type,
            sequence_number,
            timestamp,
            ssrc,
            payload,
        }
    }

    /// Serializes RTP packet into RFC 3550 wire format (12-byte header + payload)
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(12 + self.payload.len());

        // Byte 0: V=2, P=0, X=0, CC=0 -> 0x80
        buf.push(0x80);

        // Byte 1: M=0, PT
        buf.push(self.payload_type & 0x7F);

        // Bytes 2-3: Sequence number
        buf.extend_from_slice(&self.sequence_number.to_be_bytes());

        // Bytes 4-7: Timestamp
        buf.extend_from_slice(&self.timestamp.to_be_bytes());

        // Bytes 8-11: SSRC
        buf.extend_from_slice(&self.ssrc.to_be_bytes());

        // Payload
        buf.extend_from_slice(&self.payload);

        buf
    }
}

pub struct RtpStreamer;

impl RtpStreamer {
    /// Spawns RTP sender thread and continuously streams 20ms audio frames
    pub fn spawn_stream(
        socket: Arc<UdpSocket>,
        remote_rtp_addr: SocketAddr,
        mut source: AudioSource,
        running: Arc<AtomicBool>,
        payload_type: u8,
    ) -> thread::JoinHandle<()> {
        thread::Builder::new()
            .name("sip-rtp-sender".into())
            .spawn(move || {
                let mut seq: u16 = rand::random();
                let mut timestamp: u32 = rand::random();
                let ssrc: u32 = rand::random();

                log::info!("RTP streaming started to {remote_rtp_addr} (PT={payload_type}, SSRC={ssrc})");

                let frame_duration = Duration::from_millis(20);
                let mut next_tick = Instant::now();

                while running.load(Ordering::Relaxed) {
                    if let Some(frame) = source.next_frame() {
                        let packet = RtpPacket::new(
                            payload_type,
                            seq,
                            timestamp,
                            ssrc,
                            frame.to_vec(),
                        );

                        let raw_bytes = packet.to_bytes();
                        let _ = socket.send_to(&raw_bytes, remote_rtp_addr);

                        seq = seq.wrapping_add(1);
                        timestamp = timestamp.wrapping_add(160); // 160 samples per 20ms at 8kHz
                    } else {
                        // End of audio file reached
                        log::info!("Audio playback finished.");
                        break;
                    }

                    next_tick += frame_duration;
                    let now = Instant::now();
                    if next_tick > now {
                        thread::sleep(next_tick - now);
                    }
                }

                log::info!("RTP streaming stopped.");
            })
            .expect("Failed to spawn RTP streaming thread")
    }
}
