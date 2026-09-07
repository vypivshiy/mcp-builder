# 0004: Minimalist HTTP and SSE Transport via httparse and std::net

We decided to implement HTTP/SSE transport (`GET /sse`, `POST /message`, Streamable HTTP) for generated MCP servers using `httparse` and synchronous `std::net::TcpListener` instead of heavy asynchronous web frameworks (Tokio, Axum, Actix-web).

Generated servers require ultra-low binary size (<1.5 MB), instant startup (<1 ms), and minimal memory usage (<2 MB RSS). Heavy async runtimes introduce 40+ transitive crates and add 3–5 MB to binary size. By pairing the zero-allocation `httparse` crate (~25 KB overhead, 0 transitive dependencies) with standard library TCP networking, multi-threaded connection handling, and CORS support, `mcp-builder` delivers full MCP HTTP/SSE transport compliance with zero async runtime overhead.
