use crate::commands::call_tool;
use cuemol_desktop_core::DesktopAppState;
use cuemol_qsys::Camera;
use serde_json::{json, Value};
use std::sync::OnceLock;

pub fn tools_list() -> &'static Vec<Value> {
    static TOOLS: OnceLock<Vec<Value>> = OnceLock::new();
    TOOLS.get_or_init(|| {
        serde_json::from_str(include_str!("tools.json")).expect("valid bundled tool schemas")
    })
}

pub fn tool_is_mutating(name: &str) -> bool {
    tools_list()
        .iter()
        .find(|tool| tool["name"] == name)
        .is_some_and(|tool| tool["annotations"]["readOnlyHint"] == false)
}

pub fn tool_records_history(name: &str) -> bool {
    tool_is_mutating(name) && !matches!(name, "set_frame" | "animate" | "clear_undo")
}

pub fn validate_args(name: &str, args: &Value) -> Result<(), String> {
    let tool = tools_list()
        .iter()
        .find(|tool| tool["name"] == name)
        .ok_or_else(|| format!("Unknown tool: {name}"))?;
    let schema = &tool["inputSchema"];
    let values = args.as_object().ok_or("Arguments must be an object")?;
    for required in schema["required"].as_array().into_iter().flatten() {
        let key = required.as_str().unwrap();
        if !values.contains_key(key) {
            return Err(format!("Missing argument: {key}"));
        }
    }
    let properties = schema["properties"].as_object().unwrap();
    for (key, value) in values {
        let spec = properties
            .get(key)
            .ok_or_else(|| format!("Unknown argument: {key}"))?;
        let allowed = if let Some(types) = spec["type"].as_array() {
            types.iter().filter_map(Value::as_str).collect()
        } else {
            vec![spec["type"].as_str().unwrap()]
        };
        let valid = allowed.iter().any(|kind| match *kind {
            "null" => value.is_null(),
            "integer" => value.is_i64() || value.is_u64(),
            "number" => value.is_number(),
            "string" => value.is_string(),
            "boolean" => value.is_boolean(),
            "array" => value.is_array(),
            "object" => value.is_object(),
            _ => false,
        });
        if !valid {
            return Err(format!("Invalid type for {key}"));
        }
        if let Some(choices) = spec["enum"].as_array() {
            if !choices.contains(value) {
                return Err(format!("Invalid choice for {key}"));
            }
        }
        if let Some(items) = value.as_array() {
            if spec["minItems"]
                .as_u64()
                .is_some_and(|min| items.len() < min as usize)
                || spec["maxItems"]
                    .as_u64()
                    .is_some_and(|max| items.len() > max as usize)
            {
                return Err(format!("Invalid array length for {key}"));
            }
            let kind = spec["items"]["type"].as_str().unwrap_or("");
            if items.iter().any(|item| match kind {
                "string" => !item.is_string(),
                "integer" => !item.is_i64() && !item.is_u64(),
                "number" => !item.is_number(),
                _ => false,
            }) {
                return Err(format!("Invalid array item for {key}"));
            }
        }
    }
    Ok(())
}

pub struct Protocol {
    record_history: bool,
    initialized: bool,
    undo: Vec<(DesktopAppState, Camera)>,
    redo: Vec<(DesktopAppState, Camera)>,
}

impl Protocol {
    pub fn new() -> Self {
        Self {
            initialized: false,
            undo: Vec::new(),
            redo: Vec::new(),
            record_history: true,
        }
    }
    pub fn initialized(&self) -> bool {
        self.initialized
    }

    pub fn without_history() -> Self {
        Self {
            record_history: false,
            ..Self::new()
        }
    }

    pub fn handle(
        &mut self,
        request: Value,
        state: &mut DesktopAppState,
        camera: &mut Camera,
    ) -> Option<Value> {
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        if request["jsonrpc"] != "2.0"
            || !request["method"].is_string()
            || !(id.is_null() || id.is_string() || id.is_number())
        {
            return Some(rpc_error(id, -32600, "Invalid JSON-RPC request"));
        }
        let method = request["method"].as_str().unwrap();
        if request.get("id").is_none() {
            return None;
        }
        if method == "initialize" {
            self.initialized = true;
            let offered = request["params"]["protocolVersion"]
                .as_str()
                .unwrap_or("2025-11-25");
            let version = match offered {
                "2024-11-05" | "2025-03-26" | "2025-06-18" => offered,
                _ => "2025-11-25",
            };
            return Some(json!({"jsonrpc":"2.0","id":id,"result":{
                "protocolVersion":version,"capabilities":{"tools":{}},
                "serverInfo":{"name":"cuemol","version":env!("CARGO_PKG_VERSION")},
                "instructions":"Use get_scene_state to discover object and renderer IDs. Selections use CueMol syntax. Coordinates and camera lengths are in angstroms. Trajectory frame indices start at zero."
            }}));
        }
        if method == "ping" {
            return Some(json!({"jsonrpc":"2.0","id":id,"result":{}}));
        }
        if !self.initialized {
            return Some(rpc_error(id, -32002, "Initialize the server first"));
        }
        let result = match method {
            "tools/list" => {
                if request["params"]["cursor"]
                    .as_str()
                    .is_some_and(|cursor| !cursor.is_empty())
                {
                    return Some(rpc_error(id, -32602, "Invalid cursor"));
                }
                json!({"tools":tools_list()})
            }
            "tools/call" => {
                let Some(name) = request["params"]["name"].as_str() else {
                    return Some(rpc_error(id, -32602, "Missing tool name"));
                };
                let args = request["params"]
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                match validate_args(name, &args)
                    .and_then(|()| self.call(name, &args, state, camera))
                {
                    Ok(mut value) => {
                        let image = value
                            .as_object_mut()
                            .and_then(|object| object.remove("_image"));
                        let mut content = vec![
                            json!({"type":"text","text":json!({"ok":true,"result":value}).to_string()}),
                        ];
                        if let Some(image) = image {
                            content
                                .push(json!({"type":"image","data":image,"mimeType":"image/png"}));
                        }
                        json!({"content":content,"isError":false})
                    }
                    Err(error) => {
                        json!({"content":[{"type":"text","text":json!({"ok":false,"error":error}).to_string()}],"isError":true})
                    }
                }
            }
            _ => return Some(rpc_error(id, -32601, "Method not found")),
        };
        Some(json!({"jsonrpc":"2.0","id":id,"result":result}))
    }

    fn call(
        &mut self,
        name: &str,
        args: &Value,
        state: &mut DesktopAppState,
        camera: &mut Camera,
    ) -> Result<Value, String> {
        if name == "clear_undo" {
            self.undo.clear();
            self.redo.clear();
            return Ok(json!({"cleared":true}));
        }
        if name == "undo" || name == "redo" {
            let (from, to) = if name == "undo" {
                (&mut self.undo, &mut self.redo)
            } else {
                (&mut self.redo, &mut self.undo)
            };
            let Some((previous, view)) = from.pop() else {
                return Err(format!("Nothing to {name}"));
            };
            to.push((
                std::mem::replace(state, previous),
                std::mem::replace(camera, view),
            ));
            return Ok(json!({"changed":true}));
        }
        crate::advance_animation(state, camera).map_err(|e| e.to_string())?;
        let before = tool_records_history(name).then(|| (state.clone(), camera.clone()));
        match call_tool(state, camera, name, args) {
            Ok(value) => {
                if let Some(before) = before.filter(|_| self.record_history) {
                    if self.undo.len() == 20 {
                        self.undo.remove(0);
                    }
                    self.undo.push(before);
                    self.redo.clear();
                }
                Ok(value)
            }
            Err(error) => {
                if let Some((previous, view)) = before {
                    *state = previous;
                    *camera = view;
                }
                Err(error.to_string())
            }
        }
    }
}

pub fn rpc_error(id: Value, code: i32, message: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
}

impl Default for Protocol {
    fn default() -> Self {
        Self::new()
    }
}
