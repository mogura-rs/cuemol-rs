use super::*;
use boa_engine::{
    object::{builtins::JsArray, FunctionObjectBuilder},
    Context, JsNativeError, JsObject, JsResult, JsString, JsValue, NativeFunction,
};
use serde_json::{json, Value};

#[derive(Default)]
pub(crate) struct Objects {
    next_id: u64,
    handles: BTreeMap<u64, Handle>,
    callbacks: BTreeSet<u64>,
    generation: u64,
    detached: Detached,
}

#[cfg(test)]
impl Objects {
    pub(crate) fn retained_counts(&self) -> (usize, usize) {
        (self.handles.len(), self.detached.molecules.len())
    }

    pub(crate) fn callback_count(&self) -> usize {
        self.callbacks.len()
    }
}

#[derive(Clone)]
struct Handle {
    value: JsValue,
    receiver: JsValue,
}

fn error(message: impl Into<String>) -> boa_engine::JsError {
    JsNativeError::typ().with_message(message.into()).into()
}

fn handle(id: &Value) -> JsResult<Handle> {
    let id = id
        .as_u64()
        .ok_or_else(|| error("invalid Python object handle"))?;
    script_env::with(|env| {
        env.services
            .borrow()
            .python_objects
            .handles
            .get(&id)
            .cloned()
    })
    .ok_or_else(|| error("Python object handle has expired"))
}

fn decode(value: &Value, context: &mut Context) -> JsResult<JsValue> {
    match value {
        Value::Object(map) if map.contains_key("$cuemol_callback") => {
            let id = map["$cuemol_callback"]
                .as_u64()
                .ok_or_else(|| error("invalid Python callback"))?;
            let generation = script_env::with_mut(|env| {
                let mut services = env.services.borrow_mut();
                services.python_objects.callbacks.insert(id);
                services.python_objects.generation
            });
            let function = FunctionObjectBuilder::new(
                context.realm(),
                NativeFunction::from_copy_closure(move |_, args, context| {
                    invoke_callback(id, generation, args, context)
                }),
            )
            .name(JsString::from("python_callback"))
            .constructor(false)
            .build();
            function.define_property_or_throw(
                JsString::from("_python_callback_id"),
                boa_engine::property::PropertyDescriptor::builder()
                    .value(id as f64)
                    .writable(false)
                    .enumerable(false)
                    .configurable(false),
                context,
            )?;
            function.define_property_or_throw(
                JsString::from("_python_generation"),
                boa_engine::property::PropertyDescriptor::builder()
                    .value(generation as f64)
                    .writable(false)
                    .enumerable(false)
                    .configurable(false),
                context,
            )?;
            Ok(function.into())
        }
        Value::Object(map) if map.contains_key("$cuemol_ref") => {
            Ok(handle(&map["$cuemol_ref"])?.value)
        }
        Value::Array(values) => {
            let values = values
                .iter()
                .map(|value| decode(value, context))
                .collect::<JsResult<Vec<_>>>()?;
            Ok(JsArray::from_iter(values, context).into())
        }
        Value::Object(values) => {
            let object = JsObject::with_object_proto(context.intrinsics());
            for (name, value) in values {
                let value = decode(value, context)?;
                object.create_data_property_or_throw(
                    JsString::from(name.as_str()),
                    value,
                    context,
                )?;
            }
            Ok(object.into())
        }
        value => JsValue::from_json(value, context),
    }
}

fn encode(value: JsValue, receiver: JsValue, context: &mut Context) -> JsResult<Value> {
    if let Some(object) = value.as_object() {
        if object.is_array() {
            let array = JsArray::from_object(object.clone())?;
            let mut values = Vec::new();
            for index in 0..array.length(context)? {
                values.push(encode(
                    array.get(index, context)?,
                    JsValue::undefined(),
                    context,
                )?);
            }
            return Ok(Value::Array(values));
        }
        let id = script_env::with_mut(|env| {
            let mut services = env.services.borrow_mut();
            let objects = &mut services.python_objects;
            objects.next_id += 1;
            let id = objects.next_id;
            objects.handles.insert(id, Handle { value, receiver });
            id
        });
        Ok(json!({"$cuemol_ref": id}))
    } else if value.is_undefined() {
        Ok(Value::Null)
    } else {
        value.to_json(context)
    }
}

fn dispatch(request: Value, context: &mut Context) -> JsResult<Value> {
    if let Some(ids) = request["release"].as_array() {
        script_env::with_mut(|env| {
            let mut services = env.services.borrow_mut();
            for id in ids.iter().filter_map(Value::as_u64) {
                services.python_objects.handles.remove(&id);
            }
        });
    }
    let operation = request["op"].as_str().unwrap_or_default();
    if operation == "release" {
        return Ok(Value::Null);
    }
    if operation == "bytes" {
        let encoded = request["value"]
            .as_str()
            .ok_or_else(|| error("missing byte data"))?;
        let decoded = cuemol_core::Base64InStream::new(encoded).into_bytes();
        let root = context
            .global_object()
            .get(JsString::from("cuemol"), context)?;
        let create = root
            .as_object()
            .ok_or_else(|| error("missing CueMol binding"))?
            .get(JsString::from("createObj"), context)?;
        let value = create
            .as_callable()
            .ok_or_else(|| error("missing object factory"))?
            .call(&root, &[JsString::from("ByteArray").into()], context)?;
        let mut bytes = ByteArray::new();
        bytes
            .init_code(ByteArray::UINT8, decoded.len())
            .map_err(|value| error(value.to_string()))?;
        bytes.as_mut_bytes().copy_from_slice(&decoded);
        script_bytes::replace(&value, bytes)?;
        return encode(value, JsValue::undefined(), context);
    }
    let target = if operation == "root" {
        Handle {
            value: context
                .global_object()
                .get(JsString::from("cuemol"), context)?,
            receiver: JsValue::undefined(),
        }
    } else {
        handle(&request["target"])?
    };
    let object = target
        .value
        .as_object()
        .ok_or_else(|| error("target is not an object"))?;
    match operation {
        "root" | "get" => {
            let name = request["name"]
                .as_str()
                .ok_or_else(|| error("missing property name"))?;
            if !object.has_property(JsString::from(name), context)? {
                return Err(error(format!("unknown CueMol property: {name}")));
            }
            let value = object.get(JsString::from(name), context)?;
            encode(value, target.value.clone(), context)
        }
        "set" => {
            let name = request["name"]
                .as_str()
                .ok_or_else(|| error("missing property name"))?;
            if !object.has_property(JsString::from(name), context)? {
                return Err(error(format!("unknown CueMol property: {name}")));
            }
            let value = decode(&request["value"], context)?;
            object.set(JsString::from(name), value, true, context)?;
            Ok(Value::Null)
        }
        "call" => {
            let args = request["arguments"]
                .as_array()
                .ok_or_else(|| error("missing arguments"))?;
            let args = args
                .iter()
                .map(|value| decode(value, context))
                .collect::<JsResult<Vec<_>>>()?;
            let value = object.call(&target.receiver, &args, context)?;
            encode(value, JsValue::undefined(), context)
        }
        "equal" => Ok(Value::Bool(JsValue::strict_equals(
            &target.value,
            &handle(&request["other"])?.value,
        ))),
        "hash" => {
            use std::hash::{Hash, Hasher};
            let mut hash = std::collections::hash_map::DefaultHasher::new();
            object.hash(&mut hash);
            Ok(Value::from(hash.finish()))
        }
        "string" => Ok(Value::String(
            target.value.to_string(context)?.to_std_string_escaped(),
        )),
        _ => Err(error("unknown Python binding operation")),
    }
}

pub(crate) fn request(value: Value, context: &mut Context) -> std::result::Result<Value, String> {
    dispatch(value, context).map_err(|error| error.to_string())
}

pub(crate) fn prepare(bridge: &mut cuemol_py::PythonBridge) -> JsResult<()> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let connection = bridge.connection().map_err(error)?;
        script_env::with_mut(|env| {
            let mut services = env.services.borrow_mut();
            if services
                .python_connection
                .as_ref()
                .is_none_or(|previous| !previous.same_process(&connection))
            {
                let generation = services.python_objects.generation + 1;
                services.python_objects = Objects {
                    generation,
                    ..Objects::default()
                };
                services.python_connection = Some(connection);
            }
        });
    }
    #[cfg(target_arch = "wasm32")]
    let _ = bridge;
    Ok(())
}

fn invoke_callback(
    id: u64,
    generation: u64,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let connection = script_env::with(|env| {
            let services = env.services.borrow();
            (services.python_objects.generation == generation)
                .then(|| services.python_connection.clone())
                .flatten()
        })
        .ok_or_else(|| error("Python callback interpreter is no longer available"))?;
        let values = args
            .iter()
            .map(|value| encode(value.clone(), JsValue::undefined(), context))
            .collect::<JsResult<Vec<_>>>()?;
        let (value, output) = connection
            .invoke_callback(id, Value::Array(values), &mut |value| {
                request(value, context)
            })
            .map_err(error)?;
        if !output.stdout().is_empty() {
            script_cmds::push(ScriptCmd::LogWrite {
                message: output.stdout().into(),
                newline: false,
            });
        }
        if !output.success() {
            return Err(error(output.stderr()));
        }
        decode(&value, context)
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (id, generation, args, context);
        Err(error("Python callbacks require a native build"))
    }
}

fn referenced_ids(
    services: &script_services::Shared,
    context: &mut Context,
) -> JsResult<BTreeMap<String, BTreeSet<u64>>> {
    let (mut pending, generation) = {
        let services = services.borrow();
        let mut values: Vec<_> = services
            .python_objects
            .handles
            .values()
            .flat_map(|handle| [handle.value.clone(), handle.receiver.clone()])
            .collect();
        values.extend(
            services
                .scene_listeners
                .values()
                .map(|(_, value)| JsValue::from(value.clone())),
        );
        values.extend(services.event_listener.iter().cloned().map(JsValue::from));
        (values, services.python_objects.generation)
    };
    let mut seen: Vec<JsObject> = Vec::new();
    let mut ids: BTreeMap<String, BTreeSet<u64>> = BTreeMap::new();
    while let Some(value) = pending.pop() {
        let Some(object) = value.as_object() else {
            continue;
        };
        if seen.contains(object) {
            continue;
        }
        seen.push(object.clone());
        for key in object.own_property_keys(context)? {
            let Some(value) = object
                .borrow()
                .properties()
                .get(&key)
                .and_then(|descriptor| descriptor.value().cloned())
            else {
                continue;
            };
            if let (boa_engine::property::PropertyKey::String(name), Some(id)) =
                (&key, value.as_number())
            {
                let name = name.to_std_string_escaped();
                if name != "_python_callback_id"
                    || object
                        .get(JsString::from("_python_generation"), context)?
                        .as_number()
                        == Some(generation as f64)
                {
                    ids.entry(name).or_default().insert(id as u64);
                }
            }
            if value.as_object().is_some_and(|object| {
                !object.is_callable()
                    || object
                        .borrow()
                        .properties()
                        .get(&JsString::from("_python_callback_id").into())
                        .is_some()
            }) {
                pending.push(value);
            }
        }
    }
    Ok(ids)
}

pub(crate) fn finish(context: &mut Context) -> JsResult<()> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let services = script_env::with(|env| env.services.clone());
        if services.borrow().python_objects.callbacks.is_empty() {
            return Ok(());
        }
        let ids = referenced_ids(&services, context)?;
        let live = ids.get("_python_callback_id");
        let (unused, connection) = {
            let services = services.borrow();
            (
                services
                    .python_objects
                    .callbacks
                    .iter()
                    .filter(|id| live.is_none_or(|ids| !ids.contains(id)))
                    .copied()
                    .collect::<Vec<_>>(),
                services.python_connection.clone(),
            )
        };
        if let Some(connection) = connection {
            connection
                .release_callbacks(&unused, &mut |value| request(value, context))
                .map_err(error)?;
            services
                .borrow_mut()
                .python_objects
                .callbacks
                .retain(|id| !unused.contains(id));
        }
    }
    #[cfg(target_arch = "wasm32")]
    let _ = context;
    Ok(())
}

#[derive(Default)]
struct Detached {
    molecules: Vec<ScriptMol>,
    morphs: BTreeMap<u64, MorphMol>,
    animation_managers: BTreeMap<u64, AnimMgr>,
    animations: BTreeMap<u64, AnimMotion>,
    maps: BTreeMap<u64, DensityMap>,
    lines: BTreeMap<u64, Vec<AtomIntrLine>>,
    lw: BTreeMap<u64, cuemol_lwview::LWObject>,
    native_exporters: BTreeMap<u64, ScriptNativeSceneExporter>,
    umbreon_exporters: BTreeMap<u64, UmbreonSceneExporter>,
    counters: [u64; 7],
}

pub(crate) fn restore(env: &mut ScriptEnv) {
    let saved = std::mem::take(&mut env.services.borrow_mut().python_objects.detached);
    for molecule in &saved.molecules {
        env.allocated_runtime_uids.insert(molecule.uid);
    }
    env.molecules.extend(saved.molecules);
    env.morphs.extend(saved.morphs);
    env.detached_animation_managers = saved.animation_managers;
    env.pending_animations = saved.animations;
    env.pending_density_maps = saved.maps;
    env.pending_atom_intr_lines = saved.lines;
    env.pending_lw_objects = saved.lw;
    env.native_scene_exporters = saved.native_exporters;
    env.umbreon_exporters = saved.umbreon_exporters;
    let [runtime, animation, map, lines, lw, native, umbreon] = saved.counters;
    env.next_runtime_uid = env.next_runtime_uid.max(runtime);
    env.next_detached_animation_manager_id = env.next_detached_animation_manager_id.max(animation);
    env.next_pending_density_map_id = env.next_pending_density_map_id.max(map);
    env.next_pending_atom_intr_id = env.next_pending_atom_intr_id.max(lines);
    env.next_pending_lw_object_id = env.next_pending_lw_object_id.max(lw);
    env.next_native_scene_exporter_id = env.next_native_scene_exporter_id.max(native);
    env.next_umbreon_exporter_id = env.next_umbreon_exporter_id.max(umbreon);
}

pub(crate) fn retain(env: &mut ScriptEnv, context: &mut Context) -> JsResult<()> {
    let ids = referenced_ids(&env.services, context)?;
    let has = |key: &str, id: &u64| ids.get(key).is_some_and(|ids| ids.contains(id));
    let molecule_ids: BTreeSet<_> = env
        .molecules
        .iter()
        .filter(|m| {
            m.scene_uid == 0
                && [
                    "uid",
                    "object_uid",
                    "parent_uid",
                    "_molecule_uid",
                    "_object_uid",
                    "_source_uid",
                ]
                .iter()
                .any(|key| has(key, &m.uid))
        })
        .map(|m| m.uid)
        .collect();
    let saved = Detached {
        molecules: env
            .molecules
            .iter()
            .filter(|m| molecule_ids.contains(&m.uid))
            .cloned()
            .collect(),
        morphs: env
            .morphs
            .iter()
            .filter(|(id, _)| molecule_ids.contains(id))
            .map(|(id, value)| (*id, value.clone()))
            .collect(),
        animation_managers: env
            .detached_animation_managers
            .iter()
            .filter(|(id, _)| has("_scene_uid", id))
            .map(|(id, value)| (*id, value.clone()))
            .collect(),
        animations: env
            .pending_animations
            .iter()
            .filter(|(id, _)| has("uid", id))
            .map(|(id, value)| (*id, value.clone()))
            .collect(),
        maps: env
            .pending_density_maps
            .iter()
            .filter(|(id, _)| has("_pending_map_id", id))
            .map(|(id, value)| (*id, value.clone()))
            .collect(),
        lines: env
            .pending_atom_intr_lines
            .iter()
            .filter(|(id, _)| has("_pending_atom_intr_id", id))
            .map(|(id, value)| (*id, value.clone()))
            .collect(),
        lw: env
            .pending_lw_objects
            .iter()
            .filter(|(id, _)| has("_pending_lw_id", id))
            .map(|(id, value)| (*id, value.clone()))
            .collect(),
        native_exporters: env
            .native_scene_exporters
            .iter()
            .filter(|(id, _)| has("_native_exporter_id", id))
            .map(|(id, value)| (*id, value.clone()))
            .collect(),
        umbreon_exporters: std::mem::take(&mut env.umbreon_exporters)
            .into_iter()
            .filter(|(id, _)| has("_exporter_id", id))
            .collect(),
        counters: [
            env.next_runtime_uid,
            env.next_detached_animation_manager_id,
            env.next_pending_density_map_id,
            env.next_pending_atom_intr_id,
            env.next_pending_lw_object_id,
            env.next_native_scene_exporter_id,
            env.next_umbreon_exporter_id,
        ],
    };
    env.services.borrow_mut().python_objects.detached = saved;
    Ok(())
}
