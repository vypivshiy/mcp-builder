pub const PROMPTS_RESOURCES_TEMPLATE: &str = r###"// -----------------------------------------------------------------------------
// URI Template Matching Engine (RFC 6570 Level 1 Variable Matching)
// -----------------------------------------------------------------------------
pub fn match_uri_template(template: &str, uri: &str) -> Option<serde_json::Map<String, Value>> {
    let mut params = serde_json::Map::new();
    let mut t_idx = 0;
    let mut u_idx = 0;
    let t_bytes = template.as_bytes();
    let u_bytes = uri.as_bytes();

    while t_idx < t_bytes.len() {
        if t_bytes[t_idx] == b'{' {
            let var_start = t_idx + 1;
            let mut var_end = var_start;
            while var_end < t_bytes.len() && t_bytes[var_end] != b'}' {
                var_end += 1;
            }
            if var_end >= t_bytes.len() {
                return None;
            }
            let var_name = std::str::from_utf8(&t_bytes[var_start..var_end]).ok()?;
            t_idx = var_end + 1;

            if t_idx == t_bytes.len() {
                if u_idx > u_bytes.len() {
                    return None;
                }
                let val_str = std::str::from_utf8(&u_bytes[u_idx..]).ok()?;
                if val_str.is_empty() {
                    return None;
                }
                params.insert(var_name.to_string(), Value::String(val_str.to_string()));
                u_idx = u_bytes.len();
                break;
            } else {
                let lit_start = t_idx;
                let mut lit_end = lit_start;
                while lit_end < t_bytes.len() && t_bytes[lit_end] != b'{' {
                    lit_end += 1;
                }
                let next_lit = std::str::from_utf8(&t_bytes[lit_start..lit_end]).ok()?;
                t_idx = lit_end;

                let remaining_u = std::str::from_utf8(&u_bytes[u_idx..]).ok()?;
                if let Some(pos) = remaining_u.find(next_lit) {
                    let val_str = &remaining_u[..pos];
                    if val_str.is_empty() {
                        return None;
                    }
                    params.insert(var_name.to_string(), Value::String(val_str.to_string()));
                    u_idx += pos + next_lit.len();
                } else {
                    return None;
                }
            }
        } else {
            let lit_start = t_idx;
            let mut lit_end = lit_start;
            while lit_end < t_bytes.len() && t_bytes[lit_end] != b'{' {
                lit_end += 1;
            }
            let lit = std::str::from_utf8(&t_bytes[lit_start..lit_end]).ok()?;
            t_idx = lit_end;

            if u_idx + lit.len() > u_bytes.len() {
                return None;
            }
            let u_segment = std::str::from_utf8(&u_bytes[u_idx..u_idx + lit.len()]).ok()?;
            if u_segment != lit {
                return None;
            }
            u_idx += lit.len();
        }
    }

    if u_idx == u_bytes.len() {
        Some(params)
    } else {
        None
    }
}

// -----------------------------------------------------------------------------
// Prompts & Resources Execution Engines
// -----------------------------------------------------------------------------
pub fn execute_prompt_get(name: &str, args: &Value) -> Result<Value, String> {
    match name {
{% for prompt in prompts %}
        "{{ prompt.name }}" => {
            let mut effective_args = match args.as_object() {
                Some(map) => map.clone(),
                None => serde_json::Map::new(),
            };

{% for arg in prompt.arguments %}
{% if arg.required %}
            if !effective_args.contains_key("{{ arg.name }}") {
{% if arg.default %}
                effective_args.insert("{{ arg.name }}".to_string(), Value::String(r#"{{ arg.default }}"#.to_string()));
{% else %}
                return Err(format!("Missing required prompt argument '{}' for prompt '{}'", "{{ arg.name }}", "{{ prompt.name }}"));
{% endif %}
            }
{% else %}
{% if arg.default %}
            if !effective_args.contains_key("{{ arg.name }}") {
                effective_args.insert("{{ arg.name }}".to_string(), Value::String(r#"{{ arg.default }}"#.to_string()));
            }
{% endif %}
{% endif %}
{% endfor %}

            let effective_val = Value::Object(effective_args);
            let args_ref = &effective_val;

            let mut messages = Vec::new();
{% for msg in prompt.messages %}
            {
                let text_content = interpolate_template(r#"{{ msg.content }}"#, args_ref);
                messages.push(json!({
                    "role": "{{ msg.role }}",
                    "content": {
                        "type": "text",
                        "text": text_content
                    }
                }));
            }
{% endfor %}

            let mut result = serde_json::Map::new();
{% if prompt.description %}
            result.insert("description".to_string(), Value::String(r#"{{ prompt.description }}"#.to_string()));
{% endif %}
            result.insert("messages".to_string(), Value::Array(messages));

            Ok(Value::Object(result))
        }
{% endfor %}
        unknown => Err(format!("Prompt '{}' not found", unknown)),
    }
}

pub fn execute_resource_read(uri: &str) -> Result<Value, String> {
    // 1. Static Resources Exact Match
    match uri {
{% for res in resources %}
        "{{ res.uri }}" => {
{% if res.content_kind == "text" %}
            let text = interpolate_template(r#"{{ res.text_content }}"#, &json!({}));
            return Ok(json!({
                "contents": [{
                    "uri": "{{ res.uri }}",
                    "mimeType": "{{ res.mime_type }}",
                    "text": text
                }]
            }));
{% elif res.content_kind == "binary" %}
            return Ok(json!({
                "contents": [{
                    "uri": "{{ res.uri }}",
                    "mimeType": "{{ res.mime_type }}",
                    "blob": r#"{{ res.binary_content }}"#
                }]
            }));
{% elif res.content_kind == "exec" %}
            let (cmd_template, args_templates, workdir_template, envs_templates, timeout_ms_val) = {
                #[allow(unused_mut, unused_assignments)]
                let mut matched_variant: Option<(&str, Vec<&str>, Option<&str>, Vec<(&str, &str)>, u64)> = None;

                if cfg!(windows) {
                    {% for v in res.exec_variants %}
                    {% if v.os == "windows" %}
                    matched_variant = Some((
                        r#"{{ v.command }}"#,
                        vec![{% for a in v.args %}r#"{{ a }}"#,{% endfor %}],
                        {% if v.workdir %}Some(r#"{{ v.workdir }}"#){% else %}{% if res.exec_workdir %}Some(r#"{{ res.exec_workdir }}"#){% else %}None{% endif %}{% endif %},
                        vec![{% for e in v.envs %}(r#"{{ e.0 }}"#, r#"{{ e.1 }}"#),{% endfor %}],
                        {% if v.timeout_ms %}{{ v.timeout_ms }}{% else %}{{ res.exec_timeout_ms }}{% endif %},
                    ));
                    {% endif %}
                    {% endfor %}
                } else if cfg!(target_os = "linux") {
                    {% for v in res.exec_variants %}
                    {% if v.os == "linux" %}
                    matched_variant = Some((
                        r#"{{ v.command }}"#,
                        vec![{% for a in v.args %}r#"{{ a }}"#,{% endfor %}],
                        {% if v.workdir %}Some(r#"{{ v.workdir }}"#){% else %}{% if res.exec_workdir %}Some(r#"{{ res.exec_workdir }}"#){% else %}None{% endif %}{% endif %},
                        vec![{% for e in v.envs %}(r#"{{ e.0 }}"#, r#"{{ e.1 }}"#),{% endfor %}],
                        {% if v.timeout_ms %}{{ v.timeout_ms }}{% else %}{{ res.exec_timeout_ms }}{% endif %},
                    ));
                    {% endif %}
                    {% endfor %}
                    if matched_variant.is_none() {
                        {% for v in res.exec_variants %}
                        {% if v.os == "unix" %}
                        matched_variant = Some((
                            r#"{{ v.command }}"#,
                            vec![{% for a in v.args %}r#"{{ a }}"#,{% endfor %}],
                            {% if v.workdir %}Some(r#"{{ v.workdir }}"#){% else %}{% if res.exec_workdir %}Some(r#"{{ res.exec_workdir }}"#){% else %}None{% endif %}{% endif %},
                            vec![{% for e in v.envs %}(r#"{{ e.0 }}"#, r#"{{ e.1 }}"#),{% endfor %}],
                            {% if v.timeout_ms %}{{ v.timeout_ms }}{% else %}{{ res.exec_timeout_ms }}{% endif %},
                        ));
                        {% endif %}
                        {% endfor %}
                    }
                } else if cfg!(target_os = "macos") {
                    {% for v in res.exec_variants %}
                    {% if v.os == "macos" %}
                    matched_variant = Some((
                        r#"{{ v.command }}"#,
                        vec![{% for a in v.args %}r#"{{ a }}"#,{% endfor %}],
                        {% if v.workdir %}Some(r#"{{ v.workdir }}"#){% else %}{% if res.exec_workdir %}Some(r#"{{ res.exec_workdir }}"#){% else %}None{% endif %}{% endif %},
                        vec![{% for e in v.envs %}(r#"{{ e.0 }}"#, r#"{{ e.1 }}"#),{% endfor %}],
                        {% if v.timeout_ms %}{{ v.timeout_ms }}{% else %}{{ res.exec_timeout_ms }}{% endif %},
                    ));
                    {% endif %}
                    {% endfor %}
                    if matched_variant.is_none() {
                        {% for v in res.exec_variants %}
                        {% if v.os == "unix" %}
                        matched_variant = Some((
                            r#"{{ v.command }}"#,
                            vec![{% for a in v.args %}r#"{{ a }}"#,{% endfor %}],
                            {% if v.workdir %}Some(r#"{{ v.workdir }}"#){% else %}{% if res.exec_workdir %}Some(r#"{{ res.exec_workdir }}"#){% else %}None{% endif %}{% endif %},
                            vec![{% for e in v.envs %}(r#"{{ e.0 }}"#, r#"{{ e.1 }}"#),{% endfor %}],
                            {% if v.timeout_ms %}{{ v.timeout_ms }}{% else %}{{ res.exec_timeout_ms }}{% endif %},
                        ));
                        {% endif %}
                        {% endfor %}
                    }
                } else if cfg!(unix) {
                    {% for v in res.exec_variants %}
                    {% if v.os == "unix" %}
                    matched_variant = Some((
                        r#"{{ v.command }}"#,
                        vec![{% for a in v.args %}r#"{{ a }}"#,{% endfor %}],
                        {% if v.workdir %}Some(r#"{{ v.workdir }}"#){% else %}{% if res.exec_workdir %}Some(r#"{{ res.exec_workdir }}"#){% else %}None{% endif %}{% endif %},
                        vec![{% for e in v.envs %}(r#"{{ e.0 }}"#, r#"{{ e.1 }}"#),{% endfor %}],
                        {% if v.timeout_ms %}{{ v.timeout_ms }}{% else %}{{ res.exec_timeout_ms }}{% endif %},
                    ));
                    {% endif %}
                    {% endfor %}
                }

                if let Some(v) = matched_variant {
                    v
                } else {
                    #[allow(unused_mut, unused_assignments)]
                    let mut fallback_variant: Option<(&str, Vec<&str>, Option<&str>, Vec<(&str, &str)>, u64)> = None;
                    {% for v in res.exec_variants %}
                    {% if v.os == "fallback" %}
                    fallback_variant = Some((
                        r#"{{ v.command }}"#,
                        vec![{% for a in v.args %}r#"{{ a }}"#,{% endfor %}],
                        {% if v.workdir %}Some(r#"{{ v.workdir }}"#){% else %}{% if res.exec_workdir %}Some(r#"{{ res.exec_workdir }}"#){% else %}None{% endif %}{% endif %},
                        vec![{% for e in v.envs %}(r#"{{ e.0 }}"#, r#"{{ e.1 }}"#),{% endfor %}],
                        {% if v.timeout_ms %}{{ v.timeout_ms }}{% else %}{{ res.exec_timeout_ms }}{% endif %},
                    ));
                    {% endif %}
                    {% endfor %}

                    if let Some(fb) = fallback_variant {
                        fb
                    } else {
                        {% if res.exec_command %}
                        (
                            r#"{{ res.exec_command }}"#,
                            vec![{% for a in res.exec_args %}r#"{{ a }}"#,{% endfor %}],
                            {% if res.exec_workdir %}Some(r#"{{ res.exec_workdir }}"#){% else %}{% if res.exec_workdir %}Some(r#"{{ res.exec_workdir }}"#){% else %}None{% endif %}{% endif %},
                            vec![{% for e in res.exec_envs %}(r#"{{ e.0 }}"#, r#"{{ e.1 }}"#),{% endfor %}],
                            {{ res.exec_timeout_ms }},
                        )
                        {% else %}
                        (
                            "",
                            vec![],
                            None,
                            vec![],
                            5000,
                        )
                        {% endif %}
                    }
                }
            };

            if cmd_template.is_empty() {
                return Err(format!("No executable command configured for target OS '{}' on resource '{}'", std::env::consts::OS, "{{ res.uri }}"));
            }

            let cmd_str = interpolate_template(cmd_template, &json!({}));
            let raw_args: Vec<String> = args_templates.into_iter().map(|a| interpolate_template(a, &json!({}))).collect();
            let workdir_str: Option<String> = workdir_template.map(|w| interpolate_template(w, &json!({})));
            let envs: Vec<(String, String)> = envs_templates.into_iter().map(|(k, v)| (k.to_string(), interpolate_template(v, &json!({})))).collect();

            let opts = ExecOptions {
                command: &cmd_str,
                args: &raw_args,
                workdir: workdir_str.as_deref(),
                envs: &envs,
                timeout: Duration::from_millis(timeout_ms_val),
            };
            match execute_subprocess(opts) {
                Ok(output) if output.status.success() => {
                    let text = String::from_utf8_lossy(&output.stdout).to_string();
                    return Ok(json!({
                        "contents": [{
                            "uri": "{{ res.uri }}",
                            "mimeType": "{{ res.mime_type }}",
                            "text": text
                        }]
                    }));
                }
                Ok(output) => {
                    let err = String::from_utf8_lossy(&output.stderr).to_string();
                    return Err(format!("Resource command failed: {}", err));
                }
                Err(e) => return Err(e),
            }
{% else %}
            return Ok(json!({
                "contents": [{
                    "uri": "{{ res.uri }}",
                    "mimeType": "{{ res.mime_type }}",
                    "text": ""
                }]
            }));
{% endif %}
        }
{% endfor %}
        _ => {}
    }

    // 2. Resource Templates Dynamic Matching
{% for tmpl in resource_templates %}
    if let Some(mut params_map) = match_uri_template(r#"{{ tmpl.uri_template }}"#, uri) {
{% for def in tmpl.defaults %}
        if !params_map.contains_key("{{ def.0 }}") {
            if let Ok(v) = serde_json::from_str::<Value>(r#"{{ def.1 }}"#) {
                params_map.insert("{{ def.0 }}".to_string(), v);
            }
        }
{% endfor %}
        let args_val = Value::Object(params_map);
{% if tmpl.binding_type == "exec" %}
        let (cmd_template, args_templates, workdir_template, envs_templates, timeout_ms_val) = {
            #[allow(unused_mut, unused_assignments)]
            let mut matched_variant: Option<(&str, Vec<&str>, Option<&str>, Vec<(&str, &str)>, u64)> = None;

            if cfg!(windows) {
                {% for v in tmpl.exec_variants %}
                {% if v.os == "windows" %}
                matched_variant = Some((
                    r#"{{ v.command }}"#,
                    vec![{% for a in v.args %}r#"{{ a }}"#,{% endfor %}],
                    {% if v.workdir %}Some(r#"{{ v.workdir }}"#){% else %}{% if tmpl.exec_workdir %}Some(r#"{{ tmpl.exec_workdir }}"#){% else %}None{% endif %}{% endif %},
                    vec![{% for e in v.envs %}(r#"{{ e.0 }}"#, r#"{{ e.1 }}"#),{% endfor %}],
                    {% if v.timeout_ms %}{{ v.timeout_ms }}{% else %}{{ tmpl.exec_timeout_ms }}{% endif %},
                ));
                {% endif %}
                {% endfor %}
            } else if cfg!(target_os = "linux") {
                {% for v in tmpl.exec_variants %}
                {% if v.os == "linux" %}
                matched_variant = Some((
                    r#"{{ v.command }}"#,
                    vec![{% for a in v.args %}r#"{{ a }}"#,{% endfor %}],
                    {% if v.workdir %}Some(r#"{{ v.workdir }}"#){% else %}{% if tmpl.exec_workdir %}Some(r#"{{ tmpl.exec_workdir }}"#){% else %}None{% endif %}{% endif %},
                    vec![{% for e in v.envs %}(r#"{{ e.0 }}"#, r#"{{ e.1 }}"#),{% endfor %}],
                    {% if v.timeout_ms %}{{ v.timeout_ms }}{% else %}{{ tmpl.exec_timeout_ms }}{% endif %},
                ));
                {% endif %}
                {% endfor %}
                if matched_variant.is_none() {
                    {% for v in tmpl.exec_variants %}
                    {% if v.os == "unix" %}
                    matched_variant = Some((
                        r#"{{ v.command }}"#,
                        vec![{% for a in v.args %}r#"{{ a }}"#,{% endfor %}],
                        {% if v.workdir %}Some(r#"{{ v.workdir }}"#){% else %}{% if tmpl.exec_workdir %}Some(r#"{{ tmpl.exec_workdir }}"#){% else %}None{% endif %}{% endif %},
                        vec![{% for e in v.envs %}(r#"{{ e.0 }}"#, r#"{{ e.1 }}"#),{% endfor %}],
                        {% if v.timeout_ms %}{{ v.timeout_ms }}{% else %}{{ tmpl.exec_timeout_ms }}{% endif %},
                    ));
                    {% endif %}
                    {% endfor %}
                }
            } else if cfg!(target_os = "macos") {
                {% for v in tmpl.exec_variants %}
                {% if v.os == "macos" %}
                matched_variant = Some((
                    r#"{{ v.command }}"#,
                    vec![{% for a in v.args %}r#"{{ a }}"#,{% endfor %}],
                    {% if v.workdir %}Some(r#"{{ v.workdir }}"#){% else %}{% if tmpl.exec_workdir %}Some(r#"{{ tmpl.exec_workdir }}"#){% else %}None{% endif %}{% endif %},
                    vec![{% for e in v.envs %}(r#"{{ e.0 }}"#, r#"{{ e.1 }}"#),{% endfor %}],
                    {% if v.timeout_ms %}{{ v.timeout_ms }}{% else %}{{ tmpl.exec_timeout_ms }}{% endif %},
                ));
                {% endif %}
                {% endfor %}
                if matched_variant.is_none() {
                    {% for v in tmpl.exec_variants %}
                    {% if v.os == "unix" %}
                    matched_variant = Some((
                        r#"{{ v.command }}"#,
                        vec![{% for a in v.args %}r#"{{ a }}"#,{% endfor %}],
                        {% if v.workdir %}Some(r#"{{ v.workdir }}"#){% else %}{% if tmpl.exec_workdir %}Some(r#"{{ tmpl.exec_workdir }}"#){% else %}None{% endif %}{% endif %},
                        vec![{% for e in v.envs %}(r#"{{ e.0 }}"#, r#"{{ e.1 }}"#),{% endfor %}],
                        {% if v.timeout_ms %}{{ v.timeout_ms }}{% else %}{{ tmpl.exec_timeout_ms }}{% endif %},
                    ));
                    {% endif %}
                    {% endfor %}
                }
            } else if cfg!(unix) {
                {% for v in tmpl.exec_variants %}
                {% if v.os == "unix" %}
                matched_variant = Some((
                    r#"{{ v.command }}"#,
                    vec![{% for a in v.args %}r#"{{ a }}"#,{% endfor %}],
                    {% if v.workdir %}Some(r#"{{ v.workdir }}"#){% else %}{% if tmpl.exec_workdir %}Some(r#"{{ tmpl.exec_workdir }}"#){% else %}None{% endif %}{% endif %},
                    vec![{% for e in v.envs %}(r#"{{ e.0 }}"#, r#"{{ e.1 }}"#),{% endfor %}],
                    {% if v.timeout_ms %}{{ v.timeout_ms }}{% else %}{{ tmpl.exec_timeout_ms }}{% endif %},
                ));
                {% endif %}
                {% endfor %}
            }

            if let Some(v) = matched_variant {
                v
            } else {
                #[allow(unused_mut, unused_assignments)]
                let mut fallback_variant: Option<(&str, Vec<&str>, Option<&str>, Vec<(&str, &str)>, u64)> = None;
                {% for v in tmpl.exec_variants %}
                {% if v.os == "fallback" %}
                fallback_variant = Some((
                    r#"{{ v.command }}"#,
                    vec![{% for a in v.args %}r#"{{ a }}"#,{% endfor %}],
                    {% if v.workdir %}Some(r#"{{ v.workdir }}"#){% else %}{% if tmpl.exec_workdir %}Some(r#"{{ tmpl.exec_workdir }}"#){% else %}None{% endif %}{% endif %},
                    vec![{% for e in v.envs %}(r#"{{ e.0 }}"#, r#"{{ e.1 }}"#),{% endfor %}],
                    {% if v.timeout_ms %}{{ v.timeout_ms }}{% else %}{{ tmpl.exec_timeout_ms }}{% endif %},
                ));
                {% endif %}
                {% endfor %}

                if let Some(fb) = fallback_variant {
                    fb
                } else {
                    {% if tmpl.exec_command %}
                    (
                        r#"{{ tmpl.exec_command }}"#,
                        vec![{% for a in tmpl.exec_args %}r#"{{ a }}"#,{% endfor %}],
                        {% if tmpl.exec_workdir %}Some(r#"{{ tmpl.exec_workdir }}"#){% else %}None{% endif %},
                        vec![{% for e in tmpl.exec_envs %}(r#"{{ e.0 }}"#, r#"{{ e.1 }}"#),{% endfor %}],
                        {{ tmpl.exec_timeout_ms }},
                    )
                    {% else %}
                    (
                        "",
                        vec![],
                        None,
                        vec![],
                        10000,
                    )
                    {% endif %}
                }
            }
        };

        if cmd_template.is_empty() {
            return Err(format!("No executable command configured for target OS '{}' on resource template '{}'", std::env::consts::OS, "{{ tmpl.uri_template }}"));
        }

        let cmd_str = interpolate_template(cmd_template, &args_val);
        let raw_args: Vec<String> = args_templates.into_iter().map(|a| interpolate_template(a, &args_val)).collect();
        let workdir_str: Option<String> = workdir_template.map(|w| interpolate_template(w, &args_val));
        let envs: Vec<(String, String)> = envs_templates.into_iter().map(|(k, v)| (k.to_string(), interpolate_template(v, &args_val))).collect();

        let opts = ExecOptions {
            command: &cmd_str,
            args: &raw_args,
            workdir: workdir_str.as_deref(),
            envs: &envs,
            timeout: Duration::from_millis(timeout_ms_val),
        };
        match execute_subprocess(opts) {
            Ok(output) if output.status.success() => {
                let text = String::from_utf8_lossy(&output.stdout).to_string();
                return Ok(json!({
                    "contents": [{
                        "uri": uri,
                        "mimeType": "{{ tmpl.mime_type }}",
                        "text": text
                    }]
                }));
            }
            Ok(output) => {
                let err = String::from_utf8_lossy(&output.stderr).to_string();
                return Err(format!("Resource template command failed: {}", err));
            }
            Err(e) => return Err(e),
        }
{% elif tmpl.binding_type == "http" %}
        let raw_url = interpolate_template(r#"{{ tmpl.http_url }}"#, &args_val);
        let headers: Vec<(String, String)> = vec![
{% for h in tmpl.http_headers %}
            ("{{ h.0 }}".to_string(), interpolate_template(r#"{{ h.1 }}"#, &args_val)),
{% endfor %}
        ];
        let query: Vec<(String, String)> = vec![
{% for q in tmpl.http_query %}
            ("{{ q.0 }}".to_string(), interpolate_template(r#"{{ q.1 }}"#, &args_val)),
{% endfor %}
        ];
        let body_str: Option<String> = {% if tmpl.http_body %}Some(interpolate_template(r#"{{ tmpl.http_body }}"#, &args_val)){% else %}None{% endif %};
        let extract_field: Option<&str> = {% if tmpl.http_extract %}Some(r#"{{ tmpl.http_extract }}"#){% else %}None{% endif %};

        let opts = HttpOptions {
            method: "{{ tmpl.http_method }}",
            url: &raw_url,
            headers: &headers,
            query: &query,
            body: body_str.as_deref(),
            timeout: Duration::from_millis({{ tmpl.http_timeout_ms }}),
            extract_json_field: extract_field,
        };

        match execute_http_request(opts) {
            Ok(resp) => {
                return Ok(json!({
                    "contents": [{
                        "uri": uri,
                        "mimeType": "{{ tmpl.mime_type }}",
                        "text": resp
                    }]
                }));
            }
            Err(e) => return Err(e),
        }
{% else %}
        return Ok(json!({
            "contents": [{
                "uri": uri,
                "mimeType": "{{ tmpl.mime_type }}",
                "text": ""
            }]
        }));
{% endif %}
    }
{% endfor %}

    Err(format!("Resource with URI '{}' not found", uri))
}
"###;
