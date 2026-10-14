use std::collections::HashMap;
use std::net::IpAddr;
use md5::{Digest, Md5};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SipType {
    Request { method: String, uri: String },
    Response { code: u16, reason: String },
}

#[derive(Debug, Clone)]
pub struct SipMessage {
    pub msg_type: SipType,
    pub headers: HashMap<String, String>,
    pub body: String,
    pub raw: String,
}

impl SipMessage {
    pub fn parse(s: &str) -> Option<Self> {
        let trimmed = s.trim_start();
        if trimmed.is_empty() {
            return None;
        }

        let (header_part, body) = if let Some(idx) = trimmed.find("\r\n\r\n") {
            (&trimmed[..idx], &trimmed[idx + 4..])
        } else if let Some(idx) = trimmed.find("\n\n") {
            (&trimmed[..idx], &trimmed[idx + 2..])
        } else {
            (trimmed, "")
        };

        let mut lines = header_part.lines();
        let first_line = lines.next()?.trim();

        let msg_type = if first_line.starts_with("SIP/2.0") {
            let mut parts = first_line.splitn(3, ' ');
            let _version = parts.next()?;
            let code: u16 = parts.next()?.parse().ok()?;
            let reason = parts.next().unwrap_or("").to_string();
            SipType::Response { code, reason }
        } else {
            let mut parts = first_line.splitn(3, ' ');
            let method = parts.next()?.to_uppercase();
            let uri = parts.next()?.to_string();
            SipType::Request { method, uri }
        };

        let mut headers = HashMap::new();
        for line in lines {
            if let Some(colon) = line.find(':') {
                let key = line[..colon].trim().to_lowercase();
                let val = line[colon + 1..].trim().to_string();
                headers.insert(key, val);
            }
        }

        Some(Self {
            msg_type,
            headers,
            body: body.to_string(),
            raw: s.to_string(),
        })
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        let binding = name.to_lowercase();
        let key = match binding.as_str() {
            "call-id" | "i" => "call-id",
            "from" | "f" => "from",
            "to" | "t" => "to",
            "via" | "v" => "via",
            "contact" | "m" => "contact",
            "cseq" => "cseq",
            "content-type" | "c" => "content-type",
            "content-length" | "l" => "content-length",
            other => other,
        };
        self.headers.get(key).map(|s| s.as_str())
    }

    pub fn status_code(&self) -> Option<u16> {
        match &self.msg_type {
            SipType::Response { code, .. } => Some(*code),
            _ => None,
        }
    }

    #[allow(dead_code)]
    pub fn is_response(&self) -> bool {
        matches!(self.msg_type, SipType::Response { .. })
    }

    /// Extract remote RTP media IP and audio port from SDP body
    pub fn extract_remote_sdp(&self) -> Option<(IpAddr, u16)> {
        let mut remote_ip: Option<IpAddr> = None;
        let mut audio_port: Option<u16> = None;

        for line in self.body.lines() {
            let line = line.trim();
            if line.starts_with("c=IN IP4 ") {
                let ip_str = line.trim_start_matches("c=IN IP4 ").trim();
                remote_ip = ip_str.parse().ok();
            } else if line.starts_with("m=audio ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    audio_port = parts[1].parse().ok();
                }
            }
        }

        match (remote_ip, audio_port) {
            (Some(ip), Some(port)) if port > 0 => Some((ip, port)),
            _ => None,
        }
    }
}

/// Helper to generate random tags, branch, and call-id
pub fn random_id(prefix: &str) -> String {
    let num: u64 = rand::random();
    format!("{prefix}-{num:x}")
}

/// Builds SDP session payload (G.711u / G.711a)
pub fn build_sdp(local_ip: &str, rtp_port: u16) -> String {
    let sess_id: u32 = rand::random();
    format!(
        "v=0\r\n\
o=- {sess_id} {sess_id} IN IP4 {local_ip}\r\n\
s=sip-client\r\n\
c=IN IP4 {local_ip}\r\n\
t=0 0\r\n\
m=audio {rtp_port} RTP/AVP 0 8 101\r\n\
a=rtpmap:0 PCMU/8000\r\n\
a=rtpmap:8 PCMA/8000\r\n\
a=rtpmap:101 telephone-event/8000\r\n\
a=fmtp:101 0-16\r\n\
a=sendrecv\r\n"
    )
}

/// Computes HTTP Digest response for SIP RFC 2617 / RFC 3261
pub fn compute_digest_response(
    username: &str,
    password: &str,
    realm: &str,
    nonce: &str,
    method: &str,
    uri: &str,
) -> String {
    // HA1 = MD5(username:realm:password)
    let mut h1 = Md5::new();
    h1.update(format!("{username}:{realm}:{password}"));
    let ha1 = format!("{:x}", h1.finalize());

    // HA2 = MD5(method:uri)
    let mut h2 = Md5::new();
    h2.update(format!("{method}:{uri}"));
    let ha2 = format!("{:x}", h2.finalize());

    // response = MD5(HA1:nonce:HA2)
    let mut h3 = Md5::new();
    h3.update(format!("{ha1}:{nonce}:{ha2}"));
    format!("{:x}", h3.finalize())
}

/// Parses WWW-Authenticate or Proxy-Authenticate header attributes
pub fn parse_auth_header(header_val: &str) -> (Option<String>, Option<String>) {
    let mut realm = None;
    let mut nonce = None;

    for part in header_val.split(',') {
        let part = part.trim();
        if let Some((k, v)) = part.split_once('=') {
            let key = k.trim().trim_start_matches("Digest").trim().to_lowercase();
            let val = v.trim().trim_matches('"').to_string();
            if key == "realm" {
                realm = Some(val);
            } else if key == "nonce" {
                nonce = Some(val);
            }
        }
    }

    (realm, nonce)
}
