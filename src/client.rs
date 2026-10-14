//! Core SIP User Agent engine handling calls, registration, incoming calls, and RTP audio playback.

use std::net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs, UdpSocket};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use anyhow::{bail, Context, Result};

use crate::audio::AudioSource;
use crate::rtp::RtpStreamer;
use crate::sip::{
    build_sdp, compute_digest_response, parse_auth_header, random_id, SipMessage, SipType,
};

pub struct SipClientConfig {
    pub destination: String,
    pub caller_id: String,
    pub auth_user: Option<String>,
    pub password: Option<String>,
    pub play_wav: Option<PathBuf>,
    pub loop_audio: bool,
    pub auto_hangup: bool,
    pub bind_sip: String,
    pub server_proxy: Option<String>,
}

pub struct SipClient;

impl SipClient {
    /// Outgoing Call: sends INVITE, negotiates SDP, and immediately streams audio file upon 200 OK
    pub fn make_call(config: SipClientConfig, running: Arc<AtomicBool>) -> Result<()> {
        let sip_socket = UdpSocket::bind(&config.bind_sip)
            .with_context(|| format!("Failed to bind local SIP socket on {}", config.bind_sip))?;
        sip_socket.set_read_timeout(Some(Duration::from_millis(500)))?;

        let local_sip_addr = sip_socket.local_addr()?;
        let local_ip = detect_local_ip(&sip_socket);

        // Bind local RTP socket
        let rtp_socket = Arc::new(UdpSocket::bind("0.0.0.0:0")?);
        let local_rtp_port = rtp_socket.local_addr()?.port();

        // Resolve target address
        let target_addr = resolve_target_addr(&config.destination, config.server_proxy.as_deref())?;
        println!("[SIP] Calling {} via {}", config.destination, target_addr);

        let call_id = random_id("call");
        let from_tag = random_id("tag");
        let mut cseq = 1;
        let mut auth_header: Option<String> = None;

        let audio_source = if let Some(wav_path) = &config.play_wav {
            println!("[Audio] Loaded WAV audio file: {:?}", wav_path);
            AudioSource::from_wav_file(wav_path, false, config.loop_audio)?
        } else {
            println!("[Audio] No audio file specified. Using 440Hz test tone.");
            AudioSource::test_tone(false)
        };

        let mut to_tag: Option<String> = None;
        let mut remote_rtp_addr: Option<SocketAddr> = None;

        // INVITE loop (handles 401/407 authentication challenges)
        let mut call_established = false;
        let invite_deadline = Instant::now() + Duration::from_secs(35);

        for attempt in 0..3 {
            let branch = random_id("z9hG4bK");
            let sdp = build_sdp(&local_ip.to_string(), local_rtp_port);

            let mut invite_req = format!(
                "INVITE {dest_uri} SIP/2.0\r\n\
Via: SIP/2.0/UDP {local_ip}:{local_port};branch={branch}\r\n\
Max-Forwards: 70\r\n\
From: <{caller}>;tag={from_tag}\r\n\
To: <{dest_uri}>\r\n\
Call-ID: {call_id}\r\n\
CSeq: {cseq} INVITE\r\n\
Contact: <sip:{local_ip}:{local_port}>\r\n\
Content-Type: application/sdp\r\n\
Content-Length: {len}\r\n",
                dest_uri = format_sip_uri(&config.destination),
                local_ip = local_ip,
                local_port = local_sip_addr.port(),
                branch = branch,
                caller = format_sip_uri(&config.caller_id),
                from_tag = from_tag,
                call_id = call_id,
                cseq = cseq,
                len = sdp.len()
            );

            if let Some(ref auth) = auth_header {
                invite_req.push_str(auth);
                invite_req.push_str("\r\n");
            }
            invite_req.push_str("\r\n");
            invite_req.push_str(&sdp);

            sip_socket.send_to(invite_req.as_bytes(), target_addr)?;
            println!("[SIP] Sent INVITE (Attempt #{}, CSeq: {})", attempt + 1, cseq);

            // Wait for response
            let mut buf = [0u8; 4096];
            let mut got_next_step = false;

            while Instant::now() < invite_deadline && running.load(Ordering::Relaxed) {
                match sip_socket.recv_from(&mut buf) {
                    Ok((n, _src)) => {
                        let resp_str = String::from_utf8_lossy(&buf[..n]);
                        if let Some(msg) = SipMessage::parse(&resp_str) {
                            if let Some(status) = msg.status_code() {
                                println!("[SIP] Received: {} ({})", status, msg.raw.lines().next().unwrap_or(""));

                                match status {
                                    100 => {
                                        // Trying, continue waiting
                                    }
                                    180 | 183 => {
                                        println!("[SIP] Call is Ringing / In Progress...");
                                    }
                                    200 => {
                                        // 200 OK! Call accepted
                                        if let Some((remote_ip, remote_port)) = msg.extract_remote_sdp() {
                                            remote_rtp_addr = Some(SocketAddr::new(remote_ip, remote_port));
                                        }

                                        // Extract To tag for dialog
                                        if let Some(to_val) = msg.get("to") {
                                            if let Some(tag_idx) = to_val.find("tag=") {
                                                to_tag = Some(to_val[tag_idx + 4..].split(';').next().unwrap().to_string());
                                            }
                                        }

                                        call_established = true;
                                        got_next_step = true;
                                        break;
                                    }
                                    401 | 407 => {
                                        // Digest Authentication Challenge
                                        let auth_key = if status == 401 { "www-authenticate" } else { "proxy-authenticate" };
                                        if let Some(auth_val) = msg.get(auth_key) {
                                            let (realm, nonce) = parse_auth_header(auth_val);
                                            let user = config.auth_user.as_deref().unwrap_or(&config.caller_id);
                                            let pass = config.password.as_deref().unwrap_or("");

                                            if let (Some(r), Some(n)) = (realm, nonce) {
                                                println!("[SIP] Challenged with Digest Auth for realm \"{r}\"");
                                                let response = compute_digest_response(
                                                    user,
                                                    pass,
                                                    &r,
                                                    &n,
                                                    "INVITE",
                                                    &format_sip_uri(&config.destination),
                                                );

                                                let header_name = if status == 401 { "Authorization" } else { "Proxy-Authorization" };
                                                auth_header = Some(format!(
                                                    "{header_name}: Digest username=\"{user}\", realm=\"{r}\", nonce=\"{n}\", uri=\"{uri}\", response=\"{response}\", algorithm=MD5",
                                                    uri = format_sip_uri(&config.destination)
                                                ));

                                                cseq += 1;
                                                got_next_step = true;
                                                break;
                                            }
                                        }
                                        bail!("Authentication failed or credentials missing");
                                    }
                                    400..=699 => {
                                        bail!("Call failed with status: {status}");
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => {
                        continue;
                    }
                    Err(e) => return Err(e.into()),
                }
            }

            if call_established || !got_next_step {
                break;
            }
        }

        if !call_established {
            bail!("Call could not be established within timeout.");
        }

        let remote_rtp = remote_rtp_addr.ok_or_else(|| anyhow::anyhow!("Remote endpoint did not provide valid RTP SDP"))?;
        println!("[SIP] *** CALL ESTABLISHED *** (Remote RTP Audio: {})", remote_rtp);

        // Send ACK
        let ack_branch = random_id("z9hG4bK");
        let to_header = if let Some(tag) = &to_tag {
            format!("<{}>;tag={tag}", format_sip_uri(&config.destination))
        } else {
            format!("<{}>", format_sip_uri(&config.destination))
        };

        let ack_msg = format!(
            "ACK {dest_uri} SIP/2.0\r\n\
Via: SIP/2.0/UDP {local_ip}:{local_port};branch={ack_branch}\r\n\
Max-Forwards: 70\r\n\
From: <{caller}>;tag={from_tag}\r\n\
To: {to_header}\r\n\
Call-ID: {call_id}\r\n\
CSeq: {cseq} ACK\r\n\
Content-Length: 0\r\n\r\n",
            dest_uri = format_sip_uri(&config.destination),
            local_ip = local_ip,
            local_port = local_sip_addr.port(),
            ack_branch = ack_branch,
            caller = format_sip_uri(&config.caller_id),
            from_tag = from_tag,
            to_header = to_header,
            call_id = call_id,
            cseq = cseq
        );
        sip_socket.send_to(ack_msg.as_bytes(), target_addr)?;
        println!("[SIP] Sent ACK. Streaming audio now...");

        // Start RTP Audio Streaming
        let rtp_running = Arc::new(AtomicBool::new(true));
        let rtp_handle = RtpStreamer::spawn_stream(
            rtp_socket,
            remote_rtp,
            audio_source,
            Arc::clone(&rtp_running),
            0, // PCMU
        );

        // Call active loop: wait for BYE or Ctrl+C or audio completion
        let mut buf = [0u8; 2048];
        while running.load(Ordering::Relaxed) {
            match sip_socket.recv_from(&mut buf) {
                Ok((n, _)) => {
                    let msg_str = String::from_utf8_lossy(&buf[..n]);
                    if let Some(msg) = SipMessage::parse(&msg_str) {
                        if let SipType::Request { method, .. } = &msg.msg_type {
                            if method == "BYE" {
                                println!("[SIP] Remote party hung up (BYE). Sending 200 OK.");
                                let bye_ok = format!(
                                    "SIP/2.0 200 OK\r\n\
Via: {via}\r\n\
From: {from}\r\n\
To: {to}\r\n\
Call-ID: {call_id}\r\n\
CSeq: {cseq}\r\n\
Content-Length: 0\r\n\r\n",
                                    via = msg.get("via").unwrap_or(""),
                                    from = msg.get("from").unwrap_or(""),
                                    to = msg.get("to").unwrap_or(""),
                                    call_id = call_id,
                                    cseq = msg.get("cseq").unwrap_or("")
                                );
                                let _ = sip_socket.send_to(bye_ok.as_bytes(), target_addr);
                                break;
                            }
                        }
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => {
                    // Check if audio finished and auto_hangup is requested
                    if config.auto_hangup && rtp_handle.is_finished() {
                        println!("[Audio] WAV audio playback completed. Automatically hanging up (--hangup)...");
                        break;
                    }
                    continue;
                }
                Err(e) => {
                    log::error!("Socket read error: {e}");
                    break;
                }
            }
        }

        // Stop RTP and Send BYE
        rtp_running.store(false, Ordering::Relaxed);
        let _ = rtp_handle.join();

        println!("[SIP] Terminating call. Sending BYE...");
        cseq += 1;
        let bye_branch = random_id("z9hG4bK");
        let bye_msg = format!(
            "BYE {dest_uri} SIP/2.0\r\n\
Via: SIP/2.0/UDP {local_ip}:{local_port};branch={bye_branch}\r\n\
Max-Forwards: 70\r\n\
From: <{caller}>;tag={from_tag}\r\n\
To: {to_header}\r\n\
Call-ID: {call_id}\r\n\
CSeq: {cseq} BYE\r\n\
Content-Length: 0\r\n\r\n",
            dest_uri = format_sip_uri(&config.destination),
            local_ip = local_ip,
            local_port = local_sip_addr.port(),
            bye_branch = bye_branch,
            caller = format_sip_uri(&config.caller_id),
            from_tag = from_tag,
            to_header = to_header,
            call_id = call_id,
            cseq = cseq
        );
        let _ = sip_socket.send_to(bye_msg.as_bytes(), target_addr);

        println!("[SIP] Call closed successfully.");
        Ok(())
    }

    /// Inbound Listener: listens for incoming INVITE, answers with 200 OK, and plays audio
    pub fn listen(
        bind_addr: &str,
        play_wav: Option<PathBuf>,
        loop_audio: bool,
        running: Arc<AtomicBool>,
    ) -> Result<()> {
        let sip_socket = UdpSocket::bind(bind_addr)
            .with_context(|| format!("Failed to bind SIP listener on {bind_addr}"))?;
        sip_socket.set_read_timeout(Some(Duration::from_millis(500)))?;

        let local_ip = detect_local_ip(&sip_socket);
        println!("[SIP] Listening for incoming calls on {bind_addr} (Local IP: {local_ip})...");

        let mut buf = [0u8; 4096];

        while running.load(Ordering::Relaxed) {
            match sip_socket.recv_from(&mut buf) {
                Ok((n, remote_addr)) => {
                    let req_str = String::from_utf8_lossy(&buf[..n]);
                    if let Some(msg) = SipMessage::parse(&req_str) {
                        if let SipType::Request { method, .. } = &msg.msg_type {
                            if method == "INVITE" {
                                println!("[SIP] Incoming call from {}!", remote_addr);

                                let rtp_socket = Arc::new(UdpSocket::bind("0.0.0.0:0")?);
                                let local_rtp_port = rtp_socket.local_addr()?.port();

                                let sdp = build_sdp(&local_ip.to_string(), local_rtp_port);
                                let local_tag = random_id("srvtag");

                                let call_id = msg.get("call-id").unwrap_or("");
                                let cseq = msg.get("cseq").unwrap_or("");
                                let via = msg.get("via").unwrap_or("");
                                let from = msg.get("from").unwrap_or("");
                                let to = msg.get("to").unwrap_or("");

                                let ok_resp = format!(
                                    "SIP/2.0 200 OK\r\n\
Via: {via}\r\n\
From: {from}\r\n\
To: {to};tag={local_tag}\r\n\
Call-ID: {call_id}\r\n\
CSeq: {cseq}\r\n\
Contact: <sip:{local_ip}:{local_port}>\r\n\
Content-Type: application/sdp\r\n\
Content-Length: {len}\r\n\r\n{sdp}",
                                    via = via,
                                    from = from,
                                    to = to,
                                    local_tag = local_tag,
                                    call_id = call_id,
                                    cseq = cseq,
                                    local_ip = local_ip,
                                    local_port = sip_socket.local_addr()?.port(),
                                    len = sdp.len(),
                                    sdp = sdp
                                );

                                sip_socket.send_to(ok_resp.as_bytes(), remote_addr)?;
                                println!("[SIP] Answered with 200 OK. Streaming audio...");

                                if let Some((rem_ip, rem_port)) = msg.extract_remote_sdp() {
                                    let rem_rtp = SocketAddr::new(rem_ip, rem_port);
                                    let audio = if let Some(ref p) = play_wav {
                                        AudioSource::from_wav_file(p, false, loop_audio)?
                                    } else {
                                        AudioSource::test_tone(false)
                                    };

                                    let rtp_running = Arc::new(AtomicBool::new(true));
                                    let _ = RtpStreamer::spawn_stream(
                                        rtp_socket,
                                        rem_rtp,
                                        audio,
                                        rtp_running,
                                        0,
                                    );
                                }
                            }
                        }
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => {
                    continue;
                }
                Err(e) => {
                    log::error!("Listen socket error: {e}");
                    break;
                }
            }
        }

        Ok(())
    }
}

fn format_sip_uri(s: &str) -> String {
    if s.starts_with("sip:") || s.starts_with("sips:") {
        s.to_string()
    } else {
        format!("sip:{s}")
    }
}

fn resolve_target_addr(dest: &str, proxy: Option<&str>) -> Result<SocketAddr> {
    if let Some(p) = proxy {
        return p
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| anyhow::anyhow!("Could not resolve proxy {p}"));
    }

    let clean = dest.trim_start_matches("sip:").trim_start_matches("sips:");
    let host_part = if let Some((_, host)) = clean.split_once('@') {
        host
    } else {
        clean
    };

    let host_port = if host_part.contains(':') {
        host_part.to_string()
    } else {
        format!("{host_part}:5060")
    };

    host_port
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| anyhow::anyhow!("Could not resolve destination {dest}"))
}

fn detect_local_ip(socket: &UdpSocket) -> IpAddr {
    // Try to determine outbound local IP address
    if let Ok(()) = socket.connect("8.8.8.8:80") {
        if let Ok(addr) = socket.local_addr() {
            return addr.ip();
        }
    }
    IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))
}
