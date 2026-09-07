pub const HTTP_TEMPLATE: &str = r###"// -----------------------------------------------------------------------------
// HTTP Client Execution Engine (Synchronous via ureq)
// -----------------------------------------------------------------------------
{% if has_http_bindings %}
pub struct HttpOptions<'a> {
    pub method: &'a str,
    pub url: &'a str,
    pub headers: &'a [(String, String)],
    pub query: &'a [(String, String)],
    pub body: Option<&'a str>,
    pub timeout: Duration,
    pub extract_json_field: Option<&'a str>,
}

pub fn execute_http_request(opts: HttpOptions) -> Result<String, String> {
    mcp_log!("[HTTP] Sending {} {}", opts.method, opts.url);
    let mut req = match opts.method.to_uppercase().as_str() {
        "POST" => ureq::post(opts.url),
        "PUT" => ureq::put(opts.url),
        "DELETE" => ureq::delete(opts.url),
        "PATCH" => ureq::patch(opts.url),
        "HEAD" => ureq::head(opts.url),
        _ => ureq::get(opts.url),
    };

    req = req.timeout(opts.timeout);

    for (k, v) in opts.headers {
        req = req.set(k, v);
    }

    for (k, v) in opts.query {
        req = req.query(k, v);
    }

    let start = std::time::Instant::now();
    let response = if let Some(body_data) = opts.body {
        req.send_string(body_data)
    } else {
        req.call()
    };

    match response {
        Ok(resp) => {
            let status = resp.status();
            mcp_log!("[HTTP] Response status: {} in {}ms", status, start.elapsed().as_millis());
            let body_str = resp.into_string().map_err(|e| format!("Failed to read HTTP body: {}", e))?;
            if let Some(field) = opts.extract_json_field {
                if let Ok(json_val) = serde_json::from_str::<Value>(&body_str) {
                    if let Some(extracted) = json_val.pointer(field) {
                        return Ok(match extracted {
                            Value::String(s) => s.clone(),
                            other => other.to_string(),
                        });
                    }
                }
            }
            Ok(body_str)
        }
        Err(ureq::Error::Status(code, resp)) => {
            let err_body = resp.into_string().unwrap_or_default();
            Err(format!("HTTP {} error: {}", code, err_body))
        }
        Err(ureq::Error::Transport(transport_err)) => {
            Err(format!("Network transport error: {}", transport_err))
        }
    }
}
{% endif %}
"###;
