pub const STDIO_TEMPLATE: &str = r###"// -----------------------------------------------------------------------------
// Thread-Safe Stdout Frame Writer & Synchronous Stdio Transport Loop
// -----------------------------------------------------------------------------
#[derive(Clone)]
pub struct StdoutWriter {
    inner: Arc<Mutex<io::BufWriter<io::Stdout>>>,
}

impl StdoutWriter {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(io::BufWriter::new(io::stdout()))),
        }
    }

    pub fn write_response(&self, resp: &JsonRpcResponse) -> io::Result<()> {
        let mut lock = self.inner.lock().unwrap();
        serde_json::to_writer(&mut *lock, resp)?;
        lock.write_all(b"\n")?;
        lock.flush()
    }
}

impl McpServer {
    pub fn run_stdio(&mut self) -> io::Result<()> {
        let stdin = io::stdin();
        let mut reader = stdin.lock();
        let mut line_buf = String::new();

        mcp_log!("Stdio JSON-RPC 2.0 loop started");

        while reader.read_line(&mut line_buf)? > 0 {
            let trimmed = line_buf.trim_end_matches(&['\r', '\n'][..]);
            if !trimmed.is_empty() {
                match serde_json::from_str::<JsonRpcRequest>(trimmed) {
                    Ok(request) => {
                        if let Some(response) = self.dispatch(request) {
                            if let Err(e) = self.writer.write_response(&response) {
                                mcp_log!("Failed to write response: {}", e);
                            }
                        }
                    }
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
                        let _ = self.writer.write_response(&err_resp);
                    }
                }
            }
            line_buf.clear();
        }

        mcp_log!("Stdio stream reached EOF. Server exiting cleanly.");
        Ok(())
    }
}
"###;
