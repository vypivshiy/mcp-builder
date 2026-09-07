pub const WS_PIPE_TEMPLATE: &str = r###"// -----------------------------------------------------------------------------
// Base64 Simple Encoder Helper
// -----------------------------------------------------------------------------
const BASE64_ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64_encode(data: &[u8]) -> String {
    let mut encoded = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0];
        let b1 = if chunk.len() > 1 { chunk[1] } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] } else { 0 };

        encoded.push(BASE64_ALPHABET[(b0 >> 2) as usize] as char);
        encoded.push(BASE64_ALPHABET[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);

        if chunk.len() > 1 {
            encoded.push(BASE64_ALPHABET[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            encoded.push('=');
        }

        if chunk.len() > 2 {
            encoded.push(BASE64_ALPHABET[(b2 & 0x3f) as usize] as char);
        } else {
            encoded.push('=');
        }
    }
    encoded
}

// -----------------------------------------------------------------------------
// Synchronous RFC 6455 WebSocket Client
// -----------------------------------------------------------------------------
pub struct WsOptions<'a> {
    pub url: Option<&'a str>,
    pub host: Option<&'a str>,
    pub port: Option<&'a str>,
    pub endpoint: Option<&'a str>,
    pub headers: &'a [(String, String)],
    pub message: &'a str,
    pub timeout: Duration,
    pub extract_json_field: Option<&'a str>,
}

pub fn execute_ws_request(opts: WsOptions) -> Result<String, String> {
    let (host, port, path) = if let Some(raw_url) = opts.url {
        let stripped = raw_url.trim_start_matches("ws://").trim_start_matches("http://");
        let (host_port, path_part) = match stripped.find('/') {
            Some(idx) => (&stripped[..idx], &stripped[idx..]),
            None => (stripped, "/"),
        };
        let (h, p) = match host_port.find(':') {
            Some(colon) => (&host_port[..colon], host_port[colon + 1..].parse::<u16>().unwrap_or(80)),
            None => (host_port, 80),
        };
        (h.to_string(), p, path_part.to_string())
    } else {
        let h = opts.host.unwrap_or("127.0.0.1").to_string();
        let p = opts.port.and_then(|s| s.parse::<u16>().ok()).unwrap_or(80);
        let path = opts.endpoint.unwrap_or("/").to_string();
        (h, p, path)
    };

    mcp_log!("[WS] Connecting to {}:{} path '{}'", host, port, path);
    let addr_str = format!("{}:{}", host, port);
    let socket_addrs: Vec<std::net::SocketAddr> = addr_str
        .to_socket_addrs()
        .map_err(|e| format!("Failed to resolve WebSocket host '{}': {}", addr_str, e))?
        .collect();

    if socket_addrs.is_empty() {
        return Err(format!("Could not resolve host address for '{}'", addr_str));
    }

    let mut stream = std::net::TcpStream::connect_timeout(&socket_addrs[0], opts.timeout)
        .map_err(|e| format!("Failed to connect to WebSocket at '{}': {}", addr_str, e))?;

    stream.set_read_timeout(Some(opts.timeout)).ok();
    stream.set_write_timeout(Some(opts.timeout)).ok();

    // Generate Sec-WebSocket-Key (16 bytes base64)
    let nonce_seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let nonce_bytes = nonce_seed.to_le_bytes();
    let sec_key = base64_encode(&nonce_bytes);

    // Send HTTP/1.1 101 Handshake request
    let mut handshake = format!(
        "GET {} HTTP/1.1\r\nHost: {}:{}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {}\r\nSec-WebSocket-Version: 13\r\n",
        path, host, port, sec_key
    );
    for (k, v) in opts.headers {
        handshake.push_str(&format!("{}: {}\r\n", k, v));
    }
    handshake.push_str("\r\n");

    stream.write_all(handshake.as_bytes())
        .map_err(|e| format!("Failed to send WebSocket handshake: {}", e))?;
    stream.flush().map_err(|e| format!("Failed to flush WebSocket handshake: {}", e))?;

    // Read Handshake response (until \r\n\r\n)
    let mut header_buf = Vec::new();
    let mut byte_buf = [0u8; 1];
    let mut matched_end = 0;
    while header_buf.len() < 8192 {
        let n = stream.read(&mut byte_buf)
            .map_err(|e| format!("Failed to read WebSocket handshake response: {}", e))?;
        if n == 0 {
            return Err("Unexpected EOF during WebSocket handshake".to_string());
        }
        let b = byte_buf[0];
        header_buf.push(b);
        if (matched_end == 0 || matched_end == 2) && b == b'\r' {
            matched_end += 1;
        } else if (matched_end == 1 || matched_end == 3) && b == b'\n' {
            matched_end += 1;
            if matched_end == 4 {
                break;
            }
        } else {
            matched_end = if b == b'\r' { 1 } else { 0 };
        }
    }

    let header_str = String::from_utf8_lossy(&header_buf);
    if !header_str.contains("101") {
        return Err(format!("WebSocket server rejected handshake:\n{}", header_str));
    }

    // Send Masked WebSocket Text Frame (RFC 6455)
    let payload = opts.message.as_bytes();
    let payload_len = payload.len();
    let mut frame_header = Vec::new();
    frame_header.push(0x81); // FIN=1, Opcode=1 (Text)

    let mask_key = [0x37u8, 0xfau8, 0x12u8, 0x9cu8];
    if payload_len < 126 {
        frame_header.push(0x80 | (payload_len as u8));
    } else if payload_len <= 65535 {
        frame_header.push(0x80 | 126);
        frame_header.extend_from_slice(&(payload_len as u16).to_be_bytes());
    } else {
        frame_header.push(0x80 | 127);
        frame_header.extend_from_slice(&(payload_len as u64).to_be_bytes());
    }
    frame_header.extend_from_slice(&mask_key);

    let mut masked_payload = Vec::with_capacity(payload_len);
    for (i, &b) in payload.iter().enumerate() {
        masked_payload.push(b ^ mask_key[i % 4]);
    }

    stream.write_all(&frame_header)
        .map_err(|e| format!("Failed to write WebSocket frame header: {}", e))?;
    stream.write_all(&masked_payload)
        .map_err(|e| format!("Failed to write WebSocket frame payload: {}", e))?;
    stream.flush()
        .map_err(|e| format!("Failed to flush WebSocket frame: {}", e))?;

    // Read response frame(s)
    let mut response_text = String::new();
    loop {
        let mut hdr = [0u8; 2];
        stream.read_exact(&mut hdr)
            .map_err(|e| format!("Failed to read WebSocket response header: {}", e))?;

        let fin = (hdr[0] & 0x80) != 0;
        let opcode = hdr[0] & 0x0F;
        let is_masked = (hdr[1] & 0x80) != 0;
        let len_ind = hdr[1] & 0x7F;

        let frame_len: usize = if len_ind < 126 {
            len_ind as usize
        } else if len_ind == 126 {
            let mut ext = [0u8; 2];
            stream.read_exact(&mut ext)
                .map_err(|e| format!("Failed to read 16-bit extended length: {}", e))?;
            u16::from_be_bytes(ext) as usize
        } else {
            let mut ext = [0u8; 8];
            stream.read_exact(&mut ext)
                .map_err(|e| format!("Failed to read 64-bit extended length: {}", e))?;
            u64::from_be_bytes(ext) as usize
        };

        let mask_bytes = if is_masked {
            let mut m = [0u8; 4];
            stream.read_exact(&mut m)
                .map_err(|e| format!("Failed to read masking key: {}", e))?;
            Some(m)
        } else {
            None
        };

        let mut body = vec![0u8; frame_len];
        if frame_len > 0 {
            stream.read_exact(&mut body)
                .map_err(|e| format!("Failed to read WebSocket frame body: {}", e))?;
        }

        if let Some(mask) = mask_bytes {
            for (i, b) in body.iter_mut().enumerate() {
                *b ^= mask[i % 4];
            }
        }

        match opcode {
            0x1 | 0x0 => { // Text or Continuation
                let chunk = String::from_utf8_lossy(&body);
                response_text.push_str(&chunk);
                if fin {
                    break;
                }
            }
            0x8 => { // Close
                return Err("WebSocket connection closed by remote peer".to_string());
            }
            0x9 => { // Ping -> reply with Pong (opcode 0xA)
                let mut pong_hdr = vec![0x8A];
                pong_hdr.push(0x80 | (body.len() as u8));
                pong_hdr.extend_from_slice(&mask_key);
                for (i, &b) in body.iter().enumerate() {
                    pong_hdr.push(b ^ mask_key[i % 4]);
                }
                let _ = stream.write_all(&pong_hdr);
                let _ = stream.flush();
            }
            _ => {
                if fin {
                    break;
                }
            }
        }
    }

    if let Some(field) = opts.extract_json_field {
        if let Ok(json_val) = serde_json::from_str::<Value>(&response_text) {
            if let Some(extracted) = json_val.pointer(field) {
                return Ok(match extracted {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                });
            }
        }
    }

    Ok(response_text)
}

// -----------------------------------------------------------------------------
// Synchronous Raw TCP Stream / Pipe Client
// -----------------------------------------------------------------------------
pub struct PipeOptions<'a> {
    pub host: Option<&'a str>,
    pub port: Option<&'a str>,
    pub addr: Option<&'a str>,
    pub message: &'a str,
    pub framing: &'a str,
    pub timeout: Duration,
    pub extract_json_field: Option<&'a str>,
}

pub fn execute_pipe_request(opts: PipeOptions) -> Result<String, String> {
    let addr_str = if let Some(a) = opts.addr {
        a.to_string()
    } else {
        let h = opts.host.unwrap_or("127.0.0.1");
        let p = opts.port.unwrap_or("8080");
        format!("{}:{}", h, p)
    };

    mcp_log!("[PIPE] Connecting to '{}'", addr_str);
    let socket_addrs: Vec<std::net::SocketAddr> = addr_str
        .to_socket_addrs()
        .map_err(|e| format!("Failed to resolve TCP address '{}': {}", addr_str, e))?
        .collect();

    if socket_addrs.is_empty() {
        return Err(format!("Could not resolve TCP address '{}'", addr_str));
    }

    let mut stream = std::net::TcpStream::connect_timeout(&socket_addrs[0], opts.timeout)
        .map_err(|e| format!("Failed to connect to TCP server at '{}': {}", addr_str, e))?;

    stream.set_read_timeout(Some(opts.timeout)).ok();
    stream.set_write_timeout(Some(opts.timeout)).ok();

    if !opts.message.is_empty() {
        stream.write_all(opts.message.as_bytes())
            .map_err(|e| format!("Failed to send TCP message: {}", e))?;
        stream.flush().map_err(|e| format!("Failed to flush TCP stream: {}", e))?;
    }

    let framing_mode = opts.framing.trim();
    let mut response_bytes = Vec::new();

    if framing_mode == "eof" || framing_mode == "close" {
        stream.shutdown(std::net::Shutdown::Write).ok();
        stream.read_to_end(&mut response_bytes)
            .map_err(|e| format!("Failed to read TCP response until EOF: {}", e))?;
    } else if framing_mode.starts_with("bytes:") {
        let count: usize = framing_mode["bytes:".len()..].trim().parse().unwrap_or(1024);
        response_bytes.resize(count, 0);
        stream.read_exact(&mut response_bytes)
            .map_err(|e| format!("Failed to read {} exact bytes from TCP stream: {}", count, e))?;
    } else if framing_mode.starts_with("lines:") {
        let count: usize = framing_mode["lines:".len()..].trim().parse().unwrap_or(1);
        let mut reader = std::io::BufReader::new(stream);
        let mut res_str = String::new();
        for _ in 0..count {
            let mut line = String::new();
            if reader.read_line(&mut line).map_err(|e| format!("Failed to read line: {}", e))? == 0 {
                break;
            }
            res_str.push_str(&line);
        }
        response_bytes = res_str.into_bytes();
    } else if framing_mode == "json" {
        let mut brace_depth = 0i32;
        let mut bracket_depth = 0i32;
        let mut in_string = false;
        let mut escape = false;
        let mut started = false;
        let mut b = [0u8; 1];

        while stream.read(&mut b).map_err(|e| format!("Failed to read JSON stream: {}", e))? > 0 {
            let ch = b[0] as char;
            response_bytes.push(b[0]);

            if !started {
                if ch == '{' {
                    started = true;
                    brace_depth = 1;
                } else if ch == '[' {
                    started = true;
                    bracket_depth = 1;
                }
                continue;
            }

            if escape {
                escape = false;
                continue;
            }

            if ch == '\\' {
                escape = true;
                continue;
            }

            if ch == '"' {
                in_string = !in_string;
                continue;
            }

            if !in_string {
                if ch == '{' {
                    brace_depth += 1;
                } else if ch == '}' {
                    brace_depth -= 1;
                } else if ch == '[' {
                    bracket_depth += 1;
                } else if ch == ']' {
                    bracket_depth -= 1;
                }

                if brace_depth == 0 && bracket_depth == 0 {
                    break;
                }
            }
        }
    } else {
        let delim = if framing_mode.starts_with("delimiter:") {
            &framing_mode["delimiter:".len()..]
        } else if framing_mode.is_empty() {
            "\n"
        } else {
            framing_mode
        };
        let delim_bytes = delim.as_bytes();
        let mut b = [0u8; 1];

        while stream.read(&mut b).map_err(|e| format!("Failed to read from TCP stream: {}", e))? > 0 {
            response_bytes.push(b[0]);
            if response_bytes.ends_with(delim_bytes) {
                break;
            }
        }
    }

    let response_str = String::from_utf8_lossy(&response_bytes).to_string();

    if let Some(field) = opts.extract_json_field {
        if let Ok(json_val) = serde_json::from_str::<Value>(&response_str) {
            if let Some(extracted) = json_val.pointer(field) {
                return Ok(match extracted {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                });
            }
        }
    }

    Ok(response_str)
}
"###;
