# 0003: Exclusive Rust Target for Compiler and Generated Servers

We decided to target Rust exclusively for both the `mcp-gen` compiler CLI and the generated standalone server binaries, removing C++ and other polyglot backends from the roadmap.

Rust provides first-class cross-compilation toolchains (`cross`, `musl`, Windows MSVC/GNU, macOS universal binaries), robust memory safety, strong ecosystem libraries (`kdl-rs`, `serde_json`, `ureq`), and produces single self-contained static executables (<1.5 MB, <1 ms cold start). Maintaining a single high-quality Rust code-emission backend eliminates template fragmentation and allows 100% engineering focus on compiler correctness, JSON Schema synthesis, and Profile template resolution.
