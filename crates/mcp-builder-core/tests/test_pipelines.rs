use mcp_builder_core::{
    CodeGenerator, JsonSchemaSynthesizer, OutputSliceSpec, Parser,
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
fn test_schema_synthesis() {
    let spec = Parser::parse(SAMPLE_KDL).expect("Parse failed");
    let tools_list = JsonSchemaSynthesizer::synthesize_tools_list(&spec.tools);
    assert!(tools_list.is_array());
    let arr = tools_list.as_array().unwrap();
    assert_eq!(arr.len(), 2);

    let t0 = &arr[0];
    assert_eq!(t0["name"], "git_log");
    assert_eq!(t0["inputSchema"]["type"], "object");
    assert_eq!(t0["inputSchema"]["properties"]["repo_path"]["type"], "string");
    assert_eq!(t0["inputSchema"]["properties"]["max_count"]["type"], "integer");
    assert_eq!(t0["inputSchema"]["properties"]["max_count"]["default"], 10);
    assert_eq!(t0["inputSchema"]["required"], serde_json::json!(["repo_path"]));
}

#[test]
fn test_codegen_rust_project() {
    let spec = Parser::parse(SAMPLE_KDL).expect("Parse failed");
    let project = CodeGenerator::generate_rust_project(&spec).expect("Codegen failed");
    assert!(project.cargo_toml.contains("sys-mcp"));
    assert!(project.main_rs.contains("pub const SERVER_NAME: &str = \"sys-mcp\";"));
    assert!(project.main_rs.contains("TOOLS_LIST_JSON"));
    assert!(project.main_rs.contains("git_log"));
}

#[test]
fn test_output_transformation_parsing_and_codegen() {
    let kdl = r#"
        server name="test-output" version="1.0.0" {
            description "Output pipeline testing server"
        }

        tool "git_log" description="Get git log summary" {
            bind:exec "git" {
                args "log" "--oneline"
            }
            output {
                format "text"
                trim #true
                slice lines=15 head=#true
                filter-not "^\\s*$"
                filter "^commit"
            }
        }

        tool "fetch_email" description="Fetch user email" {
            bind:http method="GET" url="https://api.internal/user"
            output {
                format "text"
                extract-json "/data/attributes/email"
                regex "^(?P<user>[^@]+)@(?P<domain>.+)$" {
                    template "User '{user}' on host '{domain}'"
                }
            }
        }

        tool "tail_logs" description="Tail last 5 lines" {
            bind:exec "tail" {
                args "-n" "50" "log.txt"
            }
            output {
                slice lines=5 tail=#true
            }
        }
    "#;
    let spec = Parser::parse(kdl).expect("Failed to parse output transformation KDL");
    assert_eq!(spec.tools.len(), 3);

    let t1 = &spec.tools[0];
    assert_eq!(t1.name, "git_log");
    assert!(t1.output.trim);
    assert_eq!(t1.output.slice, Some(OutputSliceSpec { lines: 15, head: true }));
    assert_eq!(t1.output.filter_not.as_deref(), Some(r#"^\s*$"#));
    assert_eq!(t1.output.filter.as_deref(), Some(r#"^commit"#));

    let t2 = &spec.tools[1];
    assert_eq!(t2.name, "fetch_email");
    assert_eq!(t2.output.extract_json.as_deref(), Some("/data/attributes/email"));
    let reg = t2.output.regex.as_ref().unwrap();
    assert_eq!(reg.pattern, r#"^(?P<user>[^@]+)@(?P<domain>.+)$"#);
    assert_eq!(reg.template.as_deref(), Some("User '{user}' on host '{domain}'"));

    let t3 = &spec.tools[2];
    assert_eq!(t3.name, "tail_logs");
    assert_eq!(t3.output.slice, Some(OutputSliceSpec { lines: 5, head: false }));

    let gen = CodeGenerator::generate_rust_project(&spec).expect("Codegen failed");
    assert!(gen.main_rs.contains("apply_output_pipeline"));
    assert!(gen.main_rs.contains("MicroRegex"));
}
