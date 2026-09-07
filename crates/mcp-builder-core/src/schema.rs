use crate::ast::*;
use serde_json::{Map, Value};

pub struct JsonSchemaSynthesizer;

impl JsonSchemaSynthesizer {
    pub fn synthesize_tool_input_schema(params: &[ParamSpec]) -> Value {
        let mut properties = Map::new();
        let mut required = Vec::new();

        for param in params {
            let schema = Self::synthesize_param_schema(param);
            properties.insert(param.name.clone(), schema);
            if param.required {
                required.push(Value::String(param.name.clone()));
            }
        }

        let mut root = Map::new();
        root.insert("type".to_string(), Value::String("object".to_string()));
        root.insert("properties".to_string(), Value::Object(properties));
        if !required.is_empty() {
            root.insert("required".to_string(), Value::Array(required));
        }

        Value::Object(root)
    }

    pub fn synthesize_param_schema(param: &ParamSpec) -> Value {
        let mut obj = Map::new();

        match &param.param_type {
            ParamType::String => {
                obj.insert("type".to_string(), Value::String("string".to_string()));
            }
            ParamType::Integer => {
                obj.insert("type".to_string(), Value::String("integer".to_string()));
            }
            ParamType::Number => {
                obj.insert("type".to_string(), Value::String("number".to_string()));
            }
            ParamType::Boolean => {
                obj.insert("type".to_string(), Value::String("boolean".to_string()));
            }
            ParamType::Array(inner) => {
                obj.insert("type".to_string(), Value::String("array".to_string()));
                let mut items = Map::new();
                items.insert("type".to_string(), Value::String(inner.as_json_schema_type().to_string()));
                obj.insert("items".to_string(), Value::Object(items));
            }
            ParamType::Object => {
                obj.insert("type".to_string(), Value::String("object".to_string()));
            }
        }

        if let Some(desc) = &param.description {
            obj.insert("description".to_string(), Value::String(desc.clone()));
        }

        if let Some(def) = &param.default {
            obj.insert("default".to_string(), def.clone());
        }

        if let Some(min) = param.minimum {
            if let Some(num) = serde_json::Number::from_f64(min) {
                obj.insert("minimum".to_string(), Value::Number(num));
            }
        }

        if let Some(max) = param.maximum {
            if let Some(num) = serde_json::Number::from_f64(max) {
                obj.insert("maximum".to_string(), Value::Number(num));
            }
        }

        if let Some(pat) = &param.pattern {
            obj.insert("pattern".to_string(), Value::String(pat.clone()));
        }

        if let Some(enums) = &param.enum_values {
            let enum_vals: Vec<Value> = enums.iter().map(|s| Value::String(s.clone())).collect();
            obj.insert("enum".to_string(), Value::Array(enum_vals));
        }

        Value::Object(obj)
    }

    pub fn synthesize_tools_list(tools: &[ToolSpec]) -> Value {
        let list: Vec<Value> = tools
            .iter()
            .map(|t| {
                let mut map = Map::new();
                map.insert("name".to_string(), Value::String(t.name.clone()));
                if let Some(desc) = &t.description {
                    map.insert("description".to_string(), Value::String(desc.clone()));
                }
                map.insert(
                    "inputSchema".to_string(),
                    Self::synthesize_tool_input_schema(&t.params),
                );
                Value::Object(map)
            })
            .collect();
        Value::Array(list)
    }

    pub fn synthesize_resources_list(
        resources: &[ResourceSpec],
        templates: &[ResourceTemplateSpec],
    ) -> Value {
        let mut list = Vec::new();

        for r in resources {
            let mut map = Map::new();
            map.insert("uri".to_string(), Value::String(r.uri.clone()));
            if let Some(name) = &r.name {
                map.insert("name".to_string(), Value::String(name.clone()));
            } else {
                map.insert("name".to_string(), Value::String(r.uri.clone()));
            }
            if let Some(desc) = &r.description {
                map.insert("description".to_string(), Value::String(desc.clone()));
            }
            if let Some(mime) = &r.mime_type {
                map.insert("mimeType".to_string(), Value::String(mime.clone()));
            }
            list.push(Value::Object(map));
        }

        for t in templates {
            let mut map = Map::new();
            map.insert("uriTemplate".to_string(), Value::String(t.uri_template.clone()));
            if let Some(name) = &t.name {
                map.insert("name".to_string(), Value::String(name.clone()));
            } else {
                map.insert("name".to_string(), Value::String(t.uri_template.clone()));
            }
            if let Some(desc) = &t.description {
                map.insert("description".to_string(), Value::String(desc.clone()));
            }
            if let Some(mime) = &t.mime_type {
                map.insert("mimeType".to_string(), Value::String(mime.clone()));
            }
            list.push(Value::Object(map));
        }

        Value::Array(list)
    }

    pub fn synthesize_prompts_list(prompts: &[PromptSpec]) -> Value {
        let list: Vec<Value> = prompts
            .iter()
            .map(|p| {
                let mut map = Map::new();
                map.insert("name".to_string(), Value::String(p.name.clone()));
                if let Some(desc) = &p.description {
                    map.insert("description".to_string(), Value::String(desc.clone()));
                }
                if !p.arguments.is_empty() {
                    let args: Vec<Value> = p
                        .arguments
                        .iter()
                        .map(|a| {
                            let mut amap = Map::new();
                            amap.insert("name".to_string(), Value::String(a.name.clone()));
                            amap.insert("required".to_string(), Value::Bool(a.required));
                            if let Some(desc) = &a.description {
                                amap.insert("description".to_string(), Value::String(desc.clone()));
                            }
                            Value::Object(amap)
                        })
                        .collect();
                    map.insert("arguments".to_string(), Value::Array(args));
                }
                Value::Object(map)
            })
            .collect();
        Value::Array(list)
    }
}
