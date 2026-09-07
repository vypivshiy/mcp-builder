pub const SSE_TEMPLATE: &str = r###"// -----------------------------------------------------------------------------
// Minimalist HTTP / SSE Transport Engine (via httparse & std::net)
// -----------------------------------------------------------------------------
static SESSION_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn generate_session_id() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let cnt = SESSION_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("{:x}-{:x}", now, cnt)
}

fn extract_query_param(query: &str, param_name: &str) -> Option<String> {
    for pair in query.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            if k == param_name {
                return Some(v.to_string());
            }
        }
    }
    None
}

fn send_cors_preflight(stream: &mut TcpStream) -> io::Result<()> {
    let resp = "HTTP/1.1 204 No Content\r\n\
Access-Control-Allow-Origin: *\r\n\
Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n\
Access-Control-Allow-Headers: Content-Type, Authorization, MCP-Protocol-Version, Mcp-Method, Mcp-Name\r\n\
Access-Control-Max-Age: 86400\r\n\
Connection: close\r\n\
\r\n";
    stream.write_all(resp.as_bytes())?;
    stream.flush()
}

fn send_http_response(stream: &mut TcpStream, status_code: u16, status_text: &str, body: &str) -> io::Result<()> {
    let body_bytes = body.as_bytes();
    let resp = format!(
        "HTTP/1.1 {} {}\r\n\
Content-Type: text/plain; charset=utf-8\r\n\
Content-Length: {}\r\n\
Access-Control-Allow-Origin: *\r\n\
Connection: close\r\n\
\r\n\
{}",
        status_code,
        status_text,
        body_bytes.len(),
        body
    );
    stream.write_all(resp.as_bytes())?;
    stream.flush()
}

fn send_json_response(stream: &mut TcpStream, status_code: u16, status_text: &str, json_body: &str) -> io::Result<()> {
    let body_bytes = json_body.as_bytes();
    let resp = format!(
        "HTTP/1.1 {} {}\r\n\
Content-Type: application/json; charset=utf-8\r\n\
Content-Length: {}\r\n\
Access-Control-Allow-Origin: *\r\n\
Connection: close\r\n\
\r\n\
{}",
        status_code,
        status_text,
        body_bytes.len(),
        json_body
    );
    stream.write_all(resp.as_bytes())?;
    stream.flush()
}

fn handle_sse_stream(
    mut stream: TcpStream,
    sessions: Arc<Mutex<HashMap<String, mpsc::Sender<String>>>>,
) -> io::Result<()> {
    let session_id = generate_session_id();
    let (tx, rx) = mpsc::channel();

    {
        let mut map = sessions.lock().unwrap();
        map.insert(session_id.clone(), tx);
    }

    mcp_log!("SSE client connected: session_id={}", session_id);

    let _ = stream.set_read_timeout(None);

    let sse_headers = format!(
        "HTTP/1.1 200 OK\r\n\
Content-Type: text/event-stream; charset=utf-8\r\n\
Cache-Control: no-cache, no-transform\r\n\
Connection: keep-alive\r\n\
X-Accel-Buffering: no\r\n\
Access-Control-Allow-Origin: *\r\n\
\r\n\
event: endpoint\r\ndata: /message?sessionId={}\r\n\r\n",
        session_id
    );

    if let Err(e) = stream.write_all(sse_headers.as_bytes()) {
        let mut map = sessions.lock().unwrap();
        map.remove(&session_id);
        return Err(e);
    }
    if let Err(e) = stream.flush() {
        let mut map = sessions.lock().unwrap();
        map.remove(&session_id);
        return Err(e);
    }

    loop {
        match rx.recv_timeout(Duration::from_secs(15)) {
            Ok(msg) => {
                let event_payload = format!("event: message\r\ndata: {}\r\n\r\n", msg);
                if stream.write_all(event_payload.as_bytes()).is_err() || stream.flush().is_err() {
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let keepalive = ": keepalive\r\n\r\n";
                if stream.write_all(keepalive.as_bytes()).is_err() || stream.flush().is_err() {
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                break;
            }
        }
    }

    mcp_log!("SSE client disconnected: session_id={}", session_id);
    let mut map = sessions.lock().unwrap();
    map.remove(&session_id);
    Ok(())
}

fn handle_post_message(
    mut stream: TcpStream,
    sessions: Arc<Mutex<HashMap<String, mpsc::Sender<String>>>>,
    session_id: Option<&str>,
    body: &str,
) -> io::Result<()> {
    if body.trim().is_empty() {
        return send_http_response(&mut stream, 400, "Bad Request", "Empty request body");
    }

    let request: JsonRpcRequest = match serde_json::from_str(body) {
        Ok(r) => r,
        Err(e) => {
            let err_resp = JsonRpcResponse {
                jsonrpc: "2.0",
                id: None,
                result: None,
                error: Some(JsonRpcError {
                    code: PARSE_ERROR,
                    message: format!("Parse error: {}", e),
                    data: None,
                }),
            };
            let json_str = serde_json::to_string(&err_resp).unwrap_or_default();
            return send_json_response(&mut stream, 200, "OK", &json_str);
        }
    };

    let mut server = McpServer::new(StdoutWriter::new());
    let response_opt = server.dispatch(request);

    if let Some(sid) = session_id {
        let maybe_tx = {
            let map = sessions.lock().unwrap();
            map.get(sid).cloned()
        };

        if let Some(tx) = maybe_tx {
            if let Some(resp) = response_opt {
                let json_str = serde_json::to_string(&resp).unwrap_or_default();
                let _ = tx.send(json_str);
            }
            let resp = "HTTP/1.1 202 Accepted\r\n\
Content-Type: text/plain; charset=utf-8\r\n\
Access-Control-Allow-Origin: *\r\n\
Connection: close\r\n\
\r\n\
Accepted";
            stream.write_all(resp.as_bytes())?;
            return stream.flush();
        } else {
            return send_http_response(&mut stream, 404, "Not Found", "Session ID not found or expired");
        }
    }

    if let Some(resp) = response_opt {
        let json_str = serde_json::to_string(&resp).unwrap_or_default();
        send_json_response(&mut stream, 200, "OK", &json_str)
    } else {
        let resp = "HTTP/1.1 204 No Content\r\n\
Access-Control-Allow-Origin: *\r\n\
Connection: close\r\n\
\r\n";
        stream.write_all(resp.as_bytes())?;
        stream.flush()
    }
}

fn handle_http_connection(
    mut stream: TcpStream,
    sessions: Arc<Mutex<HashMap<String, mpsc::Sender<String>>>>,
) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    let mut buffer = Vec::with_capacity(4096);
    let mut chunk = [0u8; 2048];
    let mut body_offset = 0;
    let mut content_length = 0;
    let mut method = String::new();
    let mut path = String::new();

    loop {
        let n = match stream.read(&mut chunk) {
            Ok(0) => return Ok(()),
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::WouldBlock || e.kind() == io::ErrorKind::TimedOut => return Ok(()),
            Err(e) => return Err(e),
        };
        buffer.extend_from_slice(&chunk[..n]);

        let mut headers = [httparse::EMPTY_HEADER; 64];
        let mut req = httparse::Request::new(&mut headers);

        match req.parse(&buffer) {
            Ok(httparse::Status::Complete(offset)) => {
                body_offset = offset;
                if let Some(m) = req.method {
                    method = m.to_uppercase();
                }
                if let Some(p) = req.path {
                    path = p.to_string();
                }
                for h in req.headers.iter() {
                    if h.name.eq_ignore_ascii_case("content-length") {
                        if let Ok(len_str) = std::str::from_utf8(h.value) {
                            content_length = len_str.trim().parse::<usize>().unwrap_or(0);
                        }
                    }
                }
                break;
            }
            Ok(httparse::Status::Partial) => {
                if buffer.len() > 65536 {
                    return send_http_response(&mut stream, 413, "Payload Too Large", "Request header too large");
                }
                continue;
            }
            Err(e) => {
                return send_http_response(&mut stream, 400, "Bad Request", &format!("HTTP parse error: {}", e));
            }
        }
    }

    while buffer.len() < body_offset + content_length {
        let n = match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::WouldBlock || e.kind() == io::ErrorKind::TimedOut => break,
            Err(e) => return Err(e),
        };
        buffer.extend_from_slice(&chunk[..n]);
    }

    let body_bytes = if buffer.len() >= body_offset {
        &buffer[body_offset..std::cmp::min(buffer.len(), body_offset + content_length)]
    } else {
        &[]
    };

    if method == "OPTIONS" {
        return send_cors_preflight(&mut stream);
    }

    let parsed_url_path = path.split('?').next().unwrap_or("/");
    let query_string = path.split_once('?').map(|x| x.1).unwrap_or("");

    if method == "GET" && (parsed_url_path == "/sse" || parsed_url_path == "/") {
        handle_sse_stream(stream, sessions)
    } else if method == "POST" && (parsed_url_path == "/message" || parsed_url_path == "/") {
        let session_id = extract_query_param(query_string, "sessionId");
        let body_str = String::from_utf8_lossy(body_bytes);
        handle_post_message(stream, sessions, session_id.as_deref(), &body_str)
    } else {
        send_http_response(&mut stream, 404, "Not Found", "Endpoint not found")
    }
}

pub struct SseServer {
    pub host: String,
    pub port: u16,
    pub sessions: Arc<Mutex<HashMap<String, mpsc::Sender<String>>>>,
}

impl SseServer {
    pub fn new(host: &str, port: u16) -> Self {
        Self {
            host: host.to_string(),
            port,
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn run(&self) -> io::Result<()> {
        let addr = format!("{}:{}", self.host, self.port);
        let listener = TcpListener::bind(&addr)?;
        mcp_log!("SSE/HTTP Server listening on http://{}", addr);

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let sessions = Arc::clone(&self.sessions);
                    thread::spawn(move || {
                        if let Err(e) = handle_http_connection(stream, sessions) {
                            mcp_debug!("HTTP connection error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    mcp_log!("Failed to accept incoming connection: {}", e);
                }
            }
        }
        Ok(())
    }
}
"###;
