use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::process::{Command, Stdio};
use tempfile::tempdir;

#[test]
fn test_cli_e2e_build_and_run_server() {
    let test_kdl = if cfg!(windows) {
        r#"
server name="test-cli-server" version="1.2.3" {
    description "Integration test server"
}

tool "echo_tool" description="Echo message" {
    param "msg" type="string" required=#true
    exec "cmd" {
        args "/c" "echo" "{msg}"
    }
}

tool "failing_tool" description="Command that fails" {
    exec "cmd" {
        args "/c" "exit" "42"
    }
}
"#
    } else {
        r#"
server name="test-cli-server" version="1.2.3" {
    description "Integration test server"
}

tool "echo_tool" description="Echo message" {
    param "msg" type="string" required=#true
    exec "echo" {
        args "{msg}"
    }
}

tool "failing_tool" description="Command that fails" {
    exec "sh" {
        args "-c" "exit 42"
    }
}
"#
    };

    let temp = tempdir().expect("Failed to create tempdir");
    let kdl_path = temp.path().join("mcp.kdl");
    std::fs::write(&kdl_path, test_kdl).expect("Failed to write test kdl");

    let mut binary_path = temp.path().join("test_server");
    if cfg!(windows) {
        binary_path.set_extension("exe");
    }

    // Build the standalone server binary
    let build_res = mcp_builder_core::Compiler::build(test_kdl, &binary_path, false);
    assert!(
        build_res.is_ok(),
        "Failed to compile test server: {:?}",
        build_res.err()
    );
    assert!(binary_path.exists(), "Binary was not created");

    // Launch the binary as a child process with Stdio pipes
    let mut child = Command::new(&binary_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn compiled server binary");

    let mut stdin = child.stdin.take().expect("Failed to open stdin");
    let stdout = child.stdout.take().expect("Failed to open stdout");
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();

    // 1. Send initialize request
    let init_req = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test-client","version":"1.0.0"}}}"#;
    writeln!(stdin, "{}", init_req).expect("Failed to write to stdin");
    stdin.flush().expect("Failed to flush stdin");

    line.clear();
    reader.read_line(&mut line).expect("Failed to read initialize response");
    let init_resp: serde_json::Value = serde_json::from_str(&line).expect("Invalid JSON in init resp");
    assert_eq!(init_resp["id"], 1);
    assert_eq!(init_resp["result"]["serverInfo"]["name"], "test-cli-server");
    assert_eq!(init_resp["result"]["serverInfo"]["version"], "1.2.3");

    // 2. Send notifications/initialized
    let initialized_notif = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
    writeln!(stdin, "{}", initialized_notif).expect("Failed to write notif");
    stdin.flush().expect("Failed to flush notif");

    // 3. Send ping request
    let ping_req = r#"{"jsonrpc":"2.0","id":2,"method":"ping"}"#;
    writeln!(stdin, "{}", ping_req).expect("Failed to write ping");
    stdin.flush().expect("Failed to flush ping");

    line.clear();
    reader.read_line(&mut line).expect("Failed to read ping response");
    let ping_resp: serde_json::Value = serde_json::from_str(&line).expect("Invalid JSON in ping resp");
    assert_eq!(ping_resp["id"], 2);
    assert_eq!(ping_resp["result"], serde_json::json!({}));

    // 4. Send tools/list request
    let tools_req = r#"{"jsonrpc":"2.0","id":3,"method":"tools/list"}"#;
    writeln!(stdin, "{}", tools_req).expect("Failed to write tools/list");
    stdin.flush().expect("Failed to flush tools/list");

    line.clear();
    reader.read_line(&mut line).expect("Failed to read tools/list response");
    let tools_resp: serde_json::Value = serde_json::from_str(&line).expect("Invalid JSON in tools resp");
    assert_eq!(tools_resp["id"], 3);
    assert_eq!(tools_resp["result"]["tools"][0]["name"], "echo_tool");
    assert_eq!(
        tools_resp["result"]["tools"][0]["inputSchema"]["properties"]["msg"]["type"],
        "string"
    );

    // 5. Send tools/call request (success)
    let call_req = r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"echo_tool","arguments":{"msg":"hello_from_mcp"}}}"#;
    writeln!(stdin, "{}", call_req).expect("Failed to write tools/call");
    stdin.flush().expect("Failed to flush tools/call");

    line.clear();
    reader.read_line(&mut line).expect("Failed to read tools/call response");
    let call_resp: serde_json::Value = serde_json::from_str(&line).expect("Invalid JSON in call resp");
    assert_eq!(call_resp["id"], 4);
    assert_eq!(call_resp["result"]["isError"], false);
    let output_text = call_resp["result"]["content"][0]["text"].as_str().unwrap();
    assert!(output_text.contains("hello_from_mcp"));

    // 6. Send tools/call request for failing tool (error encapsulation with isError: true)
    let fail_req = r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"failing_tool","arguments":{}}}"#;
    writeln!(stdin, "{}", fail_req).expect("Failed to write fail req");
    stdin.flush().expect("Failed to flush fail req");

    line.clear();
    reader.read_line(&mut line).expect("Failed to read fail response");
    let fail_resp: serde_json::Value = serde_json::from_str(&line).expect("Invalid JSON in fail resp");
    assert_eq!(fail_resp["id"], 5);
    assert_eq!(fail_resp["result"]["isError"], true);

    // 7. Send tools/call for unknown tool
    let unknown_req = r#"{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"non_existent","arguments":{}}}"#;
    writeln!(stdin, "{}", unknown_req).expect("Failed to write unknown req");
    stdin.flush().expect("Failed to flush unknown req");

    line.clear();
    reader.read_line(&mut line).expect("Failed to read unknown response");
    let unknown_resp: serde_json::Value = serde_json::from_str(&line).expect("Invalid JSON in unknown resp");
    assert_eq!(unknown_resp["id"], 6);
    assert_eq!(unknown_resp["result"]["isError"], true);

    // 8. Close stdin and verify server shuts down cleanly
    drop(stdin);
    let status = child.wait().expect("Child failed to exit");
    assert!(status.success());
}

// =============================================================================
// Issue 13: Live Protocol Test Matrix & Example Suite Verification
// =============================================================================

fn send_ws_frame(stream: &mut std::net::TcpStream, text: &str) {
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut frame = Vec::new();
    frame.push(0x81); // FIN + Text
    if len < 126 {
        frame.push(len as u8);
    } else if len <= 65535 {
        frame.push(126);
        frame.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        frame.push(127);
        frame.extend_from_slice(&(len as u64).to_be_bytes());
    }
    frame.extend_from_slice(bytes);
    let _ = stream.write_all(&frame);
    let _ = stream.flush();
}

fn read_ws_frame(stream: &mut std::net::TcpStream) -> Option<String> {
    let mut hdr = [0u8; 2];
    if stream.read_exact(&mut hdr).is_err() {
        return None;
    }
    let is_masked = (hdr[1] & 0x80) != 0;
    let len_byte = hdr[1] & 0x7F;
    let len: usize = if len_byte < 126 {
        len_byte as usize
    } else if len_byte == 126 {
        let mut ext = [0u8; 2];
        if stream.read_exact(&mut ext).is_err() {
            return None;
        }
        u16::from_be_bytes(ext) as usize
    } else {
        let mut ext = [0u8; 8];
        if stream.read_exact(&mut ext).is_err() {
            return None;
        }
        u64::from_be_bytes(ext) as usize
    };

    let mask = if is_masked {
        let mut m = [0u8; 4];
        if stream.read_exact(&mut m).is_err() {
            return None;
        }
        Some(m)
    } else {
        None
    };

    let mut body = vec![0u8; len];
    if len > 0 && stream.read_exact(&mut body).is_err() {
        return None;
    }

    if let Some(m) = mask {
        for (i, b) in body.iter_mut().enumerate() {
            *b ^= m[i % 4];
        }
    }

    Some(String::from_utf8_lossy(&body).to_string())
}

fn ws_upgrade_handshake(stream: &mut std::net::TcpStream) -> bool {
    let mut buf = [0u8; 2048];
    let mut read_bytes = 0;
    while read_bytes < buf.len() {
        let n = match stream.read(&mut buf[read_bytes..read_bytes + 1]) {
            Ok(n) if n > 0 => n,
            _ => return false,
        };
        read_bytes += n;
        if read_bytes >= 4 && &buf[read_bytes - 4..read_bytes] == b"\r\n\r\n" {
            break;
        }
    }
    let req = String::from_utf8_lossy(&buf[..read_bytes]);
    if !req.contains("Upgrade: websocket") && !req.contains("upgrade: websocket") {
        return false;
    }
    let resp = "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\r\n";
    stream.write_all(resp.as_bytes()).is_ok() && stream.flush().is_ok()
}

#[test]
fn test_cli_e2e_examples_crossplatform_system_cli() {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let kdl_path = manifest_dir.join("../../examples/crossplatform_system_cli.kdl");
    let kdl_content = std::fs::read_to_string(&kdl_path).expect("Failed to read crossplatform_system_cli.kdl");

    let temp = tempdir().expect("Failed to create tempdir");
    let mut binary_path = temp.path().join("system_cli_mcp");
    if cfg!(windows) {
        binary_path.set_extension("exe");
    }

    let build_res = mcp_builder_core::Compiler::build(&kdl_content, &binary_path, false);
    assert!(build_res.is_ok(), "Build failed: {:?}", build_res.err());

    let mut child = Command::new(&binary_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn system_cli_mcp");

    let mut stdin = child.stdin.take().expect("Failed to open stdin");
    let stdout = child.stdout.take().expect("Failed to open stdout");
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();

    // 1. Initialize
    let init_req = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test-client","version":"1.0.0"}}}"#;
    writeln!(stdin, "{}", init_req).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let init_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(init_resp["id"], 1);
    assert_eq!(init_resp["result"]["serverInfo"]["name"], "system-cli-toolbox");

    let notif = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
    writeln!(stdin, "{}", notif).unwrap();
    stdin.flush().unwrap();

    // 2. tools/list
    let list_req = r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#;
    writeln!(stdin, "{}", list_req).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let list_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(list_resp["id"], 2);
    let tools = list_resp["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 5);

    // 3. Call eval_math
    let math_call = r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"eval_math","arguments":{"expression":"10 * (3 + 4)"}}}"#;
    writeln!(stdin, "{}", math_call).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let math_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(math_resp["id"], 3);
    assert_eq!(math_resp["result"]["isError"], false);
    assert_eq!(math_resp["result"]["content"][0]["text"], "70");

    // 4. Call get_system_info
    let sys_call = r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"get_system_info","arguments":{}}}"#;
    writeln!(stdin, "{}", sys_call).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let sys_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(sys_resp["id"], 4);
    assert_eq!(sys_resp["result"]["isError"], false);
    let sys_json: serde_json::Value = serde_json::from_str(sys_resp["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert!(sys_json.get("system").is_some());
    assert!(sys_json.get("python").is_some());

    // 5. Call list_dir_files with slicing
    let list_call = r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"list_dir_files","arguments":{"path":"."}}}"#;
    writeln!(stdin, "{}", list_call).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let list_files_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(list_files_resp["id"], 5);
    assert_eq!(list_files_resp["result"]["isError"], false);
    assert!(!list_files_resp["result"]["content"][0]["text"].as_str().unwrap().is_empty());

    // 6. Call os_ping
    let ping_call = r#"{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"os_ping","arguments":{"host":"127.0.0.1"}}}"#;
    writeln!(stdin, "{}", ping_call).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let ping_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(ping_resp["id"], 6);
    assert_eq!(ping_resp["result"]["isError"], false);

    // 7. Call os_directory_listing
    let dir_call = r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"os_directory_listing","arguments":{"path":"."}}}"#;
    writeln!(stdin, "{}", dir_call).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let dir_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(dir_resp["id"], 7);
    assert_eq!(dir_resp["result"]["isError"], false);

    drop(stdin);
    let status = child.wait().expect("Child failed to exit");
    assert!(status.success());
}

#[test]
fn test_cli_e2e_examples_python_debugger_pipe() {
    let pipe_listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind mock pipe");
    let pipe_port = pipe_listener.local_addr().unwrap().port();

    let pipe_handle = std::thread::spawn(move || {
        for mut stream in pipe_listener.incoming().flatten() {
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            if reader.read_line(&mut line).is_ok() {
                let trimmed = line.trim();
                if trimmed.contains("\"action\":\"eval\"") {
                    if trimmed.contains("modules") {
                        let resp = r#"{"status":"ok","result":["sys","os","json","math"]}"#;
                        let _ = stream.write_all(format!("{}\n", resp).as_bytes());
                    } else {
                        let resp = r#"{"status":"ok","result":"65536"}"#;
                        let _ = stream.write_all(format!("{}\n", resp).as_bytes());
                    }
                } else if trimmed.contains("\"action\":\"ping\"") {
                    let resp = r#"{"status":"pong","alive":true}"#;
                    let _ = stream.write_all(format!("{}\n", resp).as_bytes());
                }
                let _ = stream.flush();
            }
        }
    });

    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let kdl_path = manifest_dir.join("../../examples/python_debugger_pipe.kdl");
    let kdl_content = std::fs::read_to_string(&kdl_path).expect("Failed to read python_debugger_pipe.kdl");

    let temp = tempdir().expect("Failed to create tempdir");
    let mut binary_path = temp.path().join("debugger_mcp");
    if cfg!(windows) {
        binary_path.set_extension("exe");
    }

    let build_res = mcp_builder_core::Compiler::build(&kdl_content, &binary_path, false);
    assert!(build_res.is_ok(), "Build failed: {:?}", build_res.err());

    let mut child = Command::new(&binary_path)
        .env("DEBUG_PORT", pipe_port.to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn debugger_mcp");

    let mut stdin = child.stdin.take().expect("Failed to open stdin");
    let stdout = child.stdout.take().expect("Failed to open stdout");
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();

    // 1. Initialize
    let init_req = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test-client","version":"1.0.0"}}}"#;
    writeln!(stdin, "{}", init_req).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let init_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(init_resp["id"], 1);
    assert_eq!(init_resp["result"]["serverInfo"]["name"], "python-debugger-bridge");

    let notif = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
    writeln!(stdin, "{}", notif).unwrap();
    stdin.flush().unwrap();

    // 2. tools/list
    let list_req = r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#;
    writeln!(stdin, "{}", list_req).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let list_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(list_resp["id"], 2);
    let tools = list_resp["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 3);

    // 3. Call dbg_eval
    let eval_call = r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"dbg_eval","arguments":{"code":"2 ** 16"}}}"#;
    writeln!(stdin, "{}", eval_call).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let eval_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(eval_resp["id"], 3);
    assert_eq!(eval_resp["result"]["isError"], false);
    assert_eq!(eval_resp["result"]["content"][0]["text"], "65536");

    // 4. Call dbg_ping
    let ping_call = r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"dbg_ping","arguments":{}}}"#;
    writeln!(stdin, "{}", ping_call).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let ping_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(ping_resp["id"], 4);
    assert_eq!(ping_resp["result"]["isError"], false);
    let ping_json: serde_json::Value = serde_json::from_str(ping_resp["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(ping_json["status"], "pong");
    assert_eq!(ping_json["alive"], true);

    // 5. Call dbg_list_modules
    let mod_call = r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"dbg_list_modules","arguments":{}}}"#;
    writeln!(stdin, "{}", mod_call).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let mod_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(mod_resp["id"], 5);
    assert_eq!(mod_resp["result"]["isError"], false);
    let mod_json: serde_json::Value = serde_json::from_str(mod_resp["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert!(mod_json.as_array().unwrap().contains(&serde_json::json!("sys")));

    drop(stdin);
    let _ = child.wait();
    drop(pipe_handle);
}

#[test]
fn test_cli_e2e_examples_chrome_cdp() {
    let ws_listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind mock WS for CDP");
    let ws_port = ws_listener.local_addr().unwrap().port();

    let ws_handle = std::thread::spawn(move || {
        for mut stream in ws_listener.incoming().flatten() {
            if ws_upgrade_handshake(&mut stream) {
                if let Some(msg) = read_ws_frame(&mut stream) {
                    if msg.contains("Runtime.evaluate") {
                        let reply = r#"{"id":1001,"result":{"result":{"type":"string","value":"CDP_EVAL_SUCCESS_42"}}}"#;
                        send_ws_frame(&mut stream, reply);
                    } else if msg.contains("Page.navigate") {
                        let reply = r#"{"id":1002,"result":{"frameId":"F123456"}}"#;
                        send_ws_frame(&mut stream, reply);
                    } else if msg.contains("Browser.getVersion") {
                        let reply = r#"{"id":1004,"result":{"protocolVersion":"1.3","product":"Chrome/120.0","userAgent":"MockCDP/1.0"}}"#;
                        send_ws_frame(&mut stream, reply);
                    }
                }
            }
        }
    });

    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let kdl_path = manifest_dir.join("../../examples/chrome_cdp.kdl");
    let kdl_content = std::fs::read_to_string(&kdl_path).expect("Failed to read chrome_cdp.kdl");

    let temp = tempdir().expect("Failed to create tempdir");
    let mut binary_path = temp.path().join("chrome_mcp");
    if cfg!(windows) {
        binary_path.set_extension("exe");
    }

    let build_res = mcp_builder_core::Compiler::build(&kdl_content, &binary_path, false);
    assert!(build_res.is_ok(), "Build failed: {:?}", build_res.err());

    let mut child = Command::new(&binary_path)
        .env("CDP_PORT", ws_port.to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn chrome_mcp");

    let mut stdin = child.stdin.take().expect("Failed to open stdin");
    let stdout = child.stdout.take().expect("Failed to open stdout");
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();

    // 1. Initialize
    let init_req = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test-client","version":"1.0.0"}}}"#;
    writeln!(stdin, "{}", init_req).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let init_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(init_resp["id"], 1);
    assert_eq!(init_resp["result"]["serverInfo"]["name"], "chrome-devtools-bridge");

    let notif = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
    writeln!(stdin, "{}", notif).unwrap();
    stdin.flush().unwrap();

    // 2. tools/list
    let list_req = r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#;
    writeln!(stdin, "{}", list_req).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let list_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(list_resp["id"], 2);
    let tools = list_resp["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 4);

    // 3. Call cdp_eval_js
    let eval_call = r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"cdp_eval_js","arguments":{"target_id":"page_1","expression":"1 + 1"}}}"#;
    writeln!(stdin, "{}", eval_call).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let eval_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(eval_resp["id"], 3);
    assert_eq!(eval_resp["result"]["isError"], false);
    assert_eq!(eval_resp["result"]["content"][0]["text"], "CDP_EVAL_SUCCESS_42");

    // 4. Call cdp_navigate
    let nav_call = r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"cdp_navigate","arguments":{"target_id":"page_1","url":"https://example.com"}}}"#;
    writeln!(stdin, "{}", nav_call).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let nav_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(nav_resp["id"], 4);
    assert_eq!(nav_resp["result"]["isError"], false);
    assert_eq!(nav_resp["result"]["content"][0]["text"], "F123456");

    // 5. Call cdp_get_browser_version
    let ver_call = r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"cdp_get_browser_version","arguments":{}}}"#;
    writeln!(stdin, "{}", ver_call).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let ver_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(ver_resp["id"], 5);
    assert_eq!(ver_resp["result"]["isError"], false);
    let ver_json: serde_json::Value = serde_json::from_str(ver_resp["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(ver_json["result"]["product"], "Chrome/120.0");

    drop(stdin);
    let _ = child.wait();
    drop(ws_handle);
}

#[test]
fn test_cli_e2e_examples_obs_control() {
    let ws_listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind mock WS for OBS");
    let ws_port = ws_listener.local_addr().unwrap().port();

    let ws_handle = std::thread::spawn(move || {
        for mut stream in ws_listener.incoming().flatten() {
            if ws_upgrade_handshake(&mut stream) {
                if let Some(msg) = read_ws_frame(&mut stream) {
                    if msg.contains("GetStats") {
                        let reply = r#"{"op":7,"d":{"requestType":"GetStats","requestId":"req_stats","requestStatus":{"result":true,"code":100},"responseData":{"activeFps":60.0,"cpuUsage":4.5}}}"#;
                        send_ws_frame(&mut stream, reply);
                    } else if msg.contains("ToggleRecord") {
                        let reply = r#"{"op":7,"d":{"requestType":"ToggleRecord","requestId":"req_rec","requestStatus":{"result":true,"code":100},"responseData":{"outputActive":true}}}"#;
                        send_ws_frame(&mut stream, reply);
                    } else if msg.contains("SetCurrentProgramScene") {
                        let reply = r#"{"op":7,"d":{"requestType":"SetCurrentProgramScene","requestId":"req_switch","requestStatus":{"result":true,"code":100}}}"#;
                        send_ws_frame(&mut stream, reply);
                    }
                }
            }
        }
    });

    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let kdl_path = manifest_dir.join("../../examples/obs_control.kdl");
    let kdl_content = std::fs::read_to_string(&kdl_path).expect("Failed to read obs_control.kdl");

    let temp = tempdir().expect("Failed to create tempdir");
    let mut binary_path = temp.path().join("obs_mcp");
    if cfg!(windows) {
        binary_path.set_extension("exe");
    }

    let build_res = mcp_builder_core::Compiler::build(&kdl_content, &binary_path, false);
    assert!(build_res.is_ok(), "Build failed: {:?}", build_res.err());

    let mut child = Command::new(&binary_path)
        .env("OBS_PORT", ws_port.to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn obs_mcp");

    let mut stdin = child.stdin.take().expect("Failed to open stdin");
    let stdout = child.stdout.take().expect("Failed to open stdout");
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();

    // 1. Initialize
    let init_req = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test-client","version":"1.0.0"}}}"#;
    writeln!(stdin, "{}", init_req).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let init_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(init_resp["id"], 1);
    assert_eq!(init_resp["result"]["serverInfo"]["name"], "obs-studio-bridge");

    let notif = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
    writeln!(stdin, "{}", notif).unwrap();
    stdin.flush().unwrap();

    // 2. tools/list
    let list_req = r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#;
    writeln!(stdin, "{}", list_req).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let list_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(list_resp["id"], 2);
    let tools = list_resp["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 4);

    // 3. Call obs_get_stats
    let stats_call = r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"obs_get_stats","arguments":{}}}"#;
    writeln!(stdin, "{}", stats_call).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let stats_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(stats_resp["id"], 3);
    assert_eq!(stats_resp["result"]["isError"], false);
    let stats_json: serde_json::Value = serde_json::from_str(stats_resp["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(stats_json["activeFps"], 60.0);
    assert_eq!(stats_json["cpuUsage"], 4.5);

    // 4. Call obs_toggle_record
    let toggle_call = r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"obs_toggle_record","arguments":{}}}"#;
    writeln!(stdin, "{}", toggle_call).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let toggle_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(toggle_resp["id"], 4);
    assert_eq!(toggle_resp["result"]["isError"], false);
    assert_eq!(toggle_resp["result"]["content"][0]["text"], "true");

    // 5. Call obs_set_current_scene
    let scene_call = r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"obs_set_current_scene","arguments":{"scene_name":"GamingScene"}}}"#;
    writeln!(stdin, "{}", scene_call).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let scene_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(scene_resp["id"], 5);
    assert_eq!(scene_resp["result"]["isError"], false);

    drop(stdin);
    let _ = child.wait();
    drop(ws_handle);
}

#[test]
fn test_cli_e2e_examples_httpbin() {
    let http_listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind mock HTTP");
    let http_port = http_listener.local_addr().unwrap().port();

    let http_handle = std::thread::spawn(move || {
        for mut stream in http_listener.incoming().flatten() {
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut lines = Vec::new();
            let mut line = String::new();
            let mut content_len = 0;
            while let Ok(n) = reader.read_line(&mut line) {
                if n == 0 || line == "\r\n" || line == "\n" {
                    break;
                }
                if line.to_lowercase().starts_with("content-length:") {
                    if let Some(val) = line.split(':').nth(1) {
                        content_len = val.trim().parse::<usize>().unwrap_or(0);
                    }
                }
                lines.push(line.clone());
                line.clear();
            }
            if lines.is_empty() {
                continue;
            }
            let first_line = &lines[0];
            let mut body = vec![0u8; content_len];
            if content_len > 0 {
                let _ = reader.read_exact(&mut body);
            }
            let body_str = String::from_utf8_lossy(&body);

            let (status, resp_body) = if first_line.starts_with("GET /ip") {
                ("200 OK", serde_json::json!({ "origin": "192.168.1.100" }).to_string())
            } else if first_line.starts_with("POST /post") {
                let parsed: serde_json::Value = serde_json::from_str(&body_str).unwrap_or(serde_json::Value::Null);
                ("200 OK", serde_json::json!({ "json": parsed }).to_string())
            } else if first_line.starts_with("GET /get") {
                let mut args = serde_json::Map::new();
                if let Some(query) = first_line.split_whitespace().nth(1).and_then(|u| u.split('?').nth(1)) {
                    for pair in query.split('&') {
                        let mut parts = pair.split('=');
                        if let (Some(k), Some(v)) = (parts.next(), parts.next()) {
                            args.insert(k.to_string(), serde_json::Value::String(v.to_string()));
                        }
                    }
                }
                ("200 OK", serde_json::json!({ "args": args }).to_string())
            } else if first_line.starts_with("GET /headers") {
                let mut headers_map = serde_json::Map::new();
                for h in &lines[1..] {
                    if let Some((k, v)) = h.split_once(':') {
                        headers_map.insert(k.trim().to_string(), serde_json::Value::String(v.trim().to_string()));
                    }
                }
                ("200 OK", serde_json::json!({ "headers": headers_map }).to_string())
            } else {
                ("404 Not Found", serde_json::json!({ "error": "Not Found" }).to_string())
            };

            let resp = format!(
                "HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                status,
                resp_body.len(),
                resp_body
            );
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.flush();
        }
    });

    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let kdl_path = manifest_dir.join("../../examples/httpbin.kdl");
    let kdl_content = std::fs::read_to_string(&kdl_path).expect("Failed to read httpbin.kdl");

    let temp = tempdir().expect("Failed to create tempdir");
    let mut binary_path = temp.path().join("httpbin_mcp");
    if cfg!(windows) {
        binary_path.set_extension("exe");
    }

    let build_res = mcp_builder_core::Compiler::build(&kdl_content, &binary_path, false);
    assert!(build_res.is_ok(), "Build failed: {:?}", build_res.err());

    let mut child = Command::new(&binary_path)
        .env("HTTPBIN_BASE_URL", format!("http://127.0.0.1:{}", http_port))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn httpbin_mcp");

    let mut stdin = child.stdin.take().expect("Failed to open stdin");
    let stdout = child.stdout.take().expect("Failed to open stdout");
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();

    // 1. Initialize
    let init_req = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test-client","version":"1.0.0"}}}"#;
    writeln!(stdin, "{}", init_req).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let init_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(init_resp["id"], 1);
    assert_eq!(init_resp["result"]["serverInfo"]["name"], "httpbin-tester");

    let notif = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
    writeln!(stdin, "{}", notif).unwrap();
    stdin.flush().unwrap();

    // 2. tools/list
    let list_req = r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#;
    writeln!(stdin, "{}", list_req).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let list_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(list_resp["id"], 2);
    let tools = list_resp["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 4);

    // 3. Call http_get_ip
    let ip_call = r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"http_get_ip","arguments":{}}}"#;
    writeln!(stdin, "{}", ip_call).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let ip_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(ip_resp["id"], 3);
    assert_eq!(ip_resp["result"]["isError"], false);
    assert_eq!(ip_resp["result"]["content"][0]["text"], "192.168.1.100");

    // 4. Call http_post_json
    let post_call = r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"http_post_json","arguments":{"message":"HelloHttp","tag":"auth_test"}}}"#;
    writeln!(stdin, "{}", post_call).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let post_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(post_resp["id"], 4);
    assert_eq!(post_resp["result"]["isError"], false);
    let post_json: serde_json::Value = serde_json::from_str(post_resp["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(post_json["message"], "HelloHttp");
    assert_eq!(post_json["tag"], "auth_test");

    // 5. Call http_query_params
    let query_call = r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"http_query_params","arguments":{"search_term":"rust_compiler","limit":25}}}"#;
    writeln!(stdin, "{}", query_call).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let query_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(query_resp["id"], 5);
    assert_eq!(query_resp["result"]["isError"], false);
    let query_json: serde_json::Value = serde_json::from_str(query_resp["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(query_json["q"], "rust_compiler");
    assert_eq!(query_json["limit"], "25");

    // 6. Call http_inspect_headers
    let headers_call = r#"{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"http_inspect_headers","arguments":{"custom_header":"MyTag/2.0"}}}"#;
    writeln!(stdin, "{}", headers_call).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let headers_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(headers_resp["id"], 6);
    assert_eq!(headers_resp["result"]["isError"], false);
    let headers_json: serde_json::Value = serde_json::from_str(headers_resp["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(headers_json["headers"]["X-Client-Tag"], "MyTag/2.0");

    drop(stdin);
    let _ = child.wait();
    drop(http_handle);
}

#[test]
fn test_cli_e2e_build_and_run_sse_server() {
    let test_kdl = if cfg!(windows) {
        r#"
server name="test-sse-server" version="1.0.0" {
    description "SSE test server"
}

tool "echo_tool" description="Echo message" {
    param "msg" type="string" required=#true
    exec "cmd" {
        args "/c" "echo" "{msg}"
    }
}
"#
    } else {
        r#"
server name="test-sse-server" version="1.0.0" {
    description "SSE test server"
}

tool "echo_tool" description="Echo message" {
    param "msg" type="string" required=#true
    exec "echo" {
        args "{msg}"
    }
}
"#
    };

    let temp = tempdir().expect("Failed to create tempdir");
    let mut binary_path = temp.path().join("test_sse_server");
    if cfg!(windows) {
        binary_path.set_extension("exe");
    }

    let build_res = mcp_builder_core::Compiler::build(test_kdl, &binary_path, false);
    assert!(build_res.is_ok(), "Failed to compile test server: {:?}", build_res.err());

    // Pick an available port
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("Failed to bind port");
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let mut child = Command::new(&binary_path)
        .arg("-t")
        .arg("sse")
        .arg("--host")
        .arg("127.0.0.1")
        .arg("-p")
        .arg(port.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn SSE server");

    // Give the server a moment to start listening
    std::thread::sleep(std::time::Duration::from_millis(600));

    // 1. Test OPTIONS /sse (CORS)
    {
        let mut stream = std::net::TcpStream::connect(format!("127.0.0.1:{}", port))
            .expect("Failed to connect to SSE server");
        let req = format!("OPTIONS /sse HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n\r\n", port);
        stream.write_all(req.as_bytes()).unwrap();
        stream.flush().unwrap();

        let mut buf = [0u8; 1024];
        let n = stream.read(&mut buf).unwrap();
        let resp = String::from_utf8_lossy(&buf[..n]);
        assert!(resp.contains("204 No Content"), "Expected 204 No Content, got: {}", resp);
        assert!(resp.contains("Access-Control-Allow-Origin: *"));
    }

    // 2. Test GET /sse stream connection
    let mut sse_stream = std::net::TcpStream::connect(format!("127.0.0.1:{}", port))
        .expect("Failed to connect GET /sse");
    let get_req = format!("GET /sse HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n\r\n", port);
    sse_stream.write_all(get_req.as_bytes()).unwrap();
    sse_stream.flush().unwrap();

    let mut sse_reader = BufReader::new(sse_stream.try_clone().unwrap());
    let mut session_id = String::new();

    // Read until we get the endpoint event
    let mut line = String::new();
    while sse_reader.read_line(&mut line).unwrap() > 0 {
        if line.starts_with("data: /message?sessionId=") {
            session_id = line
                .trim()
                .strip_prefix("data: /message?sessionId=")
                .unwrap()
                .to_string();
            break;
        }
        line.clear();
    }
    assert!(!session_id.is_empty(), "Failed to extract session_id from SSE stream");

    // 3. Test POST /message?sessionId=<sid> with initialize
    {
        let mut post_stream = std::net::TcpStream::connect(format!("127.0.0.1:{}", port))
            .expect("Failed to connect POST /message");
        let init_body = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test-client","version":"1.0.0"}}}"#;
        let post_req = format!(
            "POST /message?sessionId={} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{}",
            session_id,
            port,
            init_body.len(),
            init_body
        );
        post_stream.write_all(post_req.as_bytes()).unwrap();
        post_stream.flush().unwrap();

        let mut buf = [0u8; 1024];
        let n = post_stream.read(&mut buf).unwrap();
        let resp = String::from_utf8_lossy(&buf[..n]);
        assert!(resp.contains("202 Accepted"), "Expected 202 Accepted, got: {}", resp);
    }

    // Read the initialize response from SSE stream
    line.clear();
    let mut init_response_json = String::new();
    while sse_reader.read_line(&mut line).unwrap() > 0 {
        if line.starts_with("data: ") {
            init_response_json = line.strip_prefix("data: ").unwrap().trim().to_string();
            break;
        }
        line.clear();
    }
    let init_resp: serde_json::Value = serde_json::from_str(&init_response_json).unwrap();
    assert_eq!(init_resp["id"], 1);
    assert_eq!(init_resp["result"]["serverInfo"]["name"], "test-sse-server");

    // 4. Test direct POST without sessionId (e.g. tools/list)
    {
        let mut post_stream = std::net::TcpStream::connect(format!("127.0.0.1:{}", port))
            .expect("Failed to connect direct POST");
        let list_body = r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#;
        let post_req = format!(
            "POST /message HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{}",
            port,
            list_body.len(),
            list_body
        );
        post_stream.write_all(post_req.as_bytes()).unwrap();
        post_stream.flush().unwrap();

        let mut buf = [0u8; 2048];
        let n = post_stream.read(&mut buf).unwrap();
        let resp = String::from_utf8_lossy(&buf[..n]);
        assert!(resp.contains("200 OK"), "Expected 200 OK, got: {}", resp);
        assert!(resp.contains("echo_tool"), "Response missing echo_tool: {}", resp);
    }

    // 5. Test tool execution via SSE stream
    {
        let mut post_stream = std::net::TcpStream::connect(format!("127.0.0.1:{}", port))
            .expect("Failed to connect tool execution POST");
        let call_body = r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"echo_tool","arguments":{"msg":"sse_test_passed"}}}"#;
        let post_req = format!(
            "POST /message?sessionId={} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{}",
            session_id,
            port,
            call_body.len(),
            call_body
        );
        post_stream.write_all(post_req.as_bytes()).unwrap();
        post_stream.flush().unwrap();
    }

    // Read tool execution response from SSE stream
    line.clear();
    let mut call_response_json = String::new();
    while sse_reader.read_line(&mut line).unwrap() > 0 {
        if line.starts_with("data: ") {
            call_response_json = line.strip_prefix("data: ").unwrap().trim().to_string();
            break;
        }
        line.clear();
    }
    let call_resp: serde_json::Value = serde_json::from_str(&call_response_json).unwrap();
    assert_eq!(call_resp["id"], 3);
    assert_eq!(call_resp["result"]["isError"], false);
    let output_text = call_resp["result"]["content"][0]["text"].as_str().unwrap();
    assert!(output_text.contains("sse_test_passed"));

    // Cleanup child process
    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn test_cli_e2e_prompts_and_resources() {
    let test_kdl = if cfg!(windows) {
        r#"
server name="test-prompts-resources" version="1.0.0" {
    description "Prompts and Resources test server"
}

resource "system://info" name="System Info" {
    description "System info"
    mime-type "application/json"
    text "{\"os\":\"test-os\"}"
}

resource-template "system://echo/{msg}" name="Echo Template" {
    description "Echo template"
    mime-type "text/plain"
    (string)param "msg" required=#true
    bind:exec "cmd" {
        args "/c" "echo" "{msg}"
    }
}

prompt "review" description="Code review" {
    argument "diff" required=#true description="Git diff"
    argument "style" required=#false default="concise" description="Review style"
    message role="user" "Review in {style} style: {diff}"
}
"#
    } else {
        r#"
server name="test-prompts-resources" version="1.0.0" {
    description "Prompts and Resources test server"
}

resource "system://info" name="System Info" {
    description "System info"
    mime-type "application/json"
    text "{\"os\":\"test-os\"}"
}

resource-template "system://echo/{msg}" name="Echo Template" {
    description "Echo template"
    mime-type "text/plain"
    (string)param "msg" required=#true
    bind:exec "echo" {
        args "{msg}"
    }
}

prompt "review" description="Code review" {
    argument "diff" required=#true description="Git diff"
    argument "style" required=#false default="concise" description="Review style"
    message role="user" "Review in {style} style: {diff}"
}
"#
    };

    let temp = tempdir().expect("Failed to create tempdir");
    let mut binary_path = temp.path().join("test_pr_server");
    if cfg!(windows) {
        binary_path.set_extension("exe");
    }

    let build_res = mcp_builder_core::Compiler::build(test_kdl, &binary_path, false);
    assert!(build_res.is_ok(), "Failed to compile test server: {:?}", build_res.err());

    let mut child = Command::new(&binary_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn compiled server binary");

    let mut stdin = child.stdin.take().expect("Failed to open stdin");
    let stdout = child.stdout.take().expect("Failed to open stdout");
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();

    // 1. Initialize
    let init_req = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test-client","version":"1.0.0"}}}"#;
    writeln!(stdin, "{}", init_req).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let init_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(init_resp["id"], 1);
    assert_eq!(init_resp["result"]["capabilities"]["prompts"]["listChanged"], false);
    assert_eq!(init_resp["result"]["capabilities"]["resources"]["subscribe"], false);

    // 2. prompts/list
    let plist_req = r#"{"jsonrpc":"2.0","id":2,"method":"prompts/list"}"#;
    writeln!(stdin, "{}", plist_req).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let plist_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(plist_resp["id"], 2);
    assert_eq!(plist_resp["result"]["prompts"][0]["name"], "review");

    // 3. prompts/get with explicit arguments
    let pget_req = r#"{"jsonrpc":"2.0","id":3,"method":"prompts/get","params":{"name":"review","arguments":{"diff":"+line1","style":"verbose"}}}"#;
    writeln!(stdin, "{}", pget_req).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let pget_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(pget_resp["id"], 3);
    assert!(pget_resp["error"].is_null());
    let msg_text = pget_resp["result"]["messages"][0]["content"]["text"].as_str().unwrap();
    assert_eq!(msg_text, "Review in verbose style: +line1");

    // 4. prompts/get with default argument fallback
    let pget_def_req = r#"{"jsonrpc":"2.0","id":4,"method":"prompts/get","params":{"name":"review","arguments":{"diff":"+line2"}}}"#;
    writeln!(stdin, "{}", pget_def_req).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let pget_def_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(pget_def_resp["id"], 4);
    assert!(pget_def_resp["error"].is_null());
    let msg_def_text = pget_def_resp["result"]["messages"][0]["content"]["text"].as_str().unwrap();
    assert_eq!(msg_def_text, "Review in concise style: +line2");

    // 5. prompts/get missing required argument -> error
    let pget_err_req = r#"{"jsonrpc":"2.0","id":5,"method":"prompts/get","params":{"name":"review","arguments":{}}}"#;
    writeln!(stdin, "{}", pget_err_req).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let pget_err_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(pget_err_resp["id"], 5);
    assert!(!pget_err_resp["error"].is_null());
    assert!(pget_err_resp["error"]["message"].as_str().unwrap().contains("diff"));

    // 6. resources/list
    let rlist_req = r#"{"jsonrpc":"2.0","id":6,"method":"resources/list"}"#;
    writeln!(stdin, "{}", rlist_req).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let rlist_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(rlist_resp["id"], 6);
    let r_arr = rlist_resp["result"]["resources"].as_array().unwrap();
    assert_eq!(r_arr.len(), 2);

    // 7. resources/read static text resource
    let rread_static_req = r#"{"jsonrpc":"2.0","id":7,"method":"resources/read","params":{"uri":"system://info"}}"#;
    writeln!(stdin, "{}", rread_static_req).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let rread_static_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(rread_static_resp["id"], 7);
    assert!(rread_static_resp["error"].is_null());
    let static_text = rread_static_resp["result"]["contents"][0]["text"].as_str().unwrap();
    assert_eq!(static_text, "{\"os\":\"test-os\"}");

    // 8. resources/read dynamic template resource
    let rread_tmpl_req = r#"{"jsonrpc":"2.0","id":8,"method":"resources/read","params":{"uri":"system://echo/dynamic_val"}}"#;
    writeln!(stdin, "{}", rread_tmpl_req).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let rread_tmpl_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(rread_tmpl_resp["id"], 8);
    assert!(rread_tmpl_resp["error"].is_null());
    let tmpl_text = rread_tmpl_resp["result"]["contents"][0]["text"].as_str().unwrap();
    assert!(tmpl_text.contains("dynamic_val"));

    // 9. resources/read non-existent resource -> error
    let rread_err_req = r#"{"jsonrpc":"2.0","id":9,"method":"resources/read","params":{"uri":"system://non_existent"}}"#;
    writeln!(stdin, "{}", rread_err_req).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let rread_err_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(rread_err_resp["id"], 9);
    assert!(!rread_err_resp["error"].is_null());

    // 10. Clean shutdown
    drop(stdin);
    let status = child.wait().expect("Child failed to exit");
    assert!(status.success());
}

#[test]
fn test_cli_e2e_build_and_run_com_server() {
    if !cfg!(windows) {
        return;
    }

    let test_kdl = r#"
server name="test-com-server" version="1.0.0" {
    description "Windows COM Automation Test Server"
}

tool "get_windir" description="Get Windows directory via WScript.Shell COM" {
    param "var_name" type="string" default="%WINDIR%"
    bind:com "WScript.Shell" {
        dispatch "ExpandEnvironmentStrings"
        args "{var_name}"
        attach #false
        timeout-ms 5000
    }
}

tool "get_temp_name" description="Get temp filename via FileSystemObject COM" {
    bind:com progid="Scripting.FileSystemObject" {
        call "GetTempName"
        attach #false
        timeout-ms 5000
    }
}
"#;

    let temp = tempdir().expect("Failed to create tempdir");
    let binary_path = temp.path().join("test_com_server.exe");

    let build_res = mcp_builder_core::Compiler::build(test_kdl, &binary_path, false);
    assert!(
        build_res.is_ok(),
        "Failed to compile COM test server: {:?}",
        build_res.err()
    );
    assert!(binary_path.exists(), "Binary was not created");

    let mut child = Command::new(&binary_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn compiled COM server binary");

    let mut stdin = child.stdin.take().expect("Failed to open stdin");
    let stdout = child.stdout.take().expect("Failed to open stdout");
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();

    // 1. Initialize
    let init_req = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test-client","version":"1.0.0"}}}"#;
    writeln!(stdin, "{}", init_req).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let init_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(init_resp["id"], 1);
    assert_eq!(init_resp["result"]["serverInfo"]["name"], "test-com-server");

    // 2. Initialized notification
    let initialized_notif = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
    writeln!(stdin, "{}", initialized_notif).unwrap();
    stdin.flush().unwrap();

    // 3. tools/list
    let tools_req = r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#;
    writeln!(stdin, "{}", tools_req).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let tools_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(tools_resp["id"], 2);
    let tools = tools_resp["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 2);
    assert_eq!(tools[0]["name"], "get_windir");
    assert_eq!(tools[1]["name"], "get_temp_name");

    // 4. tools/call get_windir via WScript.Shell COM
    let call_windir_req = r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"get_windir","arguments":{"var_name":"%WINDIR%"}}}"#;
    writeln!(stdin, "{}", call_windir_req).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let call_windir_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(call_windir_resp["id"], 3);
    assert_eq!(call_windir_resp["result"]["isError"], false);
    let windir_text = call_windir_resp["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        windir_text.to_lowercase().contains("windows") || windir_text.to_lowercase().contains("win"),
        "Unexpected windir text: {}",
        windir_text
    );

    // 5. tools/call get_temp_name via Scripting.FileSystemObject COM
    let call_temp_req = r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"get_temp_name","arguments":{}}}"#;
    writeln!(stdin, "{}", call_temp_req).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let call_temp_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(call_temp_resp["id"], 4);
    assert_eq!(call_temp_resp["result"]["isError"], false);
    let temp_name = call_temp_resp["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        temp_name.to_lowercase().ends_with(".tmp"),
        "Expected .tmp file name from FileSystemObject, got: {}",
        temp_name
    );

    // 6. Clean shutdown
    drop(stdin);
    let status = child.wait().expect("Child failed to exit");
    assert!(status.success());
}

#[test]
fn test_cli_e2e_build_and_run_output_pipelines_server() {
    let test_kdl = if cfg!(windows) {
        r#"
server name="test-output-server" version="1.0.0" {
    description "Output pipeline end-to-end testing"
}

tool "test_trim" description="Test trim transformation" {
    exec "cmd" {
        args "/c" "echo   padded text   "
    }
    output {
        trim #true
    }
}

tool "test_slice_head" description="Test slice head transformation" {
    exec "cmd" {
        args "/c" "echo line1&& echo line2&& echo line3&& echo line4"
    }
    output {
        slice lines=2 head=#true
        trim #true
    }
}

tool "test_slice_tail" description="Test slice tail transformation" {
    exec "cmd" {
        args "/c" "echo line1&& echo line2&& echo line3&& echo line4"
    }
    output {
        slice lines=2 tail=#true
        trim #true
    }
}

tool "test_filter_not" description="Test filter-not transformation" {
    exec "cmd" {
        args "/c" "echo DEBUG: starting&& echo INFO: ready&& echo DEBUG: step1&& echo INFO: done"
    }
    output {
        filter-not "DEBUG:"
        trim #true
    }
}

tool "test_extract_json" description="Test JSON Pointer extraction" {
    exec "powershell" {
        args "-NoProfile" "-Command" "Write-Output '{\"status\":\"ok\",\"user\":{\"email\":\"dev@mcp.org\",\"level\":42}}'"
    }
    output {
        extract-json "/user/email"
    }
}

tool "test_regex_template" description="Test regex extraction and template formatting" {
    exec "cmd" {
        args "/c" "echo Branch: release/v2.5, Commit: 7a8f9b"
    }
    output {
        regex "^Branch: (?P<branch>[^,]+), Commit: (?P<hash>\\w+)$" {
            template "Release '{branch}' at hash '{hash}'"
        }
    }
}
"#
    } else {
        r#"
server name="test-output-server" version="1.0.0" {
    description "Output pipeline end-to-end testing"
}

tool "test_trim" description="Test trim transformation" {
    exec "sh" {
        args "-c" "printf '  padded text   \n'"
    }
    output {
        trim #true
    }
}

tool "test_slice_head" description="Test slice head transformation" {
    exec "sh" {
        args "-c" "printf 'line1\nline2\nline3\nline4\n'"
    }
    output {
        slice lines=2 head=#true
        trim #true
    }
}

tool "test_slice_tail" description="Test slice tail transformation" {
    exec "sh" {
        args "-c" "printf 'line1\nline2\nline3\nline4\n'"
    }
    output {
        slice lines=2 tail=#true
        trim #true
    }
}

tool "test_filter_not" description="Test filter-not transformation" {
    exec "sh" {
        args "-c" "printf 'DEBUG: starting\nINFO: ready\nDEBUG: step1\nINFO: done\n'"
    }
    output {
        filter-not "DEBUG:"
        trim #true
    }
}

tool "test_extract_json" description="Test JSON Pointer extraction" {
    exec "sh" {
        args "-c" "printf '{\"status\":\"ok\",\"user\":{\"email\":\"dev@mcp.org\",\"level\":42}}\n'"
    }
    output {
        extract-json "/user/email"
    }
}

tool "test_regex_template" description="Test regex extraction and template formatting" {
    exec "sh" {
        args "-c" "printf 'Branch: release/v2.5, Commit: 7a8f9b\n'"
    }
    output {
        regex "^Branch: (?P<branch>[^,]+), Commit: (?P<hash>\\w+)$" {
            template "Release '{branch}' at hash '{hash}'"
        }
    }
}
"#
    };

    let temp = tempdir().expect("Failed to create tempdir");
    let mut binary_path = temp.path().join("test_output_server");
    if cfg!(windows) {
        binary_path.set_extension("exe");
    }

    let build_res = mcp_builder_core::Compiler::build(test_kdl, &binary_path, false);
    assert!(
        build_res.is_ok(),
        "Failed to compile output server: {:?}",
        build_res.err()
    );

    let mut child = Command::new(&binary_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn server binary");

    let mut stdin = child.stdin.take().expect("Failed to open stdin");
    let stdout = child.stdout.take().expect("Failed to open stdout");
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();

    // 1. Initialize
    let init_req = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}"#;
    writeln!(stdin, "{}", init_req).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let init_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(init_resp["id"], 1);

    // 2. Initialized notification
    let notif = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
    writeln!(stdin, "{}", notif).unwrap();
    stdin.flush().unwrap();

    // 3. Test trim
    let req_trim = r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"test_trim","arguments":{}}}"#;
    writeln!(stdin, "{}", req_trim).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let resp_trim: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(resp_trim["id"], 2);
    assert_eq!(resp_trim["result"]["isError"], false);
    let trim_text = resp_trim["result"]["content"][0]["text"].as_str().unwrap();
    assert_eq!(trim_text, "padded text");

    // 4. Test slice head
    let req_head = r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"test_slice_head","arguments":{}}}"#;
    writeln!(stdin, "{}", req_head).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let resp_head: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(resp_head["id"], 3);
    assert_eq!(resp_head["result"]["isError"], false);
    let head_text = resp_head["result"]["content"][0]["text"].as_str().unwrap();
    assert_eq!(head_text, "line1\nline2");

    // 5. Test slice tail
    let req_tail = r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"test_slice_tail","arguments":{}}}"#;
    writeln!(stdin, "{}", req_tail).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let resp_tail: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(resp_tail["id"], 4);
    assert_eq!(resp_tail["result"]["isError"], false);
    let tail_text = resp_tail["result"]["content"][0]["text"].as_str().unwrap();
    assert_eq!(tail_text, "line3\nline4");

    // 6. Test filter-not
    let req_filter = r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"test_filter_not","arguments":{}}}"#;
    writeln!(stdin, "{}", req_filter).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let resp_filter: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(resp_filter["id"], 5);
    assert_eq!(resp_filter["result"]["isError"], false);
    let filter_text = resp_filter["result"]["content"][0]["text"].as_str().unwrap();
    assert_eq!(filter_text, "INFO: ready\nINFO: done");

    // 7. Test extract-json
    let req_extract = r#"{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"test_extract_json","arguments":{}}}"#;
    writeln!(stdin, "{}", req_extract).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let resp_extract: serde_json::Value = serde_json::from_str(&line).unwrap();
    eprintln!("resp_extract: {:?}", resp_extract);
    assert_eq!(resp_extract["id"], 6);
    assert_eq!(resp_extract["result"]["isError"], false);
    let extract_text = resp_extract["result"]["content"][0]["text"].as_str().unwrap();
    assert_eq!(extract_text, "dev@mcp.org");

    // 8. Test regex template
    let req_reg = r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"test_regex_template","arguments":{}}}"#;
    writeln!(stdin, "{}", req_reg).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let resp_reg: serde_json::Value = serde_json::from_str(&line).unwrap();
    eprintln!("resp_reg: {:?}", resp_reg);
    assert_eq!(resp_reg["id"], 7);
    assert_eq!(resp_reg["result"]["isError"], false);
    let reg_text = resp_reg["result"]["content"][0]["text"].as_str().unwrap();
    assert_eq!(reg_text, "Release 'release/v2.5' at hash '7a8f9b'");

    // 9. Clean shutdown
    drop(stdin);
    let status = child.wait().expect("Child failed to exit");
    assert!(status.success());
}

#[test]
fn test_cli_e2e_build_and_run_ws_and_pipe_server() {
    use std::net::TcpListener;

    // 1. Mock WebSocket Server
    let ws_listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind mock WS");
    let ws_port = ws_listener.local_addr().unwrap().port();

    let ws_handle = std::thread::spawn(move || {
        let (mut stream, _) = ws_listener.accept().expect("WS accept failed");
        let mut buf = [0u8; 1024];
        let n = stream.read(&mut buf).unwrap();
        let req_str = String::from_utf8_lossy(&buf[..n]);
        assert!(req_str.contains("Upgrade: websocket"));

        // Send 101 response
        let resp = "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n\r\n";
        stream.write_all(resp.as_bytes()).unwrap();
        stream.flush().unwrap();

        // Read WebSocket text frame from client
        let mut hdr = [0u8; 2];
        stream.read_exact(&mut hdr).unwrap();
        let len = (hdr[1] & 0x7F) as usize;
        let mut mask = [0u8; 4];
        stream.read_exact(&mut mask).unwrap();
        let mut payload = vec![0u8; len];
        stream.read_exact(&mut payload).unwrap();
        for (i, b) in payload.iter_mut().enumerate() {
            *b ^= mask[i % 4];
        }
        let msg = String::from_utf8_lossy(&payload);
        assert!(msg.contains("Runtime.evaluate"));

        // Send unmasked server-to-client text frame: {"result":{"value":"WS_SUCCESS"}}
        let reply_json = r#"{"result":{"value":"WS_SUCCESS"}}"#;
        let reply_bytes = reply_json.as_bytes();
        let mut frame = vec![0x81, reply_bytes.len() as u8];
        frame.extend_from_slice(reply_bytes);
        stream.write_all(&frame).unwrap();
        stream.flush().unwrap();
    });

    // 2. Mock Raw TCP Pipe Server
    let pipe_listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind mock Pipe");
    let pipe_port = pipe_listener.local_addr().unwrap().port();

    let pipe_handle = std::thread::spawn(move || {
        let (mut stream, _) = pipe_listener.accept().expect("Pipe accept failed");
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        assert_eq!(line.trim(), "PING hello");

        stream.write_all(b"PONG hello\r\n").unwrap();
        stream.flush().unwrap();
    });

    // 3. Define KDL with ws and pipe bindings
    let test_kdl = format!(
        r#"
server name="test-network-server" version="1.0.0" {{
    description "E2E Network bindings server"
}}

env {{
    WS_PORT default="{ws_port}" description="WebSocket Port"
    PIPE_PORT default="{pipe_port}" description="Pipe Port"
}}

tool "eval_ws" description="Evaluate via WebSocket" {{
    (string)param "expr" required=#true description="Expression"

    bind:ws url="ws://127.0.0.1:{{env:WS_PORT}}/devtools/browser" {{
        message "{{\"id\":1,\"method\":\"Runtime.evaluate\",\"params\":{{\"expression\":\"{{expr}}\"}}}}"
        timeout-ms 5000
        extract-json "/result/value"
    }}
}}

tool "redis_ping" description="Send Redis ping" {{
    (string)param "arg" default="hello" description="Ping argument"

    bind:pipe host="127.0.0.1" port=pipe_port_placeholder {{
        message "PING {{arg}}\r\n"
        framing "\r\n"
        timeout-ms 5000
    }}
}}
"#
    ).replace("pipe_port_placeholder", &pipe_port.to_string());

    let temp = tempdir().expect("Failed to create tempdir");
    let mut binary_path = temp.path().join("test_network_server");
    if cfg!(windows) {
        binary_path.set_extension("exe");
    }

    let build_res = mcp_builder_core::Compiler::build(&test_kdl, &binary_path, false);
    assert!(
        build_res.is_ok(),
        "Failed to compile network server: {:?}",
        build_res.err()
    );
    assert!(binary_path.exists(), "Binary was not created");

    let mut child = Command::new(&binary_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn compiled server binary");

    let mut stdin = child.stdin.take().expect("Failed to open stdin");
    let stdout = child.stdout.take().expect("Failed to open stdout");
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();

    // 1. Initialize
    let init_req = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test-client","version":"1.0.0"}}}"#;
    writeln!(stdin, "{}", init_req).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let init_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(init_resp["id"], 1);

    let notif = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
    writeln!(stdin, "{}", notif).unwrap();
    stdin.flush().unwrap();

    // 2. Call eval_ws
    let ws_call = r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"eval_ws","arguments":{"expr":"1+1"}}}"#;
    writeln!(stdin, "{}", ws_call).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let ws_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(ws_resp["id"], 2);
    assert_eq!(ws_resp["result"]["isError"], false);
    assert_eq!(ws_resp["result"]["content"][0]["text"], "WS_SUCCESS");

    // 3. Call redis_ping
    let pipe_call = r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"redis_ping","arguments":{"arg":"hello"}}}"#;
    writeln!(stdin, "{}", pipe_call).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let pipe_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(pipe_resp["id"], 3);
    assert_eq!(pipe_resp["result"]["isError"], false);
    assert_eq!(pipe_resp["result"]["content"][0]["text"], "PONG hello\r\n");

    // 4. Clean shutdown
    drop(stdin);
    let status = child.wait().expect("Child failed to exit");
    assert!(status.success());

    ws_handle.join().unwrap();
    pipe_handle.join().unwrap();
}

#[test]
fn test_cli_e2e_build_and_run_profiles_server() {
    let pipe_listener = std::net::TcpListener::bind("127.0.0.1:0").expect("Failed to bind mock pipe");
    let pipe_port = pipe_listener.local_addr().unwrap().port();

    let pipe_handle = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = pipe_listener.accept() {
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            if reader.read_line(&mut line).is_ok() {
                // Redis RESP or plain command echo
                let trimmed = line.trim();
                let resp = format!("+\"PONG:{}\"\r\n", trimmed);
                let _ = stream.write_all(resp.as_bytes());
                let _ = stream.flush();
            }
        }
    });

    let exec_cmd = if cfg!(windows) { "cmd" } else { "sh" };
    let exec_arg0 = if cfg!(windows) { "/c" } else { "-c" };
    let exec_arg1 = if cfg!(windows) { "echo PROFILED_EXEC_{val}" } else { "echo PROFILED_EXEC_{val}" };

    let test_kdl = format!(
        r#"
server name="profiled-server" version="2.0.0" {{
    description "E2E Profiled Server"
}}

profile "custom-cmd" {{
    description "Custom exec profile"
    exec "{exec_cmd}" {{
        args "{exec_arg0}" "{exec_arg1}"
    }}
}}

tool "run_custom" description="Run tool using custom user profile" {{
    param "val" type="string" description="Value to echo"
    profile "custom-cmd"
}}

tool "redis_get" description="Execute redis get via redis-tcp profile" {{
    param "key" type="string" description="Redis key"
    profile "redis-tcp" {{
        host "127.0.0.1"
        port "{pipe_port}"
        command "GET"
        args "$key"
    }}
}}
"#,
        exec_cmd = exec_cmd,
        exec_arg0 = exec_arg0,
        exec_arg1 = exec_arg1,
        pipe_port = pipe_port,
    );

    let temp = tempdir().expect("Failed to create tempdir");
    let mut binary_path = temp.path().join("profiled_server");
    if cfg!(windows) {
        binary_path.set_extension("exe");
    }

    let build_res = mcp_builder_core::Compiler::build(&test_kdl, &binary_path, false);
    assert!(
        build_res.is_ok(),
        "Failed to compile profiled server: {:?}",
        build_res.err()
    );

    let mut child = Command::new(&binary_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn profiled server");

    let mut stdin = child.stdin.take().expect("Failed to open stdin");
    let stdout = child.stdout.take().expect("Failed to open stdout");
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();

    // 1. Initialize
    let init_req = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test-client","version":"1.0.0"}}}"#;
    writeln!(stdin, "{}", init_req).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let init_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(init_resp["id"], 1);
    assert_eq!(init_resp["result"]["serverInfo"]["name"], "profiled-server");

    let notif = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
    writeln!(stdin, "{}", notif).unwrap();
    stdin.flush().unwrap();

    // 2. Call run_custom
    let custom_call = r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"run_custom","arguments":{"val":"42"}}}"#;
    writeln!(stdin, "{}", custom_call).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let custom_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(custom_resp["id"], 2);
    assert_eq!(custom_resp["result"]["isError"], false);
    assert!(custom_resp["result"]["content"][0]["text"].as_str().unwrap().contains("PROFILED_EXEC_42"));

    // 3. Call redis_get
    let redis_call = r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"redis_get","arguments":{"key":"mykey"}}}"#;
    writeln!(stdin, "{}", redis_call).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let redis_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(redis_resp["id"], 3);
    assert_eq!(redis_resp["result"]["isError"], false);
    assert!(redis_resp["result"]["content"][0]["text"].as_str().unwrap().contains("PONG:GET mykey"));

    drop(stdin);
    let status = child.wait().expect("Child failed to exit");
    assert!(status.success());
    pipe_handle.join().unwrap();
}

#[test]
fn test_cli_e2e_conditional_os_exec_server() {
    let test_kdl = r#"
server name="crossplatform-os-server" version="1.0.0" {
    description "E2E Cross-Platform OS Conditional Process Execution Server"
}

tool "cross_tool" description="Execute cross-platform nested commands" {
    param "message" type="string" description="Message to echo"
    bind:exec {
        when:windows "cmd" {
            args "/c" "echo WIN_EXEC_{message}"
        }
        when:unix "sh" {
            args "-c" "echo UNIX_EXEC_{message}"
        }
        fallback "echo" {
            args "FALLBACK_{message}"
        }
    }
}

tool "direct_tool" description="Execute cross-platform direct node bindings" {
    param "message" type="string" description="Message to echo"
    exec:windows "cmd" {
        args "/c" "echo WIN_DIRECT_{message}"
    }
    exec:unix "sh" {
        args "-c" "echo UNIX_DIRECT_{message}"
    }
    fallback "echo" {
        args "FALLBACK_{message}"
    }
}

resource "system://os-banner" {
    name "OS Banner"
    mime-type "text/plain"
    exec:windows "cmd" {
        args "/c" "echo WIN_RESOURCE_BANNER"
    }
    exec:unix "sh" {
        args "-c" "echo UNIX_RESOURCE_BANNER"
    }
}

resource-template "system://echo/{text}" {
    name "Echo Template"
    mime-type "text/plain"
    param "text" type="string"
    exec {
        when:windows "cmd" {
            args "/c" "echo WIN_TMPL_{text}"
        }
        when:unix "sh" {
            args "-c" "echo UNIX_TMPL_{text}"
        }
    }
}
"#;

    let temp = tempdir().expect("Failed to create tempdir");
    let mut binary_path = temp.path().join("crossplatform_server");
    if cfg!(windows) {
        binary_path.set_extension("exe");
    }

    let build_res = mcp_builder_core::Compiler::build(test_kdl, &binary_path, false);
    assert!(
        build_res.is_ok(),
        "Failed to compile cross-platform server: {:?}",
        build_res.err()
    );

    let mut child = Command::new(&binary_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn cross-platform server");

    let mut stdin = child.stdin.take().expect("Failed to open stdin");
    let stdout = child.stdout.take().expect("Failed to open stdout");
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();

    // 1. Initialize
    let init_req = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test-client","version":"1.0.0"}}}"#;
    writeln!(stdin, "{}", init_req).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let init_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(init_resp["id"], 1);
    assert_eq!(init_resp["result"]["serverInfo"]["name"], "crossplatform-os-server");

    let notif = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
    writeln!(stdin, "{}", notif).unwrap();
    stdin.flush().unwrap();

    // 2. Call cross_tool
    let cross_call = r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"cross_tool","arguments":{"message":"HELLO_WORLD"}}}"#;
    writeln!(stdin, "{}", cross_call).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let cross_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(cross_resp["id"], 2);
    assert_eq!(cross_resp["result"]["isError"], false);
    let text = cross_resp["result"]["content"][0]["text"].as_str().unwrap();
    if cfg!(windows) {
        assert!(text.contains("WIN_EXEC_HELLO_WORLD"), "Expected WIN_EXEC_HELLO_WORLD, got: {}", text);
    } else {
        assert!(text.contains("UNIX_EXEC_HELLO_WORLD"), "Expected UNIX_EXEC_HELLO_WORLD, got: {}", text);
    }

    // 3. Call direct_tool
    let direct_call = r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"direct_tool","arguments":{"message":"FOOBAR"}}}"#;
    writeln!(stdin, "{}", direct_call).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let direct_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(direct_resp["id"], 3);
    assert_eq!(direct_resp["result"]["isError"], false);
    let dtext = direct_resp["result"]["content"][0]["text"].as_str().unwrap();
    if cfg!(windows) {
        assert!(dtext.contains("WIN_DIRECT_FOOBAR"), "Expected WIN_DIRECT_FOOBAR, got: {}", dtext);
    } else {
        assert!(dtext.contains("UNIX_DIRECT_FOOBAR"), "Expected UNIX_DIRECT_FOOBAR, got: {}", dtext);
    }

    // 4. Read static resource
    let res_req = r#"{"jsonrpc":"2.0","id":4,"method":"resources/read","params":{"uri":"system://os-banner"}}"#;
    writeln!(stdin, "{}", res_req).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let res_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(res_resp["id"], 4);
    let rtext = res_resp["result"]["contents"][0]["text"].as_str().unwrap();
    if cfg!(windows) {
        assert!(rtext.contains("WIN_RESOURCE_BANNER"), "Expected WIN_RESOURCE_BANNER, got: {}", rtext);
    } else {
        assert!(rtext.contains("UNIX_RESOURCE_BANNER"), "Expected UNIX_RESOURCE_BANNER, got: {}", rtext);
    }

    // 5. Read dynamic resource template
    let tmpl_req = r#"{"jsonrpc":"2.0","id":5,"method":"resources/read","params":{"uri":"system://echo/DYNAMIC_VAL"}}"#;
    writeln!(stdin, "{}", tmpl_req).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let tmpl_resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(tmpl_resp["id"], 5);
    let ttext = tmpl_resp["result"]["contents"][0]["text"].as_str().unwrap();
    if cfg!(windows) {
        assert!(ttext.contains("WIN_TMPL_DYNAMIC_VAL"), "Expected WIN_TMPL_DYNAMIC_VAL, got: {}", ttext);
    } else {
        assert!(ttext.contains("UNIX_TMPL_DYNAMIC_VAL"), "Expected UNIX_TMPL_DYNAMIC_VAL, got: {}", ttext);
    }

    drop(stdin);
    let status = child.wait().expect("Child failed to exit");
    assert!(status.success());
}

#[test]
fn test_cli_e2e_template_modifiers_json_and_url() {
    let test_kdl = r#"
server name="test-modifiers-server" version="1.0.0" {
    description "Test server for template modifiers"
}

env {
    API_TAG default="v1 release"
}

tool "test_json_escaped" description="Echo JSON escaped payload" {
    param "code" type="string" required=#true
    param "user" type="string" required=#true
    param "meta" type="object"
    param "query" type="string"
    exec {
        when:windows "powershell" {
            args "-NoProfile" "-Command" "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; Write-Output ('RESULT:' + @'
{\"script\": {code:json}, \"author\": {user:json}, \"meta\": {meta:json}, \"env\": {env:API_TAG:json}, \"url\": \"https://api.com?q={query:url}\"}
'@)"
        }
        when:unix "sh" {
            args "-c" "printf '%s\n' 'RESULT:{\"script\": {code:json}, \"author\": {user:json}, \"meta\": {meta:json}, \"env\": {env:API_TAG:json}, \"url\": \"https://api.com?q={query:url}\"}'"
        }
    }
}
"#;

    let temp = tempdir().expect("Failed to create tempdir");
    let mut binary_path = temp.path().join("test_server_mods");
    if cfg!(windows) {
        binary_path.set_extension("exe");
    }

    let build_res = mcp_builder_core::Compiler::build(test_kdl, &binary_path, false);
    assert!(
        build_res.is_ok(),
        "Failed to compile test server: {:?}",
        build_res.err()
    );
    assert!(binary_path.exists(), "Binary was not created");

    let mut child = Command::new(&binary_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn compiled server binary");

    let mut stdin = child.stdin.take().expect("Failed to open stdin");
    let stdout = child.stdout.take().expect("Failed to open stdout");
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();

    // 1. Initialize
    let init_req = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test-client","version":"1.0.0"}}}"#;
    writeln!(stdin, "{}", init_req).unwrap();
    stdin.flush().unwrap();
    reader.read_line(&mut line).unwrap();

    // 2. Call tool with multiline string containing quotes and special chars
    let multiline_code = "function test() {\n    let msg = \"hello \\\"world\\\"\";\n    return msg;\n}";
    let user_name = "Alice & Bob <Admin> \"Special\"";
    let meta_obj = serde_json::json!({
        "count": 42,
        "items": ["a", "b"]
    });
    let query_str = "hello world & tag=test";

    let tool_call_json = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "test_json_escaped",
            "arguments": {
                "code": multiline_code,
                "user": user_name,
                "meta": meta_obj,
                "query": query_str
            }
        }
    });

    writeln!(stdin, "{}", serde_json::to_string(&tool_call_json).unwrap()).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();

    let tool_resp: serde_json::Value = serde_json::from_str(&line).expect("Failed to parse tool response");
    assert_eq!(tool_resp["id"], 2);
    assert_eq!(tool_resp["result"]["isError"], false);
    let text = tool_resp["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("RESULT:"), "Expected 'RESULT:' prefix in: {}", text);

    let mut json_part = text.split("RESULT:").nth(1).unwrap().trim();
    if json_part.ends_with('"') {
        json_part = &json_part[..json_part.len() - 1];
    }
    let parsed: serde_json::Value = serde_json::from_str(json_part).expect("Failed to parse serialized JSON result");
    assert_eq!(parsed["script"], multiline_code);
    assert_eq!(parsed["author"], user_name);
    assert_eq!(parsed["meta"]["count"], 42);
    assert_eq!(parsed["meta"]["items"], serde_json::json!(["a", "b"]));
    assert_eq!(parsed["env"], "v1 release");
    assert_eq!(parsed["url"], "https://api.com?q=hello%20world%20%26%20tag%3Dtest");

    drop(stdin);
    let status = child.wait().expect("Child failed to exit");
    assert!(status.success());
}
