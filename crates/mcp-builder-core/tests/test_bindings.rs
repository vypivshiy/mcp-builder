use mcp_builder_core::{
    CodeGenerator, Compiler, Parser, TargetOs, ToolBinding,
};

const SAMPLE_KDL: &str = r#"
    server name="sys-mcp" version="1.0.0" {
        description "Native system orchestration and developer telemetry MCP server."
        author "Engineering Core"
        license "MIT"
    }

    transports {
        stdio enabled=#true
        sse enabled=#false
    }

    env {
        API_KEY required=#true description="Secret API key"
        WORKSPACE_DIR default="." description="Working directory"
    }

    tool "git_log" description="Extracts formatted git commit history." {
        param "repo_path" (string)type="string" required=#true description="Absolute filesystem path."
        param "max_count" (integer)type="integer" default=10 minimum=1 maximum=100 description="Max commits."
        
        bind:exec "git" {
            workdir "{repo_path}"
            args "-C" "{repo_path}" "log" "-n" "{max_count}" "--oneline"
            timeout-ms 5000
        }
    }

    tool "calc" description="Compute expression" {
        param "expr" type="string" required=#true
        exec "echo" {
            args "{expr}"
        }
    }

    resource "system://info" name="System Info" {
        description "Static machine info"
        mime-type "application/json"
        text "{\"os\": \"windows\"}"
    }

    prompt "review" description="Code review" {
        argument "diff" required=#true description="Git diff"
        message role="user" "Please review: {diff}"
    }
"#;

const HTTP_KDL: &str = r#"
    server name="weather-mcp" version="1.0.0" {
        description "Weather API server"
    }

    tool "fetch_weather" description="Get weather" {
        param "city" (string)type="string" required=#true
        param "units" (string)type="string" default="metric"
        
        bind:http method="GET" url="https://api.open-meteo.com/v1/forecast" {
            query "city" "{city}"
            query "units" "{units}"
            timeout-ms 3000
        }
    }
"#;

#[test]
fn test_compiler_emit_and_check_exec() {
    let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
    let project_path = temp_dir.path();
    Compiler::emit(SAMPLE_KDL, project_path).expect("Emit failed");

    assert!(project_path.join("Cargo.toml").exists());
    assert!(project_path.join("src").join("main.rs").exists());

    let output = std::process::Command::new("cargo")
        .arg("check")
        .current_dir(project_path)
        .output()
        .expect("Failed to run cargo check on generated server");

    assert!(
        output.status.success(),
        "Generated exec project failed cargo check:\nSTDOUT:\n{}\nSTDERR:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_compiler_emit_and_check_http() {
    let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
    let project_path = temp_dir.path();
    Compiler::emit(HTTP_KDL, project_path).expect("Emit failed");

    assert!(project_path.join("Cargo.toml").exists());
    assert!(project_path.join("src").join("main.rs").exists());

    let output = std::process::Command::new("cargo")
        .arg("check")
        .current_dir(project_path)
        .output()
        .expect("Failed to run cargo check on generated HTTP server");

    assert!(
        output.status.success(),
        "Generated http project failed cargo check:\nSTDOUT:\n{}\nSTDERR:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_com_binding_parsing_and_codegen() {
    let kdl = r#"
        server name="test-com" version="1.0.0" {
            description "COM automation server"
        }

        tool "excel_read" description="Read cell from Excel" {
            param "cell" (string)type="string" default="A1"
            bind:com "Excel.Application" {
                attach #true
                bring-to-front #false
                dispatch "ActiveSheet.Range"
                args "{cell}"
                timeout-ms 5000
            }
        }

        tool "ida_get_ea" description="Get screen EA from IDA Pro" {
            bind:com progid="IDA.Application" {
                call "GetScreenEA"
                attach #true
            }
        }
    "#;
    let spec = Parser::parse(kdl).expect("Failed to parse COM binding KDL");
    assert_eq!(spec.tools.len(), 2);

    let t1 = &spec.tools[0];
    assert_eq!(t1.name, "excel_read");
    match &t1.binding {
        Some(ToolBinding::Com(c)) => {
            assert_eq!(c.progid, "Excel.Application");
            assert_eq!(c.method, "ActiveSheet.Range");
            assert_eq!(c.args, vec!["{cell}"]);
            assert!(c.attach);
            assert!(!c.bring_to_front);
            assert_eq!(c.timeout_ms, 5000);
        }
        other => panic!("Expected COM binding, got {:?}", other),
    }

    let t2 = &spec.tools[1];
    assert_eq!(t2.name, "ida_get_ea");
    match &t2.binding {
        Some(ToolBinding::Com(c)) => {
            assert_eq!(c.progid, "IDA.Application");
            assert_eq!(c.method, "GetScreenEA");
            assert!(c.attach);
        }
        other => panic!("Expected COM binding, got {:?}", other),
    }

    let gen = CodeGenerator::generate_rust_project(&spec).expect("Codegen failed");
    assert!(gen.main_rs.contains("execute_com_request"));
    assert!(gen.main_rs.contains("com_native"));
    assert!(gen.main_rs.contains("CLSIDFromProgID"));
    assert!(gen.main_rs.contains("IDispatchVtbl"));
    assert!(gen.main_rs.contains("DISPPARAMS"));
    assert!(gen.main_rs.contains("VariantClear"));
    assert!(gen.main_rs.contains("SysAllocStringLen"));
}

#[test]
fn test_ws_and_pipe_binding_parsing_and_codegen() {
    let kdl = r#"
        server name="test-network-bindings" version="1.0.0" {
            description "Test server for WebSocket and Pipe bindings"
        }

        env {
            DAEMON_HOST default="127.0.0.1" description="Host address"
            DAEMON_PORT default="9999" description="Daemon port"
        }

        tool "query_ws" description="Query daemon over WebSocket" {
            (string)param "target_id" required=#true description="Target ID"
            (string)param "query" default="status" description="Query command"

            bind:ws url="ws://{env:DAEMON_HOST}:{env:DAEMON_PORT}/ws/{target_id}" {
                header "Authorization" "Bearer test-token"
                message "{\"cmd\":\"{query}\"}"
                timeout-ms 6000
                extract-json "/result/data"
            }
        }

        tool "redis_command" description="Send raw TCP command" {
            (string)param "cmd" required=#true description="Redis command line"

            bind:pipe host="127.0.0.1" port=6379 {
                message "{cmd}\r\n"
                framing "\r\n"
                timeout-ms 3000
            }
        }
    "#;

    let spec = Parser::parse(kdl).expect("Failed to parse WS & Pipe KDL");
    assert_eq!(spec.tools.len(), 2);

    let t_ws = &spec.tools[0];
    assert_eq!(t_ws.name, "query_ws");
    match &t_ws.binding {
        Some(ToolBinding::Ws(ws)) => {
            assert_eq!(ws.url.as_deref(), Some("ws://{env:DAEMON_HOST}:{env:DAEMON_PORT}/ws/{target_id}"));
            assert_eq!(ws.headers.len(), 1);
            assert_eq!(ws.headers[0], ("Authorization".to_string(), "Bearer test-token".to_string()));
            assert_eq!(ws.message, "{\"cmd\":\"{query}\"}");
            assert_eq!(ws.timeout_ms, 6000);
            assert_eq!(ws.extract_json.as_deref(), Some("/result/data"));
        }
        other => panic!("Expected ToolBinding::Ws, got {:?}", other),
    }

    let t_pipe = &spec.tools[1];
    assert_eq!(t_pipe.name, "redis_command");
    match &t_pipe.binding {
        Some(ToolBinding::Pipe(pipe)) => {
            assert_eq!(pipe.host.as_deref(), Some("127.0.0.1"));
            assert_eq!(pipe.port.as_deref(), Some("6379"));
            assert_eq!(pipe.message, "{cmd}\r\n");
            assert_eq!(pipe.framing, "\r\n");
            assert_eq!(pipe.timeout_ms, 3000);
        }
        other => panic!("Expected ToolBinding::Pipe, got {:?}", other),
    }

    let gen = CodeGenerator::generate_rust_project(&spec).expect("Codegen failed");
    assert!(gen.main_rs.contains("pub fn execute_ws_request"));
    assert!(gen.main_rs.contains("pub fn execute_pipe_request"));
    assert!(gen.main_rs.contains("execute_ws_request(opts)"));
    assert!(gen.main_rs.contains("execute_pipe_request(opts)"));
    assert!(gen.main_rs.contains("WsOptions"));
    assert!(gen.main_rs.contains("PipeOptions"));
}

#[test]
fn test_conditional_os_exec_parsing_and_codegen() {
    let kdl_source = r#"
        server name="crossplatform-exec-test" version="1.0.0" {
            description "Testing cross-platform OS conditional process execution"
        }

        tool "list-files" {
            description "List directory contents cross-platform"
            param "path" type="string" default="."
            bind:exec {
                when:windows "cmd" {
                    args "/c" "dir" "{path}"
                }
                when:linux "ls" {
                    args "-la" "{path}"
                }
                when:macos "ls" {
                    args "-la" "{path}"
                }
                fallback "dir" {
                    args "{path}"
                }
            }
        }

        tool "system-info" {
            description "Query system hardware info"
            exec:windows "cmd" {
                args "/c" "systeminfo"
            }
            exec:unix "uname" {
                args "-a"
            }
        }

        resource "system://crossplatform-info" {
            name "System Info"
            mime-type "text/plain"
            exec:windows "cmd" {
                args "/c" "ver"
            }
            exec:unix "uname" {
                args "-sr"
            }
        }

        resource-template "system://crossplatform-ping/{host}" {
            name "Crossplatform Ping"
            mime-type "text/plain"
            param "host" type="string"
            exec {
                when:windows "ping" {
                    args "-n" "1" "{host}"
                }
                when:unix "ping" {
                    args "-c" "1" "{host}"
                }
            }
        }
    "#;

    let (spec, report) = Compiler::lint(kdl_source, Some("crossplatform.kdl"));
    assert!(spec.is_some(), "Expected valid parse, got diagnostics:\n{}", report.render_plain());
    assert!(!report.has_errors());

    let server_spec = spec.unwrap();
    assert_eq!(server_spec.tools.len(), 2);

    // Check Tool 1: list-files
    let t1 = &server_spec.tools[0];
    assert_eq!(t1.name, "list-files");
    if let Some(ToolBinding::Exec(e)) = &t1.binding {
        assert_eq!(e.variants.len(), 4);
        assert_eq!(e.variants[0].os, TargetOs::Windows);
        assert_eq!(e.variants[0].command, "cmd");
        assert_eq!(e.variants[0].args, vec!["/c", "dir", "{path}"]);
        assert_eq!(e.variants[1].os, TargetOs::Linux);
        assert_eq!(e.variants[1].command, "ls");
        assert_eq!(e.variants[2].os, TargetOs::Macos);
        assert_eq!(e.variants[2].command, "ls");
        assert_eq!(e.variants[3].os, TargetOs::Fallback);
        assert_eq!(e.variants[3].command, "dir");
    } else {
        panic!("Expected ExecBinding on list-files");
    }

    // Check Tool 2: system-info
    let t2 = &server_spec.tools[1];
    assert_eq!(t2.name, "system-info");
    if let Some(ToolBinding::Exec(e)) = &t2.binding {
        assert_eq!(e.variants.len(), 2);
        assert_eq!(e.variants[0].os, TargetOs::Windows);
        assert_eq!(e.variants[0].command, "cmd");
        assert_eq!(e.variants[1].os, TargetOs::Unix);
        assert_eq!(e.variants[1].command, "uname");
    } else {
        panic!("Expected ExecBinding on system-info");
    }

    // Check Resource
    assert_eq!(server_spec.resources.len(), 1);
    let res = &server_spec.resources[0];
    if let Some(mcp_builder_core::ResourceContent::Exec(e)) = &res.content {
        assert_eq!(e.variants.len(), 2);
        assert_eq!(e.variants[0].os, TargetOs::Windows);
        assert_eq!(e.variants[0].command, "cmd");
        assert_eq!(e.variants[1].os, TargetOs::Unix);
        assert_eq!(e.variants[1].command, "uname");
    } else {
        panic!("Expected Exec resource content");
    }

    // Check Resource Template
    assert_eq!(server_spec.resource_templates.len(), 1);
    let tmpl = &server_spec.resource_templates[0];
    if let Some(ToolBinding::Exec(e)) = &tmpl.binding {
        assert_eq!(e.variants.len(), 2);
        assert_eq!(e.variants[0].os, TargetOs::Windows);
        assert_eq!(e.variants[0].command, "ping");
        assert_eq!(e.variants[1].os, TargetOs::Unix);
        assert_eq!(e.variants[1].command, "ping");
    } else {
        panic!("Expected Exec binding on resource template");
    }

    // Verify Code Generation
    let gen = CodeGenerator::generate_rust_project(&server_spec).expect("Code generator failed");
    assert!(gen.main_rs.contains("if cfg!(windows)"));
    assert!(gen.main_rs.contains("else if cfg!(target_os = \"linux\")"));
    assert!(gen.main_rs.contains("else if cfg!(target_os = \"macos\")"));
    assert!(gen.main_rs.contains("else if cfg!(unix)"));
}
