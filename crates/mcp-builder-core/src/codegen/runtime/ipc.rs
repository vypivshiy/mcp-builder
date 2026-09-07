pub const IPC_TEMPLATE: &str = r###"// -----------------------------------------------------------------------------
// File-Based Mailbox IPC Engine (Atomic File Exchange & Heartbeat Check)
// -----------------------------------------------------------------------------
pub struct IpcOptions<'a> {
    pub dir: &'a str,
    pub method: &'a str,
    pub params: Option<&'a str>,
    pub timeout: Duration,
}

pub fn execute_ipc_request(opts: IpcOptions) -> Result<Value, String> {
    let raw_dir = opts.dir.trim();
    let resolved_dir_str = resolve_path_value(raw_dir);
    let ipc_dir = std::path::PathBuf::from(&resolved_dir_str);

    mcp_log!("[IPC] Target directory: {}", ipc_dir.display());

    if !ipc_dir.exists() {
        let cwd_cand = std::env::current_dir().map(|c| c.join(raw_dir).display().to_string()).unwrap_or_default();
        let exe_cand = std::env::current_exe().ok().and_then(|e| e.parent().map(|p| p.join(raw_dir).display().to_string())).unwrap_or_default();
        return Err(format!(
            "IPC directory '{}' does not exist.\n  Checked CWD: {}\n  Checked ExeDir: {}\nEnsure the game is running and UE4SS_MCP mod is installed.",
            raw_dir, cwd_cand, exe_cand
        ));
    }

    let heartbeat_file = ipc_dir.join("heartbeat.txt");
    if heartbeat_file.exists() {
        if let Ok(hb_str) = std::fs::read_to_string(&heartbeat_file) {
            if let Ok(hb_time) = hb_str.trim().parse::<u64>() {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                if now > hb_time && (now - hb_time) > 15 {
                    mcp_log!("[IPC][WARN] Stale heartbeat ({}s ago). Game may be paused or hung.", now - hb_time);
                } else {
                    mcp_log!("[IPC] Heartbeat alive (last seen {}s ago).", now.saturating_sub(hb_time));
                }
            }
        }
    } else {
        mcp_log!("[IPC][WARN] No heartbeat.txt found in {}. Mod might not have started yet.", ipc_dir.display());
    }

    let cmd_file = ipc_dir.join("command.json");
    let resp_file = ipc_dir.join("response.json");
    let tmp_cmd_file = ipc_dir.join("command.tmp");

    let _ = std::fs::remove_file(&resp_file);

    let parsed_params: Value = if let Some(p_str) = opts.params {
        serde_json::from_str(p_str).unwrap_or_else(|_| Value::String(p_str.to_string()))
    } else {
        json!({})
    };

    let req_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let cmd_payload = json!({
        "jsonrpc": "2.0",
        "id": req_id,
        "method": opts.method,
        "params": parsed_params
    });

    let payload_str = cmd_payload.to_string();
    mcp_log!("[IPC] Writing command (method='{}', id={}): {}", opts.method, req_id, payload_str);

    if let Err(e) = std::fs::write(&tmp_cmd_file, &payload_str) {
        return Err(format!("Failed to write to '{}': {}", tmp_cmd_file.display(), e));
    }
    if let Err(e) = std::fs::rename(&tmp_cmd_file, &cmd_file) {
        let _ = std::fs::write(&cmd_file, &payload_str);
    }

    let start = std::time::Instant::now();
    let poll_interval = Duration::from_millis(5);

    loop {
        if resp_file.exists() {
            thread::sleep(Duration::from_millis(2));
            match std::fs::read_to_string(&resp_file) {
                Ok(content) => {
                    let _ = std::fs::remove_file(&resp_file);
                    let elapsed = start.elapsed();
                    mcp_log!("[IPC] Response received in {}ms: {}", elapsed.as_millis(), content.trim());
                    if let Ok(resp_json) = serde_json::from_str::<Value>(&content) {
                        if let Some(err) = resp_json.get("error") {
                            if !err.is_null() {
                                return Err(format!("Game IPC Error: {}", err));
                            }
                        }
                        if let Some(res) = resp_json.get("result") {
                            return Ok(res.clone());
                        }
                        return Ok(resp_json);
                    } else {
                        return Ok(Value::String(content));
                    }
                }
                Err(_) => {}
            }
        }

        if start.elapsed() >= opts.timeout {
            let _ = std::fs::remove_file(&cmd_file);
            return Err(format!(
                "IPC Timeout ({}ms) waiting for response from game at '{}'. Method: '{}'",
                opts.timeout.as_millis(),
                ipc_dir.display(),
                opts.method
            ));
        }

        thread::sleep(poll_interval);
    }
}
"###;
