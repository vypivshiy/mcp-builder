use crate::ast::*;
use crate::schema::JsonSchemaSynthesizer;
use minijinja::{context, Environment};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod context;
pub mod runtime;

pub use self::context::*;

#[derive(Debug, Error)]
pub enum CodegenError {
    #[error("Template Rendering Error: {0}")]
    TemplateError(#[from] minijinja::Error),

    #[error("I/O Error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization Error: {0}")]
    JsonError(#[from] serde_json::Error),
}

pub struct CodeGenerator;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedProject {
    pub cargo_toml: String,
    pub main_rs: String,
}

impl CodeGenerator {
    pub fn generate_rust_project(server: &ServerSpec) -> Result<GeneratedProject, CodegenError> {
        Self::generate_rust_project_with_source(server, None)
    }

    pub fn generate_rust_project_with_source(
        server: &ServerSpec,
        source_file: Option<&str>,
    ) -> Result<GeneratedProject, CodegenError> {
        let mut env = Environment::new();
        env.add_template("Cargo.toml", CARGO_TOML_TEMPLATE)?;
        env.add_template("main.rs", MAIN_RS_TEMPLATE)?;
        env.add_template("runtime/header.rs", runtime::HEADER_TEMPLATE)?;
        env.add_template("runtime/output.rs", runtime::output::OUTPUT_TEMPLATE)?;
        env.add_template("runtime/exec.rs", runtime::exec::EXEC_TEMPLATE)?;
        env.add_template("runtime/http.rs", runtime::http::HTTP_TEMPLATE)?;
        env.add_template("runtime/ipc.rs", runtime::ipc::IPC_TEMPLATE)?;
        env.add_template("runtime/com.rs", runtime::com::COM_TEMPLATE)?;
        env.add_template("runtime/ws_pipe.rs", runtime::ws_pipe::WS_PIPE_TEMPLATE)?;
        env.add_template("runtime/dispatch.rs", runtime::DISPATCH_TEMPLATE)?;
        env.add_template(
            "runtime/prompts_resources.rs",
            runtime::prompts_resources::PROMPTS_RESOURCES_TEMPLATE,
        )?;
        env.add_template("runtime/server.rs", runtime::SERVER_TEMPLATE)?;
        env.add_template("runtime/stdio.rs", runtime::stdio::STDIO_TEMPLATE)?;
        env.add_template("runtime/sse.rs", runtime::sse::SSE_TEMPLATE)?;
        env.add_template("runtime/cli.rs", runtime::CLI_TEMPLATE)?;

        let has_http_bindings = server
            .tools
            .iter()
            .any(|t| matches!(t.binding, Some(ToolBinding::Http(_))))
            || server
                .resource_templates
                .iter()
                .any(|t| matches!(t.binding, Some(ToolBinding::Http(_))));
        let tools_list_json = serde_json::to_string_pretty(
            &JsonSchemaSynthesizer::synthesize_tools_list(&server.tools),
        )?;
        let resources_list_json = serde_json::to_string_pretty(
            &JsonSchemaSynthesizer::synthesize_resources_list(
                &server.resources,
                &server.resource_templates,
            ),
        )?;
        let prompts_list_json = serde_json::to_string_pretty(
            &JsonSchemaSynthesizer::synthesize_prompts_list(&server.prompts),
        )?;

        let server_name_kebab = server.name.to_lowercase().replace(['_', ' '], "-");

        let compiled_prompts = context::compile_prompts(&server.prompts);
        let compiled_resources = context::compile_resources(&server.resources);
        let compiled_resource_templates =
            context::compile_resource_templates(&server.resource_templates);
        let compiled_tools = context::compile_tools(&server.tools);
        let compiled_envs = context::compile_envs(&server.envs);

        let default_host = server
            .transports
            .sse_host
            .as_deref()
            .unwrap_or("0.0.0.0");
        let default_port = server.transports.sse_port.unwrap_or(8000);

        let ctx = context! {
            source_file => source_file.unwrap_or("mcp.kdl"),
            server_name => server.name,
            server_name_kebab => server_name_kebab,
            server_version => server.version,
            server_description => server.description.as_deref().unwrap_or("Standalone Native MCP Server"),
            server_author => server.author.as_deref().unwrap_or("mcp-builder"),
            server_license => server.license.as_deref().unwrap_or("MIT"),
            protocol_version => server.protocol_version,
            has_http_bindings => has_http_bindings,
            tools_list_json => tools_list_json,
            resources_list_json => resources_list_json,
            prompts_list_json => prompts_list_json,
            tools => compiled_tools,
            prompts => compiled_prompts,
            resources => compiled_resources,
            resource_templates => compiled_resource_templates,
            envs => compiled_envs,
            default_host => default_host,
            default_port => default_port,
        };

        let cargo_toml = env.get_template("Cargo.toml")?.render(&ctx)?;
        let main_rs = env.get_template("main.rs")?.render(&ctx)?;

        Ok(GeneratedProject {
            cargo_toml,
            main_rs,
        })
    }
}

const CARGO_TOML_TEMPLATE: &str = r#"# =============================================================================
# Generated by mcp-builder from {{ source_file }} - DO NOT EDIT MANUALLY
# =============================================================================

[package]
name = "{{ server_name_kebab }}"
version = "{{ server_version }}"
edition = "2021"
authors = ["{{ server_author }}"]
description = "{{ server_description }}"
license = "{{ server_license }}"
publish = false

[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
httparse = "1.9"
{% if has_http_bindings %}
ureq = { version = "2.10", default-features = false, features = ["tls", "json"] }
{% endif %}

[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
panic = "abort"
strip = true
"#;

const MAIN_RS_TEMPLATE: &str = r###"// =============================================================================
// Generated by mcp-builder from {{ source_file }} - DO NOT EDIT MANUALLY
// =============================================================================

{% include "runtime/header.rs" %}
{% include "runtime/output.rs" %}
{% include "runtime/exec.rs" %}
{% include "runtime/http.rs" %}
{% include "runtime/ipc.rs" %}
{% include "runtime/com.rs" %}
{% include "runtime/ws_pipe.rs" %}
{% include "runtime/dispatch.rs" %}
{% include "runtime/prompts_resources.rs" %}
{% include "runtime/server.rs" %}
{% include "runtime/stdio.rs" %}
{% include "runtime/sse.rs" %}
{% include "runtime/cli.rs" %}
"###;
