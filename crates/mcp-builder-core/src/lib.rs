//! # `mcp-builder-core`
//!
//! High-performance declarative Model Context Protocol (MCP) server compiler and IDL processing engine.
//!
//! This crate provides the complete toolchain to parse KDL 2.0 IDL specifications, lint and validate
//! semantics with rich span-preserving diagnostics, resolve transport profiles, synthesize JSON Schema
//! definitions (Draft 7/2020-12), and emit standalone, zero-dependency Rust server binaries.
//!
//! ## Architectural Pipeline
//!
//! ```text
//!  ┌──────────────┐      ┌─────────────┐      ┌─────────────┐
//!  │  mcp.kdl     │ ───► │ Preprocess  │ ───► │ KDL Parser  │
//!  └──────────────┘      └─────────────┘      └─────────────┘
//!                                                    │
//!                                                    ▼
//!  ┌──────────────┐      ┌─────────────┐      ┌─────────────┐
//!  │ Rust Codegen │ ◄─── │ Diagnostics │ ◄─── │   Profile   │
//!  │ & Schema Gen │      │   Linter    │      │  Resolver   │
//!  └──────────────┘      └─────────────┘      └─────────────┘
//! ```
//!
//! ## Key Modules
//!
//! - [`ast`]: Strongly typed Abstract Syntax Tree for server declarations, tools, bindings, profiles, resources, and prompts.
//! - [`parser`]: Resilient KDL 2.0 parser extracting metadata, type annotations, and execution bindings.
//! - [`preprocess`]: Preprocessor normalizing booleans (`#true`/`#false`) and type properties.
//! - [`profile`]: Profile catalog, built-in presets (`cdp-chrome`, `ida-pro`, `redis-tcp`, `rest-json`), and inheritance.
//! - [`diagnostics`]: Rich compiler diagnostic reporting engine with ANSI formatting, source carets, and typo suggestions.
//! - [`schema`]: JSON Schema synthesizer generating MCP-compliant `tools/list` and `prompts/list` metadata.
//! - [`codegen`]: Pure-Rust code generation emitting zero-dependency standalone server runtimes.
//! - [`compiler`]: Top-level orchestrator exposing `check`, `emit`, `build`, and `lint` workflows.

pub mod ast;
pub mod codegen;
pub mod compiler;
pub mod diagnostics;
pub mod helpers;
pub mod parser;
pub mod preprocess;
pub mod profile;
pub mod schema;

pub use ast::*;
pub use codegen::{CodeGenerator, GeneratedProject};
pub use compiler::Compiler;
pub use diagnostics::{
    find_closest_match, levenshtein_distance, Diagnostic, DiagnosticReport, DiagnosticSeverity,
    SourceLocation,
};
pub use helpers::*;
pub use parser::{Parser, ParserError};
pub use profile::{ProfileCatalog, ProfileResolver};
pub use schema::JsonSchemaSynthesizer;

/// Returns the current version of the `mcp-builder-core` crate.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
