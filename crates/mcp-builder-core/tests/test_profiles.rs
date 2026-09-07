use mcp_builder_core::{
    CodeGenerator, Compiler, TargetOs, ToolBinding,
};

#[test]
fn test_builtin_profiles_resolution() {
    let kdl = r#"
        server name="profile-test-server" version="1.0.0" {
            description "Profile-driven MCP server"
        }

        tool "eval_js" description="Evaluate JS expression in Chrome" {
            param "expr" type="string" description="JS expression"
            profile "cdp-chrome" {
                method "Runtime.evaluate"
                params {
                    expression "$expr"
                }
            }
        }

        tool "get_ea" description="Get IDA screen address" {
            profile "ida-pro" {
                method "get_screen_ea"
            }
        }

        tool "get_redis_key" description="Get value from Redis" {
            param "key" type="string" description="Redis key"
            profile "redis-tcp" {
                command "GET"
                args "$key"
            }
        }

        tool "fetch_user" description="Fetch user by ID" {
            param "id" type="string" description="User ID"
            profile "rest-json" {
                url "https://api.example.com/users/{id}"
            }
        }
    "#;

    let spec = Compiler::check(kdl).expect("Compiler check failed for builtin profiles");
    assert_eq!(spec.tools.len(), 4);

    // Tool 0: cdp-chrome -> WsBinding
    match &spec.tools[0].binding {
        Some(ToolBinding::Ws(ws)) => {
            assert_eq!(ws.url.as_deref(), Some("ws://127.0.0.1:9222/devtools/browser"));
            assert!(ws.message.contains(r#""method": "Runtime.evaluate""#));
            assert!(ws.message.contains(r#""expression": "$expr""#));
            assert_eq!(ws.extract_json.as_deref(), Some("$.result"));
        }
        other => panic!("Expected ToolBinding::Ws for cdp-chrome, got {:?}", other),
    }

    // Tool 1: ida-pro -> PipeBinding
    match &spec.tools[1].binding {
        Some(ToolBinding::Pipe(pipe)) => {
            assert_eq!(pipe.host.as_deref(), Some("127.0.0.1"));
            assert_eq!(pipe.port.as_deref(), Some("8888"));
            assert_eq!(pipe.framing, "\n");
            assert!(pipe.message.contains(r#""method": "get_screen_ea""#));
            assert_eq!(pipe.extract_json.as_deref(), Some("$.result"));
        }
        other => panic!("Expected ToolBinding::Pipe for ida-pro, got {:?}", other),
    }

    // Tool 2: redis-tcp -> PipeBinding
    match &spec.tools[2].binding {
        Some(ToolBinding::Pipe(pipe)) => {
            assert_eq!(pipe.host.as_deref(), Some("127.0.0.1"));
            assert_eq!(pipe.port.as_deref(), Some("6379"));
            assert_eq!(pipe.framing, "\r\n");
            assert_eq!(pipe.message, "GET $key\r\n");
        }
        other => panic!("Expected ToolBinding::Pipe for redis-tcp, got {:?}", other),
    }

    // Tool 3: rest-json -> HttpBinding
    match &spec.tools[3].binding {
        Some(ToolBinding::Http(http)) => {
            assert_eq!(http.url, "https://api.example.com/users/{id}");
            assert_eq!(http.method, "GET");
            assert!(http.headers.iter().any(|(k, v)| k == "Content-Type" && v == "application/json"));
            assert!(http.headers.iter().any(|(k, v)| k == "Accept" && v == "application/json"));
        }
        other => panic!("Expected ToolBinding::Http for rest-json, got {:?}", other),
    }

    let gen = CodeGenerator::generate_rust_project(&spec).expect("Codegen failed");
    assert!(gen.main_rs.contains("execute_ws_request"));
    assert!(gen.main_rs.contains("execute_pipe_request"));
    assert!(gen.main_rs.contains("execute_http_request"));
}

#[test]
fn test_user_defined_profile_and_inheritance() {
    let kdl = r#"
        server name="custom-profiles-srv" version="1.0.0" {
            profile "auth-api"
        }

        profile "base-api" {
            description "Base REST endpoint"
            http {
                url "https://api.internal.corp"
                headers {
                    "X-Client" "mcp-server"
                }
            }
        }

        profile "auth-api" extends="base-api" {
            description "Authenticated REST endpoint"
            http {
                headers {
                    "Authorization" "Bearer $API_TOKEN"
                }
            }
        }

        env {
            API_TOKEN required=#true description="Service token"
        }

        tool "get_profile" description="Fetch user profile" {
            param "user_id" type="string"
            profile "auth-api" {
                url "https://api.internal.corp/users/{user_id}"
            }
        }

        tool "default_ping" description="Ping service using default server profile" {
            profile "auth-api" {
                url "https://api.internal.corp/ping"
            }
        }
    "#;

    let spec = Compiler::check(kdl).expect("Compiler check failed for custom profiles");
    assert_eq!(spec.profiles.len(), 2);
    assert_eq!(spec.tools.len(), 2);

    let t0 = &spec.tools[0];
    match &t0.binding {
        Some(ToolBinding::Http(http)) => {
            assert_eq!(http.url, "https://api.internal.corp/users/{user_id}");
            assert!(http.headers.iter().any(|(k, v)| k == "X-Client" && v == "mcp-server"));
            assert!(http.headers.iter().any(|(k, v)| k == "Authorization" && v == "Bearer $API_TOKEN"));
        }
        other => panic!("Expected ToolBinding::Http, got {:?}", other),
    }
}

#[test]
fn test_profile_diagnostics() {
    let kdl_typo = r#"
        server name="test-err" version="1.0.0"
        tool "bad_profile" {
            profile "cdp-chrme" {
                method "Runtime.evaluate"
            }
        }
    "#;
    let (spec1, report1) = Compiler::lint(kdl_typo, Some("test.kdl"));
    assert!(spec1.is_none());
    assert!(report1.has_errors());
    let diag1 = report1.diagnostics.iter().find(|d| d.code == "E0065").unwrap();
    assert_eq!(diag1.note.as_deref(), Some("Did you mean 'cdp-chrome'?"));

    let kdl_cycle = r#"
        server name="test-cycle" version="1.0.0"
        profile "p1" extends="p2" {
            pipe {
                host "127.0.0.1"
            }
        }
        profile "p2" extends="p1" {
            pipe {
                port "8080"
            }
        }
        tool "cyclic_tool" {
            profile "p1"
        }
    "#;
    let (spec2, report2) = Compiler::lint(kdl_cycle, Some("test.kdl"));
    assert!(spec2.is_none());
    assert!(report2.has_errors());
    assert!(report2.diagnostics.iter().any(|d| d.code == "E0066"));

    let kdl_conflict = r#"
        server name="test-conflict" version="1.0.0"
        tool "conflicting_tool" {
            profile "rest-json"
            bind:exec "echo" {
                args "hello"
            }
        }
    "#;
    let (spec3, report3) = Compiler::lint(kdl_conflict, Some("test.kdl"));
    assert!(spec3.is_none());
    assert!(report3.has_errors());
    assert!(report3.diagnostics.iter().any(|d| d.code == "E0054"));
}

#[test]
fn test_conditional_os_exec_profile_inheritance() {
    let kdl_source = r#"
        server name="test-prof-inherit" version="1.0.0"

        profile "base-cross" {
            exec:windows "cmd" {
                args "/c" "echo BASE_WIN"
            }
            exec:unix "sh" {
                args "-c" "echo BASE_UNIX"
            }
        }

        profile "derived-cross" extends="base-cross" {
            exec:windows "powershell" {
                args "-Command" "Write-Output DERIVED_WIN"
            }
        }

        tool "inherited_tool" {
            profile "derived-cross"
        }
    "#;

    let (spec, report) = Compiler::lint(kdl_source, Some("test.kdl"));
    assert!(spec.is_some(), "Got diagnostics:\n{}", report.render_plain());
    assert!(!report.has_errors());

    let server = spec.unwrap();
    let tool = &server.tools[0];
    if let Some(ToolBinding::Exec(e)) = &tool.binding {
        assert_eq!(e.variants.len(), 2);
        let win_var = e.variants.iter().find(|v| v.os == TargetOs::Windows).unwrap();
        assert_eq!(win_var.command, "powershell");
        assert_eq!(win_var.args, vec!["-Command", "Write-Output DERIVED_WIN"]);

        let unix_var = e.variants.iter().find(|v| v.os == TargetOs::Unix).unwrap();
        assert_eq!(unix_var.command, "sh");
        assert_eq!(unix_var.args, vec!["-c", "echo BASE_UNIX"]);
    } else {
        panic!("Expected ExecBinding");
    }
}
