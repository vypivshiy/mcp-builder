use mcp_builder_core::Compiler;

#[test]
fn test_diagnostics_syntax_error() {
    let kdl = r#"
        server name="test-syntax" {
            unclosed quote "here
        }
    "#;
    let (spec, report) = Compiler::lint(kdl, Some("test.kdl"));
    assert!(spec.is_none());
    assert!(report.has_errors());
    assert_eq!(report.error_count(), 1);
    assert_eq!(report.diagnostics[0].code, "E0001");
    let rendered = report.render_plain();
    assert!(rendered.contains("error[E0001]"));
    assert!(rendered.contains("test.kdl:"));
}

#[test]
fn test_diagnostics_duplicate_tool_and_prompt() {
    let kdl = r#"
        server name="test-dups" version="1.0.0"
        tool "calc" {
            exec "echo" {
                args "1"
            }
        }
        tool "calc" {
            exec "echo" {
                args "2"
            }
        }
        prompt "review" {
            message role="user" "1"
        }
        prompt "review" {
            message role="user" "2"
        }
    "#;
    let (spec, report) = Compiler::lint(kdl, Some("test.kdl"));
    assert!(spec.is_none());
    assert!(report.has_errors());
    let codes: Vec<&str> = report.diagnostics.iter().map(|d| d.code.as_str()).collect();
    assert!(codes.contains(&"E0010"));
    assert!(codes.contains(&"E0011"));
}

#[test]
fn test_diagnostics_duplicate_param() {
    let kdl = r#"
        server name="test-dup-param" version="1.0.0"
        tool "fetch" {
            param "url" type="string"
            param "url" type="string"
            exec "curl" {
                args "{url}"
            }
        }
    "#;
    let (spec, report) = Compiler::lint(kdl, Some("test.kdl"));
    assert!(spec.is_none());
    assert!(report.has_errors());
    assert!(report.diagnostics.iter().any(|d| d.code == "E0013"));
}

#[test]
fn test_diagnostics_undeclared_param_with_typo_suggestion() {
    let kdl = r#"
        server name="test-typo" version="1.0.0"
        tool "greet" {
            param "username" type="string"
            exec "echo" {
                args "Hello {usernam}"
            }
        }
    "#;
    let (spec, report) = Compiler::lint(kdl, Some("test.kdl"));
    assert!(spec.is_none());
    assert!(report.has_errors());
    let diag = report.diagnostics.iter().find(|d| d.code == "E0020").unwrap();
    assert!(diag.message.contains("usernam"));
    assert_eq!(diag.note.as_deref(), Some("Did you mean '{username}'?"));
    let rendered = report.render_plain();
    assert!(rendered.contains("error[E0020]"));
    assert!(rendered.contains("Did you mean '{username}'?"));
}

#[test]
fn test_diagnostics_undeclared_env_with_typo_suggestion() {
    let kdl = r#"
        server name="test-env-typo" version="1.0.0"
        env {
            API_SECRET default="xyz"
        }
        tool "fetch" {
            bind:http method="GET" url="https://api.com" {
                header "Authorization" "Bearer {env:API_SECRETT}"
            }
        }
    "#;
    let (spec, report) = Compiler::lint(kdl, Some("test.kdl"));
    assert!(spec.is_none());
    assert!(report.has_errors());
    let diag = report.diagnostics.iter().find(|d| d.code == "E0021").unwrap();
    assert!(diag.message.contains("API_SECRETT"));
    assert_eq!(diag.note.as_deref(), Some("Did you mean '{env:API_SECRET}'?"));
}

#[test]
fn test_diagnostics_invalid_constraints() {
    let kdl = r#"
        server name="test-constraints" version="1.0.0"
        tool "calc" {
            param "count" type="integer" minimum=100 maximum=10 pattern="[a-"
            exec "echo" {
                args "{count}"
            }
        }
    "#;
    let (spec, report) = Compiler::lint(kdl, Some("test.kdl"));
    assert!(spec.is_none());
    assert!(report.has_errors());
    let codes: Vec<&str> = report.diagnostics.iter().map(|d| d.code.as_str()).collect();
    assert!(codes.contains(&"E0031"));
    assert!(codes.contains(&"E0032"));
}

#[test]
fn test_diagnostics_warnings() {
    let kdl = r#"
        server name="test-warnings" version="1.0.0"
        env {
            UNUSED_ENV default="secret"
        }
        tool "hello" {
            param "unused_param" type="string"
            param "shadowed" type="string" required=#true default="default_val"
            exec "echo" {
                args "hi"
            }
        }
    "#;
    let (spec, report) = Compiler::lint(kdl, Some("test.kdl"));
    assert!(spec.is_some());
    assert!(!report.has_errors());
    assert!(report.has_warnings());
    let codes: Vec<&str> = report.diagnostics.iter().map(|d| d.code.as_str()).collect();
    assert!(codes.contains(&"W0001")); // Unused parameter
    assert!(codes.contains(&"W0002")); // Unused env
    assert!(codes.contains(&"W0003")); // Missing tool description
    assert!(codes.contains(&"W0004")); // Missing param description
    assert!(codes.contains(&"W0006")); // Shadowed default on required param
}

#[test]
fn test_diagnostics_missing_server() {
    let kdl = r#"
        tool "standalone_tool" description="No server" {
            bind:exec "echo"
        }
    "#;
    let (spec, report) = Compiler::lint(kdl, Some("test.kdl"));
    assert!(spec.is_none());
    assert!(report.has_errors());
    assert!(report.diagnostics.iter().any(|d| d.code == "E0002"));
}

#[test]
fn test_diagnostics_unknown_top_level() {
    let kdl = r#"
        servr name="typo-server" version="1.0.0"
    "#;
    let (spec, report) = Compiler::lint(kdl, Some("test.kdl"));
    assert!(spec.is_none());
    assert!(report.has_errors());
    let diag = report.diagnostics.iter().find(|d| d.code == "E0003").unwrap();
    assert!(diag.message.contains("servr"));
    assert_eq!(diag.note.as_deref(), Some("Did you mean 'server'?"));
}

#[test]
fn test_diagnostics_server_unknown_child_and_prop() {
    let kdl = r#"
        server name="ida-mcp-poc" version="0.1.0" {
            description "Native high-performance IDA Pro MCP Bridge"
            author "Reverse Engineering Team"
            license "MIT"
            protocol-version "2024-11-05"
            extra_node "invalid"
        }
        tool "ping" description="ping" {
            bind:exec "echo"
        }
    "#;
    let (spec, report) = Compiler::lint(kdl, Some("test.kdl"));
    assert!(spec.is_none());
    assert!(report.has_errors());
    let diag = report.diagnostics.iter().find(|d| d.code == "E0040").unwrap();
    assert!(diag.message.contains("extra_node"));
    assert!(diag.help.as_deref().unwrap().contains("Allowed children in server block"));
}

#[test]
fn test_diagnostics_transports_unknown_transport() {
    let kdl = r#"
        server name="test-transports" version="1.0.0"
        transports {
            stdio enabled=#true
            sse enabled=#true {
                host "127.0.0.1"
                port 8088
            }
            foo bar="baz"
        }
        tool "ping" description="ping" {
            bind:exec "echo"
        }
    "#;
    let (spec, report) = Compiler::lint(kdl, Some("test.kdl"));
    assert!(spec.is_none());
    assert!(report.has_errors());
    let diag = report.diagnostics.iter().find(|d| d.code == "E0041").unwrap();
    assert!(diag.message.contains("foo"));
    assert!(diag.help.as_deref().unwrap().contains("Supported transports are 'stdio' and 'sse'"));
}

#[test]
fn test_diagnostics_transports_all_disabled() {
    let kdl = r#"
        server name="test-all-disabled" version="1.0.0"
        transports {
            stdio enabled=#false
            sse enabled=#false
        }
        tool "ping" description="ping" {
            bind:exec "echo"
        }
    "#;
    let (spec, report) = Compiler::lint(kdl, Some("test.kdl"));
    assert!(spec.is_none());
    assert!(report.has_errors());
    assert!(report.diagnostics.iter().any(|d| d.code == "E0042"));
}

#[test]
fn test_diagnostics_tool_validation_missing_and_multiple_bindings() {
    let kdl = r#"
        server name="test-tool-val" version="1.0.0"
        tool "no_binding" description="missing binding" {
            param "x" type="string"
        }
        tool "multi_binding" description="multiple bindings" {
            bind:exec "echo"
            bind:http method="GET" url="https://example.com"
        }
    "#;
    let (spec, report) = Compiler::lint(kdl, Some("test.kdl"));
    assert!(spec.is_none());
    assert!(report.has_errors());
    assert!(report.diagnostics.iter().any(|d| d.code == "E0053"));
    assert!(report.diagnostics.iter().any(|d| d.code == "E0054"));
}

#[test]
fn test_diagnostics_tool_unknown_child_with_typo() {
    let kdl = r#"
        server name="test-tool-typo" version="1.0.0"
        tool "calc" description="calc tool" {
            paramss "num" type="integer"
            bind:exec "echo" {
                args "{num}"
                workdr "/tmp"
            }
        }
    "#;
    let (spec, report) = Compiler::lint(kdl, Some("test.kdl"));
    assert!(spec.is_none());
    assert!(report.has_errors());
    let diag_param = report.diagnostics.iter().find(|d| d.code == "E0052").unwrap();
    assert!(diag_param.message.contains("paramss"));
    assert_eq!(diag_param.note.as_deref(), Some("Did you mean 'param'?"));

    let diag_workdr = report.diagnostics.iter().find(|d| d.code == "E0056").unwrap();
    assert!(diag_workdr.message.contains("workdr"));
    assert_eq!(diag_workdr.note.as_deref(), Some("Did you mean 'workdir'?"));
}

#[test]
fn test_diagnostics_http_binding_validation() {
    let kdl = r#"
        server name="test-http-val" version="1.0.0"
        tool "fetch" description="fetch API" {
            bind:http method="INVALID_METHOD" {
                timeout-ms 5000
            }
        }
    "#;
    let (spec, report) = Compiler::lint(kdl, Some("test.kdl"));
    assert!(spec.is_none());
    assert!(report.has_errors());
    assert!(report.diagnostics.iter().any(|d| d.code == "E0057")); // missing URL
    assert!(report.diagnostics.iter().any(|d| d.code == "E0058")); // invalid HTTP method
}

#[test]
fn test_diagnostics_param_and_output_validation() {
    let kdl = r#"
        server name="test-param-val" version="1.0.0"
        tool "format_tool" description="formatter" {
            param "mode" type="strng" default="invalid_choice" {
                enum "fast" "slow"
            }
            output format="unknown_format"
            bind:exec "echo" {
                args "{mode}"
            }
        }
    "#;
    let (spec, report) = Compiler::lint(kdl, Some("test.kdl"));
    assert!(spec.is_none());
    assert!(report.has_errors());
    let diag_type = report.diagnostics.iter().find(|d| d.code == "E0071").unwrap();
    assert!(diag_type.message.contains("strng"));
    assert_eq!(diag_type.note.as_deref(), Some("Did you mean 'string'?"));

    let diag_enum = report.diagnostics.iter().find(|d| d.code == "E0073").unwrap();
    assert!(diag_enum.message.contains("invalid_choice"));

    let diag_out = report.diagnostics.iter().find(|d| d.code == "E0075").unwrap();
    assert!(diag_out.message.contains("unknown_format"));
}

#[test]
fn test_diagnostics_com_binding_validation() {
    let kdl_missing_progid = r#"
        server name="test-com-err" version="1.0.0"
        tool "broken_com" {
            bind:com {
                dispatch "Speak"
            }
        }
    "#;
    let (spec, report) = Compiler::lint(kdl_missing_progid, Some("test.kdl"));
    assert!(spec.is_none());
    assert!(report.has_errors());
    assert!(report.diagnostics.iter().any(|d| d.code == "E0061"));

    let kdl_unknown_prop = r#"
        server name="test-com-err2" version="1.0.0"
        tool "broken_com2" {
            bind:com "Excel.Application" attch=#true {
                dispach "Range"
            }
        }
    "#;
    let (spec2, report2) = Compiler::lint(kdl_unknown_prop, Some("test.kdl"));
    assert!(spec2.is_none());
    assert!(report2.has_errors());
    let diag_prop = report2.diagnostics.iter().find(|d| d.message.contains("attch")).unwrap();
    assert_eq!(diag_prop.code, "E0062");
    assert_eq!(diag_prop.note.as_deref(), Some("Did you mean 'attach'?"));

    let diag_child = report2.diagnostics.iter().find(|d| d.message.contains("dispach")).unwrap();
    assert_eq!(diag_child.code, "E0062");
    assert_eq!(diag_child.note.as_deref(), Some("Did you mean 'dispatch'?"));
}

#[test]
fn test_diagnostics_output_pipeline_validation() {
    let kdl_invalid_slice = r#"
        server name="test-out-err1" version="1.0.0"
        tool "bad_slice" {
            bind:exec "echo" {
                args "hi"
            }
            output {
                slice lines=0
            }
        }
    "#;
    let (spec1, report1) = Compiler::lint(kdl_invalid_slice, Some("test.kdl"));
    assert!(spec1.is_none());
    assert!(report1.has_errors());
    assert!(report1.diagnostics.iter().any(|d| d.code == "E0077"));

    let kdl_invalid_regex = r#"
        server name="test-out-err2" version="1.0.0"
        tool "bad_regex" {
            bind:exec "echo" {
                args "hi"
            }
            output {
                regex "[a-z"
            }
        }
    "#;
    let (spec2, report2) = Compiler::lint(kdl_invalid_regex, Some("test.kdl"));
    assert!(spec2.is_none());
    assert!(report2.has_errors());
    assert!(report2.diagnostics.iter().any(|d| d.code == "E0078"));

    let kdl_unknown_output_prop = r#"
        server name="test-out-err3" version="1.0.0"
        tool "bad_output" {
            bind:exec "echo" {
                args "hi"
            }
            output {
                trm #true
            }
        }
    "#;
    let (spec3, report3) = Compiler::lint(kdl_unknown_output_prop, Some("test.kdl"));
    assert!(spec3.is_none());
    assert!(report3.has_errors());
    let diag = report3.diagnostics.iter().find(|d| d.code == "E0076").unwrap();
    assert_eq!(diag.note.as_deref(), Some("Did you mean 'trim'?"));
}

#[test]
fn test_diagnostics_ws_and_pipe_validation() {
    let kdl_missing_ws_target = r#"
        server name="test-ws-err" version="1.0.0"
        tool "bad_ws" {
            bind:ws {
                message "hello"
            }
        }
    "#;
    let (spec1, report1) = Compiler::lint(kdl_missing_ws_target, Some("test.kdl"));
    assert!(spec1.is_none());
    assert!(report1.has_errors());
    assert!(report1.diagnostics.iter().any(|d| d.code == "E0063"));

    let kdl_missing_pipe_target = r#"
        server name="test-pipe-err" version="1.0.0"
        tool "bad_pipe" {
            bind:pipe {
                message "ping"
            }
        }
    "#;
    let (spec2, report2) = Compiler::lint(kdl_missing_pipe_target, Some("test.kdl"));
    assert!(spec2.is_none());
    assert!(report2.has_errors());
    assert!(report2.diagnostics.iter().any(|d| d.code == "E0064"));

    let kdl_unknown_ws_prop = r#"
        server name="test-ws-prop-err" version="1.0.0"
        tool "bad_ws_prop" {
            bind:ws url="ws://127.0.0.1:8080" {
                mesage "hello"
            }
        }
    "#;
    let (spec3, report3) = Compiler::lint(kdl_unknown_ws_prop, Some("test.kdl"));
    assert!(spec3.is_none());
    assert!(report3.has_errors());
    let diag = report3.diagnostics.iter().find(|d| d.code == "E0063").unwrap();
    assert_eq!(diag.note.as_deref(), Some("Did you mean 'message'?"));
}

#[test]
fn test_conditional_os_exec_diagnostics() {
    // 1. Missing command in variant
    let kdl_missing_var_cmd = r#"
        server name="test-diag" version="1.0.0"
        tool "broken-tool" {
            exec {
                when:windows ""
                when:unix "echo"
            }
        }
    "#;
    let (spec1, report1) = Compiler::lint(kdl_missing_var_cmd, Some("test.kdl"));
    assert!(spec1.is_none());
    assert!(report1.has_errors());
    assert!(report1.diagnostics.iter().any(|d| d.code == "E0055"));

    // 2. Incomplete platform coverage warning W0007 (only windows without fallback or unix)
    let kdl_incomplete_coverage = r#"
        server name="test-warn" version="1.0.0"
        tool "win-only-tool" {
            exec {
                when:windows "cmd" {
                    args "/c" "dir"
                }
            }
        }
    "#;
    let (spec2, report2) = Compiler::lint(kdl_incomplete_coverage, Some("test.kdl"));
    assert!(spec2.is_some());
    assert!(!report2.has_errors());
    assert!(report2.diagnostics.iter().any(|d| d.code == "W0007"));
}

#[test]
fn test_diagnostics_unknown_placeholder_modifier() {
    let kdl = r#"
        server name="test-mod" version="1.0.0"
        tool "run" {
            param "payload" type="string"
            exec "echo" {
                args "{payload:jzon}"
            }
        }
    "#;
    let (spec, report) = Compiler::lint(kdl, Some("test.kdl"));
    assert!(spec.is_none());
    assert!(report.has_errors());
    let diag = report.diagnostics.iter().find(|d| d.code == "E0025").unwrap();
    assert!(diag.message.contains("jzon"));
    assert_eq!(diag.note.as_deref(), Some("Did you mean ':json'?"));
    let rendered = report.render_plain();
    assert!(rendered.contains("error[E0025]"));
    assert!(rendered.contains("Did you mean ':json'?"));
}

#[test]
fn test_diagnostics_valid_placeholder_modifiers() {
    let kdl = r#"
        server name="test-valid-mods" version="1.0.0"
        env {
            API_HOST default="example.com"
        }
        tool "query" {
            param "code" type="string"
            param "val" type="object"
            param "search" type="string"
            bind:http method="POST" url="https://{env:API_HOST:url}/api?q={search:url}" {
                body "{\"code\": {code:json}, \"val\": {val:json}}"
            }
        }
    "#;
    let (spec, report) = Compiler::lint(kdl, Some("test.kdl"));
    assert!(spec.is_some(), "Expected clean lint, got:\n{}", report.render_plain());
    assert!(!report.has_errors());
}
