use super::*;
use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
    io::Write,
    process::{Command as Process, Stdio},
};

#[test]
fn pure_grammar_matches_read_only_reference() {
    let Some(reference) = std::env::var_os("CUEMOL_CHIMERAX_REFERENCE") else {
        return;
    };
    let mut cases = vec![json!({"kind":"catalog","text":""})];
    for chains in [0, 4, 5, 249, 250] {
        for atoms in [199_999, 200_000, 999_999, 1_000_000] {
            for residues in [10, 4999, 5000] {
                cases.push(json!({"kind":"autoStyle","text":"","size":{"chains":chains,"atoms":atoms,"residues":residues,"oneSequence":true}}));
            }
        }
    }
    for value in [-20, 0, 20, 100, 120] {
        cases.push(json!({"kind":"alpha","text":value.to_string()}));
    }

    for command in catalog() {
        cases.push(json!({"kind":"bind","text":command.name}));
    }
    for text in [
        "c",
        "col red",
        "color #1/A red target a",
        "col byh",
        "color #1 #ff000080",
        "color #1 rgba(255,0,0,0.5)",
        "color site spring green",
        "color red target a target c",
        "color red TARGET a TARGET c",
        "set silh true",
        "set SILH true",
        "color red T a",
        "set silhouetteWidth 2 silhouetteColor blue",
        "hide #1 atoms",
        "hide #1 atoms,bonds",
        "hide #1 atoms, bonds",
        "select #1/A:10A-20B residues t",
        "select intersect",
        "select add objects #1",
        "select #1 residues on",
        "select red",
        "open \"C:\\Users\\x.pdb\" name \"two words\"",
        "open 'C:\\Users\\x.pdb'",
        "open \"unterminated",
        "move -y 2.5",
        "move 1,2,3 -1e2",
        "move 1..2,3,4 1",
        "turn x 90",
        "turn 1 2",
        "view matrix camera 1,0,0,0,1,0,0,0,1,0,0,0",
        "rainbow #1 chains palette blue:red",
        "rainbow objects #1 target ac",
        "distance #1:1@CA #2:1@CA decimalPlaces 2",
        "save output.data models #1 format pdb",
        "graphics sil t w 2",
        "surface hide #1",
        "surface h #1",
        "color #ff0000",
        "color #1234",
        "color #123456 red",
        "color protein & red",
        "color #1.2 red",
        "color @@bfactor>=20 red",
        "color red halfbond true",
        "style /A sph",
        "style /A b",
        "info models #1 attribute name",
        "alias x color $1 red; show $1",
        "alias delete x",
        "~alias all",
        "name site #1:10",
        "name frozen site",
        "name list builtins true",
        "unknown",
        "graphics",
    ] {
        cases.push(json!({"kind":"bind","text":text}));
    }
    for text in [
        "#1/A | #2/B",
        "#1,3-5/A,B:10-20,30@CA,CB",
        ":010A-020B",
        ":start-10",
        ":10-end",
        ":ALA",
        ":5HT",
        ":+10",
        ":-10--2",
        "@C*",
        "@C?",
        "@C[AB]",
        "@C[!AB]",
        "@C[",
        "@\"CA\"",
        "#ff0000",
        "#12345",
        "#123456",
        "#1.2",
        "#!1",
        "#*",
        "protein | ligand & ~solvent",
        "~protein & ligand",
        "#1 #2",
        "(protein | nucleic) & ~solvent",
        "protein &",
        "(protein",
        "protein red",
        "@@bfactor>=20",
        "@@occupancy<=0.5",
        "@@name==CA",
        "@@name!=C*",
        "@@element=Fe",
        "::number>10",
        "::number!=2",
        "::number=2.5",
        "::polymer_type=0",
        "::polymer_type!=2",
        "::ss_type=1",
        "::is_helix",
        "::^is_strand",
        "@@serial_number=1",
        "//chain_id=A",
        "##name=first",
        "##name!=first",
        "##name==First",
        "##name=f*",
        "##number=1",
        "sel",
        "site",
        "Fe",
        "ions",
        "backbone",
        "nucleic-acid",
        "#1/A @< 5",
        "#1/A :> 3.2",
        "(protein | ligand) @< 5",
        "#1/A @< 0",
        "#1/A @< -1",
        "#1/A @< 1e2",
    ] {
        cases.push(json!({"kind":"spec","text":text}));
    }
    for text in colors::tables().named.keys() {
        cases.push(json!({"kind":"color","text":text}));
    }
    for text in [
        "spring green target a",
        "light goldenrod yellow",
        "dark slate gray rest",
        "RED",
        "\"red\"",
        "notacolor",
        "#fff",
        "#abcd",
        "#123456",
        "#12345678",
        "#ffff00008000",
        "#ffff00008000ffff",
        "#12345",
        "#ggg",
        "100,50,0",
        "100,50,0,20",
        "2.3.4,0,0",
        "hsl(120,1.,0.5)",
        "hsl(2.0943951023931953rad,100%,50%)",
        "hsla(0.5turn,1,0.5,0.5)",
        "gray(50%,25%)",
        "gray(1turn)",
        "rgb(1,2,3) tail",
        "rgba(255,0,0,1turn)",
        "rgb(1e2,0,0)",
    ] {
        cases.push(json!({"kind":"color","text":text}));
    }
    for channel in [
        "-10", "0", "1", "10", "128", "255", "300", "0.5", "1.0", "50%", "100%",
    ] {
        for function in ["rgb", "rgba", "hsl", "hsla", "gray"] {
            let arguments = match function {
                "gray" => channel.to_string(),
                "rgba" | "hsla" => format!("{channel},{channel},{channel},0.5"),
                _ => format!("{channel},{channel},{channel}"),
            };
            cases.push(json!({"kind":"color","text":format!("{function}({arguments})")}));
        }
    }
    for text in [
        "# comment\ncolor #1 red; hide #2",
        "show #1; # comment; hide #2",
        "alias paint color $1 red; show $1",
        "alias list; color red",
        "alias delete paint; color blue",
        "color 'red;blue';show\nopen C:\\a.pdb",
        "open x\\\nshow",
        "open \"unterminated;show",
        "  ; \r\n \n",
        "color 'a\\'b';show",
    ] {
        cases.push(json!({"kind":"split","text":text}));
    }
    for text in [
        "rainbow",
        "gray",
        "redblue",
        "cyanmaroon",
        "cyan-gray-maroon",
        "lipophilicity",
        "^rainbow",
        "^red:green:blue",
        "0,red:0.2,blue:1,white",
        "0,red:0,white:1,black",
        "red:rgba(255,0,0,0.5)",
        "no-palette",
        "red:0.5,blue",
        "1,red:0,blue",
        "red:bad",
        "-.2,blue:1.2,red",
    ] {
        cases.push(json!({"kind":"palette","text":text}));
    }
    for text in ["A", "z", "1", "0", "AB", "aB", "主", "🧬", ""] {
        cases.push(json!({"kind":"chain","text":text}));
    }
    for number in 1..=24 {
        cases.push(json!({"kind":"model","text":number.to_string()}));
    }
    for value in [
        0.,
        1.,
        -1.,
        1.234567,
        1e-9,
        1e-6,
        1e-5,
        1e4,
        1e5,
        1e8,
        -1.234567e10,
        99999.9,
    ] {
        cases.push(json!({"kind":"precision","text":value.to_string()}));
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut child = Process::new("node")
        .arg(root.join("scripts/check-reference-chimerax.mjs"))
        .arg(reference)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(serde_json::to_string(&cases).unwrap().as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let expected: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    let names = BTreeSet::from(["site".to_string()]);
    let mut state = cuemol_desktop_core::DesktopAppState::new();
    let scene = state.new_scene("reference", true).result_scene;
    let mut models = Vec::new();
    for (i, name) in ["first", "second"].into_iter().enumerate() {
        let id = state
            .scene_manager_mut()
            .scene_mut(scene)
            .unwrap()
            .add_molecule(name, cuemol_molstr::MolCoord::new());
        state.set_molecule_selection_text(id, "none").unwrap();
        models.push(Model {
            id,
            number: i as u32 + 1,
            name: name.into(),
            molecule: true,
        });
    }
    let context = spec::Context {
        state: &state,
        models: &models,
        names: &names,
    };
    let mut failures = Vec::new();
    for (case, wanted) in cases.iter().zip(&expected) {
        let text = case["text"].as_str().unwrap();
        let actual = match case["kind"].as_str().unwrap() {
            "catalog"=>json!(catalog().iter().map(|c|json!({"name":c.name,"summary":c.summary,"args":c.args.iter().map(|a|json!({"name":a.name,"role":a.role})).collect::<Vec<_>>()})).collect::<Vec<_>>()),
            "autoStyle"=>json!(super::auto_style::choice(case["size"]["chains"].as_u64().unwrap() as usize,case["size"]["atoms"].as_u64().unwrap() as usize,case["size"]["residues"].as_u64().unwrap() as usize)),
            "alpha"=>json!(super::representation::alpha(text.parse().unwrap())),
            "chain" => {
                let (hex, exact) = super::palette::chain(text);
                json!({"hex":hex,"exact":exact})
            }
            "model" => {
                let (hex, exact) = super::palette::model(text.parse().unwrap());
                json!({"hex":hex,"exact":exact})
            }
            "precision" => json!(super::view::precision(text.parse().unwrap(), 5)),
            "palette" => match super::palette::Palette::read(text) {
                Err(error) => json!({"error":error}),
                Ok(p) => {
                    json!({"stops":p.0.iter().map(|(at,rgb)|json!({"at":at,"rgb":rgb})).collect::<Vec<_>>(),"colors":p.colors(7),"ranges":p.ranges(&(1..=130).map(|i|i.to_string()).collect::<Vec<_>>())})
                }
            },
            "split" => json!(syntax::split_lines(text)),
            "color" => colors::read(text).map_or(
                Value::Null,
                |color| json!({"hex":color.hex,"alpha":color.alpha,"rest":color.rest}),
            ),
            "bind" => match syntax::resolve(text, catalog()) {
                Err(error) => json!({"error":error}),
                Ok((command, _)) => match syntax::bind(command, text, &names) {
                    Ok(args) => json!({"name":command.name,"args":args}),
                    Err(error) => json!({"error":error}),
                },
            },
            "spec" => match spec::parse(text, &names) {
                Ok(None) => Value::Null,
                Err(error) => json!({"error":error.message,"pos":error.pos}),
                Ok(Some((expr, consumed))) => match models
                    .iter()
                    .map(|model| context.emit(&expr, model))
                    .collect::<Result<Vec<_>, _>>()
                {
                    Ok(expressions) => json!({"consumed":consumed,"expressions":expressions}),
                    Err(error) => json!({"error":error.message,"pos":error.pos}),
                },
            },
            _ => unreachable!(),
        };
        if case["kind"] == "bind" && text == "surface hidePatches" {
            assert!(wanted.get("error").is_some());
            assert_eq!(actual["name"], "surface hidePatches");
            continue;
        }
        if !equivalent(&actual, wanted) {
            failures.push(format!("{case}\n  Rust: {actual}\n  Reference: {wanted}"));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} cases differ:\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n")
    );
    eprintln!(
        "Checked {} ChimeraX cases (one documented camelCase command correction)",
        cases.len()
    );
}

fn equivalent(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Number(a), Value::Number(b)) => {
            (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() <= 1e-14
        }
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| equivalent(a, b))
        }
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len()
                && a.iter()
                    .all(|(key, a)| b.get(key).is_some_and(|b| equivalent(a, b)))
        }
        _ => left == right,
    }
}
