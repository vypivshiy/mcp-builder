use mcp_builder_core::{
    EnvType, ParamType, Parser, ToolBinding,
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

#[test]
fn test_parse_sample_kdl() {
    let spec = Parser::parse(SAMPLE_KDL).expect("Failed to parse sample KDL");
    assert_eq!(spec.name, "sys-mcp");
    assert_eq!(spec.version, "1.0.0");
    assert_eq!(spec.tools.len(), 2);
    assert_eq!(spec.resources.len(), 1);
    assert_eq!(spec.prompts.len(), 1);
    assert_eq!(spec.envs.len(), 2);

    let tool0 = &spec.tools[0];
    assert_eq!(tool0.name, "git_log");
    assert_eq!(tool0.params.len(), 2);
    assert_eq!(tool0.params[0].name, "repo_path");
    assert_eq!(tool0.params[0].param_type, ParamType::String);
    assert!(tool0.params[0].required);

    assert_eq!(tool0.params[1].name, "max_count");
    assert_eq!(tool0.params[1].param_type, ParamType::Integer);
    assert_eq!(tool0.params[1].minimum, Some(1.0));
    assert_eq!(tool0.params[1].maximum, Some(100.0));
}

#[test]
fn test_type_annotated_params() {
    let kdl = r#"
        server name="test-types" version="1.0.0"
        tool "calc" {
            (integer)param "a" required=#true
            (number)param "b" default=3.14
            (boolean)param "c"
            param (string)"d"
            bind:exec "echo" {
                args "{a}" "{b}" "{c}" "{d}"
            }
        }
    "#;
    let spec = Parser::parse(kdl).expect("Failed to parse typed params");
    assert_eq!(spec.tools[0].params.len(), 4);
    assert_eq!(spec.tools[0].params[0].param_type, ParamType::Integer);
    assert_eq!(spec.tools[0].params[1].param_type, ParamType::Number);
    assert_eq!(spec.tools[0].params[2].param_type, ParamType::Boolean);
    assert_eq!(spec.tools[0].params[3].param_type, ParamType::String);
}

#[test]
fn test_lint_undeclared_env_fails() {
    let kdl = r#"
        server name="test-lint-env" version="1.0.0"
        tool "fetch" {
            bind:http method="GET" url="https://api.com" {
                header "Authorization" "Bearer {env:MISSING_TOKEN}"
            }
        }
    "#;
    let err = Parser::parse(kdl).unwrap_err();
    let err_msg = err.to_string();
    assert!(err_msg.contains("MISSING_TOKEN"));
}

#[test]
fn test_lint_undeclared_param_fails() {
    let kdl = r#"
        server name="test-lint-param" version="1.0.0"
        tool "greet" {
            param "name" type="string"
            exec "echo" {
                args "Hello {typo_name}"
            }
        }
    "#;
    let err = Parser::parse(kdl).unwrap_err();
    let err_msg = err.to_string();
    assert!(err_msg.contains("typo_name"));
}

#[test]
fn test_enum_param_synthesis() {
    let kdl = r#"
        server name="test-enum" version="1.0.0"
        tool "calc" description="calculator" {
            (number)param "a" required=#true
            (number)param "b" required=#true
            (string)param "o" required=#true description="operation" {
                enum "+" "-" "*" "/"
            }
            param "mode" enum="fast,precise" default="fast"
            bind:exec "echo" {
                args "{a}" "{b}" "{o}" "{mode}"
            }
        }
    "#;
    let spec = Parser::parse(kdl).expect("Failed to parse enum params");
    let schemas = mcp_builder_core::JsonSchemaSynthesizer::synthesize_tool_input_schema(&spec.tools[0].params);
    assert_eq!(
        schemas["properties"]["o"]["enum"],
        serde_json::json!(["+", "-", "*", "/"])
    );
    assert_eq!(
        schemas["properties"]["mode"]["enum"],
        serde_json::json!(["fast", "precise"])
    );
}

#[test]
fn test_ipc_binding_and_typed_env() {
    let kdl = r#"
        server name="ue4-mcp" version="1.0.0"
        env {
            (path)UE4SS_MODS_DIR cli="--mods-dir" short="-m" default="Mods" description="Path to Mods directory"
        }
        tool "eval" description="Evaluate code" {
            (string)param "code" required=#true
            bind:ipc {
                dir "{env:UE4SS_MODS_DIR}/UE4SS_MCP/ipc"
                method "eval"
                params "{\"code\":\"{code}\"}"
                timeout-ms 5000
            }
        }
    "#;
    let spec = Parser::parse(kdl).expect("Failed to parse IPC binding and typed env");
    assert_eq!(spec.envs.len(), 1);
    assert_eq!(spec.envs[0].name, "UE4SS_MODS_DIR");
    assert_eq!(spec.envs[0].var_type, EnvType::Path);
    assert_eq!(spec.envs[0].cli.as_deref(), Some("--mods-dir"));
    assert_eq!(spec.envs[0].short.as_deref(), Some("-m"));

    assert_eq!(spec.tools.len(), 1);
    let tool = &spec.tools[0];
    assert_eq!(tool.name, "eval");
    match &tool.binding {
        Some(ToolBinding::Ipc(ipc)) => {
            assert_eq!(ipc.dir, "{env:UE4SS_MODS_DIR}/UE4SS_MCP/ipc");
            assert_eq!(ipc.method, "eval");
            assert_eq!(ipc.params.as_deref(), Some("{\"code\":\"{code}\"}"));
            assert_eq!(ipc.timeout_ms, 5000);
        }
        other => panic!("Expected Ipc binding, got {:?}", other),
    }

    let gen = mcp_builder_core::CodeGenerator::generate_rust_project(&spec).expect("Codegen failed");
    assert!(gen.main_rs.contains("execute_ipc_request"));
    assert!(gen.main_rs.contains("--mods-dir"));
    assert!(gen.main_rs.contains("-m"));
    assert!(gen.main_rs.contains("resolve_path_value"));
}
