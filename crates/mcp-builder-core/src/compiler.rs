use crate::ast::ServerSpec;
use crate::codegen::{CodeGenerator, GeneratedProject};
use crate::diagnostics::DiagnosticReport;
use crate::parser::Parser;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CompilerError {
    #[error("Parser Error: {0}")]
    Parser(#[from] crate::parser::ParserError),

    #[error("Codegen Error: {0}")]
    Codegen(#[from] crate::codegen::CodegenError),

    #[error("I/O Error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Cargo Build Failed: {0}")]
    BuildFailed(String),

    #[error("Built binary not found at expected location: {0}")]
    BinaryNotFound(PathBuf),
}

pub struct Compiler;

impl Compiler {
    /// Parse and validate an `mcp.kdl` source string.
    pub fn check(kdl_source: &str) -> Result<ServerSpec, CompilerError> {
        let spec = Parser::parse(kdl_source)?;
        Ok(spec)
    }

    /// Run full diagnostic linter on an `mcp.kdl` source string, returning AST (if valid) and all diagnostics.
    pub fn lint(kdl_source: &str, file_name: Option<&str>) -> (Option<ServerSpec>, DiagnosticReport) {
        Parser::lint(kdl_source, file_name)
    }

    /// Generate the Rust project files (`Cargo.toml`, `src/main.rs`) from `mcp.kdl`.
    pub fn generate(kdl_source: &str) -> Result<GeneratedProject, CompilerError> {
        Self::generate_with_source(kdl_source, None)
    }

    /// Generate the Rust project files (`Cargo.toml`, `src/main.rs`) from a specific source file name.
    pub fn generate_with_source(
        kdl_source: &str,
        source_file: Option<&str>,
    ) -> Result<GeneratedProject, CompilerError> {
        let spec = Self::check(kdl_source)?;
        let project = CodeGenerator::generate_rust_project_with_source(&spec, source_file)?;
        Ok(project)
    }

    /// Emit generated Rust project files to a destination directory on disk.
    pub fn emit(kdl_source: &str, output_dir: &Path) -> Result<PathBuf, CompilerError> {
        Self::emit_with_source(kdl_source, output_dir, None)
    }

    /// Emit generated Rust project files to a destination directory with source file metadata.
    pub fn emit_with_source(
        kdl_source: &str,
        output_dir: &Path,
        source_file: Option<&str>,
    ) -> Result<PathBuf, CompilerError> {
        let project = Self::generate_with_source(kdl_source, source_file)?;

        fs::create_dir_all(output_dir)?;
        let src_dir = output_dir.join("src");
        fs::create_dir_all(&src_dir)?;

        fs::write(output_dir.join("Cargo.toml"), &project.cargo_toml)?;
        fs::write(src_dir.join("main.rs"), &project.main_rs)?;

        Ok(output_dir.to_path_buf())
    }

    /// Compile `mcp.kdl` directly into a standalone executable binary using `cargo build`.
    pub fn build(
        kdl_source: &str,
        output_binary_path: &Path,
        release: bool,
    ) -> Result<PathBuf, CompilerError> {
        Self::build_with_source(kdl_source, output_binary_path, release, None)
    }

    /// Compile a specific KDL source file into a standalone executable binary.
    pub fn build_with_source(
        kdl_source: &str,
        output_binary_path: &Path,
        release: bool,
        source_file: Option<&str>,
    ) -> Result<PathBuf, CompilerError> {
        let temp_dir = tempfile::tempdir()?;
        let temp_path = temp_dir.path();

        let spec = Self::check(kdl_source)?;
        let server_name_kebab = spec.name.to_lowercase().replace(['_', ' '], "-");

        Self::emit_with_source(kdl_source, temp_path, source_file)?;

        let mut cmd = Command::new("cargo");
        cmd.arg("build");
        cmd.current_dir(temp_path);

        if release {
            cmd.arg("--release");
        }

        let output = cmd.output().map_err(|e| {
            CompilerError::BuildFailed(format!("Failed to spawn cargo build: {}", e))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Err(CompilerError::BuildFailed(format!(
                "Cargo build failed with code {:?}:\nSTDOUT:\n{}\nSTDERR:\n{}",
                output.status.code(),
                stdout,
                stderr
            )));
        }

        let target_dir = if release { "release" } else { "debug" };
        let mut binary_name = server_name_kebab.clone();
        if cfg!(windows) {
            binary_name.push_str(".exe");
        }

        let built_binary = temp_path
            .join("target")
            .join(target_dir)
            .join(&binary_name);

        if !built_binary.exists() {
            return Err(CompilerError::BinaryNotFound(built_binary));
        }

        if let Some(parent) = output_binary_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::copy(&built_binary, output_binary_path)?;
        if cfg!(windows) && output_binary_path.extension().and_then(|s| s.to_str()) != Some("exe") {
            let exe_path = output_binary_path.with_extension("exe");
            let _ = fs::copy(&built_binary, &exe_path);
        }

        Ok(output_binary_path.to_path_buf())
    }
}
