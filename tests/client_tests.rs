use std::path::PathBuf;

#[path = "../src/audio.rs"]
mod audio;
#[path = "../src/rtp.rs"]
mod rtp;
#[path = "../src/sip.rs"]
mod sip;

use audio::{linear_to_alaw, linear_to_ulaw, AudioSource};
use rtp::RtpPacket;
use sip::{build_sdp, compute_digest_response, parse_auth_header, SipMessage, SipType};

#[test]
fn test_g711_companding() {
    let pcm_zero = 0i16;
    let pcm_pos = 10000i16;
    let pcm_neg = -10000i16;

    let ulaw_zero = linear_to_ulaw(pcm_zero);
    let ulaw_pos = linear_to_ulaw(pcm_pos);
    let ulaw_neg = linear_to_ulaw(pcm_neg);

    assert_eq!(ulaw_zero, 0xFF);
    assert_ne!(ulaw_pos, ulaw_neg);

    let alaw_zero = linear_to_alaw(pcm_zero);
    let alaw_pos = linear_to_alaw(pcm_pos);
    assert_ne!(alaw_zero, alaw_pos);
}

#[test]
fn test_rtp_packet_structure() {
    let payload = vec![0x01, 0x02, 0x03, 0x04];
    let packet = RtpPacket::new(0, 100, 16000, 0x12345678, payload);
    let bytes = packet.to_bytes();

    assert_eq!(bytes.len(), 12 + 4);
    // V=2, P=0, X=0, CC=0 -> 0x80
    assert_eq!(bytes[0], 0x80);
    // PT=0
    assert_eq!(bytes[1], 0x00);
    // Seq = 100 (0x0064)
    assert_eq!(bytes[2], 0x00);
    assert_eq!(bytes[3], 0x64);
    // Payload
    assert_eq!(&bytes[12..], &[0x01, 0x02, 0x03, 0x04]);
}

#[test]
fn test_sip_message_parsing_and_sdp_extraction() {
    let raw = "SIP/2.0 200 OK\r\n\
Via: SIP/2.0/UDP 127.0.0.1:5060;branch=z9hG4bK-1\r\n\
From: <sip:alice@domain.com>;tag=abc\r\n\
To: <sip:bob@domain.com>;tag=xyz\r\n\
Call-ID: call-test-99\r\n\
CSeq: 1 INVITE\r\n\
Content-Type: application/sdp\r\n\
Content-Length: 120\r\n\
\r\n\
v=0\r\n\
o=bob 1234 1234 IN IP4 192.168.1.50\r\n\
s=Call\r\n\
c=IN IP4 192.168.1.50\r\n\
t=0 0\r\n\
m=audio 40000 RTP/AVP 0 8\r\n";

    let msg = SipMessage::parse(raw).expect("Valid SIP message");
    assert_eq!(msg.status_code(), Some(200));
    assert_eq!(msg.get("call-id"), Some("call-test-99"));
    assert_eq!(msg.get("cseq"), Some("1 INVITE"));

    let sdp = msg.extract_remote_sdp().expect("SDP should extract");
    assert_eq!(sdp.0.to_string(), "192.168.1.50");
    assert_eq!(sdp.1, 40000);
}

#[test]
fn test_digest_authentication_calculation() {
    let auth_header = "Digest realm=\"biloxi.com\", nonce=\"dcd98b7102dd2f0e8b11d0f600bfb0c093\"";
    let (realm, nonce) = parse_auth_header(auth_header);
    assert_eq!(realm.as_deref(), Some("biloxi.com"));
    assert_eq!(nonce.as_deref(), Some("dcd98b7102dd2f0e8b11d0f600bfb0c093"));

    let resp = compute_digest_response(
        "bob",
        "secret",
        realm.as_deref().unwrap(),
        nonce.as_deref().unwrap(),
        "INVITE",
        "sip:bob@biloxi.com",
    );
    assert!(!resp.is_empty());
    assert_eq!(resp.len(), 32); // MD5 hex length
}

#[test]
fn test_test_tone_audio_source() {
    let mut audio = AudioSource::test_tone(false);
    let frame1 = audio.next_frame();
    assert!(frame1.is_some());
    assert_eq!(frame1.unwrap().len(), 160);
}
