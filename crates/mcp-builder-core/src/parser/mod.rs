pub mod bindings;
pub mod output;
pub mod profiles;
pub mod prompts_resources;
pub mod server;
pub mod tools;
pub mod validators;

use crate::ast::ServerSpec;
use crate::diagnostics::{Diagnostic, DiagnosticReport, SourceLocation};
use crate::preprocess::preprocess_kdl;
use crate::profile::ProfileResolver;
use kdl::KdlDocument;
use thiserror::Error;

use profiles::parse_profile_node;
use prompts_resources::{parse_prompt_node, parse_resource_node, parse_resource_template_node};
use server::{parse_env_node, parse_server_node, parse_transports_node};
use tools::parse_tool_node;
use validators::{run_linter, validate_document};

#[derive(Debug, Error)]
pub enum ParserError {
    #[error("KDL Syntax Error: {0}")]
    KdlError(#[from] kdl::KdlError),

    #[error("Semantic Validation Error at {location}: {message}")]
    ValidationError {
        location: String,
        message: String,
    },

    #[error("{0}")]
    Diagnostics(String),
}

pub struct Parser;

impl Parser {
    pub fn parse(input: &str) -> Result<ServerSpec, ParserError> {
        let (maybe_spec, report) = Self::lint(input, None);
        if report.has_errors() {
            return Err(ParserError::Diagnostics(report.render_plain()));
        }
        maybe_spec.ok_or_else(|| ParserError::ValidationError {
            location: "mcp.kdl".to_string(),
            message: "Failed to parse specification".to_string(),
        })
    }

    pub fn lint(input: &str, file_name: Option<&str>) -> (Option<ServerSpec>, DiagnosticReport) {
        let file_str = file_name.unwrap_or("mcp.kdl");
        let mut report = DiagnosticReport::new(file_str, input);

        let preprocessed = preprocess_kdl(input);
        let doc: KdlDocument = match preprocessed.parse() {
            Ok(d) => d,
            Err(e) => {
                let offset = e.span.offset();
                let len = e.span.len();
                let loc = SourceLocation::from_offset(input, offset, len);
                report.add(
                    Diagnostic::error("E0001", format!("KDL Syntax Error: {}", e))
                        .with_location(loc)
                        .with_help("Check KDL syntax, unclosed quotes, or missing braces."),
                );
                return (None, report);
            }
        };

        validate_document(&doc, input, &mut report);

        let server_res = Self::from_document(&doc);
        let mut server = match server_res {
            Ok(s) => s,
            Err(e) => {
                report.add(Diagnostic::error("E0001", format!("{}", e)));
                return (None, report);
            }
        };

        ProfileResolver::resolve(&mut server, input, &mut report);

        run_linter(input, &server, &mut report);

        if report.has_errors() {
            (None, report)
        } else {
            (Some(server), report)
        }
    }

    pub fn from_document(doc: &KdlDocument) -> Result<ServerSpec, ParserError> {
        let mut server = ServerSpec::default();
        let mut server_declared = false;

        for node in doc.nodes() {
            let node_name = node.name().value();
            match node_name {
                "server" => {
                    server_declared = true;
                    parse_server_node(node, &mut server)?;
                }
                "transports" => {
                    parse_transports_node(node, &mut server.transports)?;
                }
                "env" => {
                    parse_env_node(node, &mut server.envs)?;
                }
                "profile" => {
                    let profile = parse_profile_node(node)?;
                    server.profiles.push(profile);
                }
                "tool" => {
                    let tool = parse_tool_node(node)?;
                    server.tools.push(tool);
                }
                "resource" => {
                    let resource = parse_resource_node(node)?;
                    server.resources.push(resource);
                }
                "resource-template" => {
                    let template = parse_resource_template_node(node)?;
                    server.resource_templates.push(template);
                }
                "prompt" => {
                    let prompt = parse_prompt_node(node)?;
                    server.prompts.push(prompt);
                }
                _ => {}
            }
        }

        if !server_declared && server.name.is_empty() {
            server.name = "mcp-server".to_string();
        }

        Ok(server)
    }
}
