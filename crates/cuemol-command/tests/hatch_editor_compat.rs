use cuemol_command::hatch_editor::{catalog, enabled, Environment, Values};
use serde_json::{json, Value};
use std::{
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

#[test]
fn hatch_editor_fields_and_dependencies_match_the_reference_typescript() {
    let root = std::env::var_os("CUEMOL2_REF_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../cuemol2"));
    let source = root.join("tritium/react-gui/src/renderer/data/hatchSpec.ts");
    if !source.is_file() {
        eprintln!("SKIP: current CueMol hatch editor reference is absent");
        return;
    }
    let mut cases = Vec::new();
    let mut expected = Vec::new();
    for section in ["layer", "tone", "ink"] {
        let base: Values = catalog()[section]
            .as_array()
            .unwrap()
            .iter()
            .map(|field| (field["key"].as_str().unwrap().into(), field["def"].clone()))
            .collect();
        for kind in ["line", "dot", "stipple"] {
            for ao_enabled in [false, true] {
                for base_is_albedo in [false, true] {
                    for changed in std::iter::once("").chain(base.keys().map(String::as_str)) {
                        for value in [0., 1., 2.] {
                            let mut values = base.clone();
                            if section == "layer" {
                                values.insert("kind".into(), json!(kind));
                            }
                            if values.get(changed).is_some_and(Value::is_number) {
                                values.insert(changed.into(), json!(value));
                            }
                            let env = Environment {
                                ao_enabled,
                                base_is_albedo,
                                density: 1.,
                                width_scale: 1.,
                                supersample: 3.,
                            };
                            let fields = base
                                .keys()
                                .map(|key| {
                                    (key.clone(), json!(enabled(section, key, &values, env)))
                                })
                                .collect::<Values>();
                            expected.push(Value::Object(fields));
                            cases.push(json!({"section":section,"values":values,"env":{"aoEnabled":ao_enabled,"baseIsAlbedo":base_is_albedo}}));
                        }
                    }
                }
            }
        }
    }
    let script = r#"
import {pathToFileURL} from 'node:url';
import {readFileSync} from 'node:fs';
const ref=await import(pathToFileURL(process.argv[1]));
const cases=JSON.parse(readFileSync(0,'utf8'));
const result=cases.map(({section,values,env})=>Object.fromEntries(Object.keys(values).filter(key=>key!=='kind').map(key=>[
  key,section==='layer'?ref.layerFieldEnabled(key,values):section==='tone'?ref.toneFieldEnabled(key,values,env):ref.inkFieldEnabled(key,values,env)
])));
console.log(JSON.stringify({catalog:{layer:ref.LAYER_FIELDS,tone:ref.TONE_FIELDS,ink:ref.INK_FIELDS},result}));
"#;
    let mut child = match Command::new("node")
        .args([
            "--experimental-strip-types",
            "--input-type=module",
            "-e",
            script,
        ])
        .arg(source)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("SKIP: Node is required for the optional TypeScript oracle");
            return;
        }
        Err(error) => panic!("Cannot run the TypeScript oracle: {error}"),
    };
    child
        .stdin
        .take()
        .unwrap()
        .write_all(serde_json::to_string(&cases).unwrap().as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let reference: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(&reference["catalog"], catalog());
    assert_eq!(reference["result"], json!(expected));
    eprintln!(
        "Compared {} current hatch-field dependency cases",
        cases.len()
    );
}
