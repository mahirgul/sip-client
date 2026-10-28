# sip-client 📞🦀

[![Crates.io](https://img.shields.io/crates/v/sip-client.svg)](https://crates.io/crates/sip-client)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-stable-brightgreen.svg)](https://www.rust-lang.org/)
[![Platform](https://img.shields.io/badge/Platform-Linux%20%7C%20Windows-blue.svg)](https://github.com/mahirgul/sip-client/releases)
[![Website](https://img.shields.io/badge/Live-Demo%20%26%20Docs-cyan.svg)](https://mahirgul.github.io/sip-client/)

A lightweight, zero-bloat, cross-platform **SIP client** in 100% Rust. Runs in the terminal on both **Linux** and **Windows**, and as a native Win32 application.

Designed for automated dialing, notification calls, testing, and audio broadcasting: automatically streams WAV audio files over RTP (G.711u / G.711a) immediately upon call answer.

---

## ✨ Features

- **Zero Bloat & Ultra-Fast**: Pure Rust implementation with no heavy C library dependencies.
- **Cross-Platform**: Native terminal client on both **Linux** and **Windows**, with native **Win32** dialog integration.
- **Automated Audio Broadcast**:
  - Automatically streams any `.wav` audio file over RTP (G.711 $\mu$-law / A-law) as soon as the call connects (`200 OK`).
  - `--loop`: Continuously loop audio during the call.
  - `--hangup`: Automatically send `BYE` and disconnect once the audio file finishes playing.
- **SIP RFC 3261 Compliant**:
  - Supports `INVITE`, `ACK`, `BYE`, `CANCEL`, `REGISTER`, `OPTIONS`.
  - Supports **SIP Digest Authentication** (MD5 RFC 2617) for secure PBX and registrar connections.
- **Inbound Auto-Answer Mode (`listen`)**:
  - Listen for incoming calls, answer with `200 OK`, and automatically play a pre-recorded message or announcement.
- **Built-In Audio Generator (`gen-wav`)**:
  - Generate clean 8kHz test WAV files on the fly.

---

## 🚀 Quick Start

### Installation via Cargo

```bash
cargo install sip-client
```

Or download pre-compiled binaries for Windows (`sip-client.exe`) and Linux from [GitHub Releases](https://github.com/mahirgul/sip-client/releases).

---

## 📖 Usage Examples

### 1. Place a Call and Play an Audio File (Auto-Hangup when done)

```bash
# Dial destination and play audio message, then hang up:
sip-client call sip:1000@192.168.1.1 --play announcement.wav --hangup
```

### 2. Place an Authenticated PBX Call

```bash
sip-client call sip:200@pbx.company.com \
  --user 101 \
  --password secret \
  --play welcome.wav \
  --hangup
```

### 3. Continuous Looping Audio Call

```bash
sip-client call sip:music@10.0.0.1 --play holdmusic.wav --loop
```

### 4. Inbound Auto-Answer & Announcement Server

```bash
# Listen on port 5060, answer incoming calls, and stream message
sip-client listen --bind 0.0.0.0:5060 --play welcome.wav --loop
```

### 5. Generate Test Audio File

```bash
sip-client gen-wav test_audio.wav --duration 5
```

### 6. Windows Win32 Notification Mode

```powershell
# Runs on Windows and displays a native Win32 dialog on status
.\sip-client.exe call sip:100@192.168.1.50 --play alert.wav --hangup --win32
```

---

## ⚙️ CLI Reference

```
Usage: sip-client <COMMAND>

Commands:
  call     Make an outgoing SIP call and optionally stream an audio WAV file on answer
  listen   Listen for incoming SIP calls, auto-answer, and stream audio
  gen-wav  Generate a sample 8kHz mono test WAV audio file
  help     Print this message or the help of the given subcommand(s)
```

---

## 📄 License

Distributed under the **MIT License**. See [`LICENSE`](LICENSE) for details.

---

## ☁️ Cloud Infrastructure Partner

[![UpCloud](https://img.shields.io/badge/UpCloud-$250_Free_Credit-7b1fa2?style=flat&logo=cloud&logoColor=white)](https://signup.upcloud.com/?promo=affiliate-aipbx250&amp;utm_source=aipbx&amp;utm_medium=affiliate&amp;utm_campaign=affiliate-aipbx250)

Deploy VoIP PBXs, Asterisk systems, or Rust microservices on [UpCloud](https://signup.upcloud.com/?promo=affiliate-aipbx250&amp;utm_source=aipbx&amp;utm_medium=affiliate&amp;utm_campaign=affiliate-aipbx250) with high-speed MaxIOPS storage. Get **$250 in free cloud credits** with promo code `affiliate-aipbx250`.

