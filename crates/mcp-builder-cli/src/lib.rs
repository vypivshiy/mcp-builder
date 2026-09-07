use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use mcp_builder_core::Compiler;
use std::fs;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "mcp-builder",
    version,
    about = "Universal Declarative Model Context Protocol (MCP) IDL Compiler"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Parse and validate an mcp.kdl IDL file
    Check {
        /// Path to the mcp.kdl file
        file: PathBuf,
    },
    /// Emit generated Rust server project source files
    Emit {
        /// Path to the mcp.kdl file
        file: PathBuf,
        /// Output directory where project files will be written
        #[arg(short, long, default_value = "./dist")]
        output: PathBuf,
    },
    /// Compile mcp.kdl into a standalone native binary
    Build {
        /// Path to the mcp.kdl file
        file: PathBuf,
        /// Output binary path
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Compile with release profile optimizations
        #[arg(short, long, default_value_t = true)]
        release: bool,
    },
}

pub fn run_cli() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Check { file } => {
            let content = fs::read_to_string(&file)
                .with_context(|| format!("Failed to read IDL file at '{}'", file.display()))?;
            let file_str = file.to_string_lossy();
            let (maybe_spec, report) = Compiler::lint(&content, Some(&file_str));

            let rendered = report.render_ansi();
            if !rendered.is_empty() {
                println!("{}", rendered);
            }

            if report.has_errors() {
                anyhow::bail!(
                    "Validation failed with {} error(s) and {} warning(s).",
                    report.error_count(),
                    report.warning_count()
                );
            }

            let spec = maybe_spec.expect("Spec must exist when there are no errors");

            println!("✓ Server: '{}' v{}", spec.name, spec.version);
            if let Some(desc) = &spec.description {
                println!("  Description: {}", desc);
            }
            println!("  Protocol Version: {}", spec.protocol_version);
            println!("  Transports: stdio={}", spec.transports.stdio);
            println!("  Tools ({}):", spec.tools.len());
            for tool in &spec.tools {
                let binding = match &tool.binding {
                    Some(mcp_builder_core::ToolBinding::Exec(e)) => {
                        if !e.variants.is_empty() {
                            let variants_str = e.variants.iter().map(|v| format!("{}:{}", v.os.as_str(), v.command)).collect::<Vec<_>>().join(", ");
                            if !e.command.is_empty() {
                                format!("exec:{} [{}]", e.command, variants_str)
                            } else {
                                format!("exec:[{}]", variants_str)
                            }
                        } else {
                            format!("exec:{}", e.command)
                        }
                    }
                    Some(mcp_builder_core::ToolBinding::Http(h)) => format!("http:{} {}", h.method, h.url),
                    Some(mcp_builder_core::ToolBinding::Ipc(i)) => format!("ipc:{} (method:{})", i.dir, i.method),
                    Some(mcp_builder_core::ToolBinding::Native(n)) => format!("native:{}", n.symbol),
                    Some(mcp_builder_core::ToolBinding::Com(c)) => format!("com:{}", c.progid),
                    Some(mcp_builder_core::ToolBinding::Ws(w)) => format!("ws:{}", w.url.as_deref().unwrap_or(w.host.as_deref().unwrap_or("127.0.0.1"))),
                    Some(mcp_builder_core::ToolBinding::Pipe(p)) => format!("pipe:{}", p.addr.as_deref().unwrap_or(p.host.as_deref().unwrap_or("127.0.0.1"))),
                    None => "none".to_string(),
                };
                println!(
                    "    - {} (params: {}, binding: {})",
                    tool.name,
                    tool.params.len(),
                    binding
                );
            }
            if !spec.resources.is_empty() || !spec.resource_templates.is_empty() {
                println!(
                    "  Resources: {} static, {} templates",
                    spec.resources.len(),
                    spec.resource_templates.len()
                );
            }
            if !spec.prompts.is_empty() {
                println!("  Prompts: {}", spec.prompts.len());
            }
            if !spec.envs.is_empty() {
                println!("  Environment variables: {}", spec.envs.len());
            }
            let warn_suffix = if report.has_warnings() {
                format!(" (with {} warning(s))", report.warning_count())
            } else {
                String::new()
            };
            println!("\nVerification successful: 0 errors detected{}.", warn_suffix);
        }
        Commands::Emit { file, output } => {
            let content = fs::read_to_string(&file)
                .with_context(|| format!("Failed to read IDL file at '{}'", file.display()))?;
            let filename = file.file_name().and_then(|f| f.to_str());
            let out_path = Compiler::emit_with_source(&content, &output, filename)
                .with_context(|| format!("Failed to emit Rust project to '{}'", output.display()))?;
            println!(
                "✓ Emitted standalone Rust MCP server project to '{}'",
                out_path.display()
            );
        }
        Commands::Build {
            file,
            output,
            release,
        } => {
            let content = fs::read_to_string(&file)
                .with_context(|| format!("Failed to read IDL file at '{}'", file.display()))?;
            let spec = Compiler::check(&content)
                .with_context(|| format!("Validation error in '{}'", file.display()))?;

            let default_name = {
                let mut n = spec.name.to_lowercase().replace(['_', ' '], "-");
                if cfg!(windows) {
                    n.push_str(".exe");
                }
                n
            };

            let out_bin = output.unwrap_or_else(|| PathBuf::from(default_name));

            println!(
                "Compiling MCP server '{}' (release: {})...",
                spec.name, release
            );

            let filename = file.file_name().and_then(|f| f.to_str());
            Compiler::build_with_source(&content, &out_bin, release, filename)
                .with_context(|| format!("Failed to build binary '{}'", out_bin.display()))?;

            println!("✓ Built standalone binary at '{}'", out_bin.display());
        }
    }

    Ok(())
}
