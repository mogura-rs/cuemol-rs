use cuemol_core::{CueMolError, Result};
use cuemol_desktop_core::DesktopAppState;
use serde_json::Value;

#[cfg(target_arch = "wasm32")]
pub fn calculate(_state: &mut DesktopAppState, _args: &Value) -> Result<Value> {
    Err(CueMolError::invalid_argument(
        "APBS runs in the native application; import the resulting DX map in the browser",
    ))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn calculate(state: &mut DesktopAppState, args: &Value) -> Result<Value> {
    use crate::commands::{scene, uid};
    use cuemol_desktop_core::{GuiApbsChargeMethod, GuiApbsOptions, GuiApbsPaths};
    let id = uid(args, "objId")?;
    let start = std::time::Instant::now();
    let method = if args["method"] == "internal" {
        GuiApbsChargeMethod::Internal
    } else {
        GuiApbsChargeMethod::Pdb2pqr
    };
    let options = GuiApbsOptions {
        charge_method: method,
        use_hydrogen: args["useHydrogen"] == true,
        use_npbe: args["nonLinear"] == true,
        temperature: args["temperature"].as_f64().unwrap_or(298.15),
        grid_spacing: args["gridSpacing"].as_f64().unwrap_or(1.),
        solvent_dielectric: args["waterDielec"].as_f64().unwrap_or(78.54),
        solute_dielectric: args["protDielec"].as_f64().unwrap_or(2.),
    };
    let work = tempfile::Builder::new().prefix("cuemol-apbs-").tempdir()?;
    let paths = GuiApbsPaths {
        pdb_path: work.path().join("molecule.pdb"),
        pqr_path: work.path().join("molecule.pqr"),
        input_path: work.path().join("apbs.in"),
        dx_path: work.path().join("potential.dx"),
    };
    state.prepare_apbs_files(
        id,
        args["selection"].as_str().unwrap_or("*"),
        &options,
        &paths,
    )?;
    let executable = crate::external::resolve(state, "apbs")?;
    if method == GuiApbsChargeMethod::Pdb2pqr {
        let pdb2pqr = crate::external::resolve(state, "pdb2pqr")?;
        let ff = args["forceField"]
            .as_str()
            .unwrap_or("PARSE")
            .to_ascii_uppercase();
        if !["AMBER", "CHARMM", "PARSE", "TYL06", "PEOEPB", "SWANSON"].contains(&ff.as_str()) {
            return Err(CueMolError::invalid_argument(
                "unsupported PDB2PQR force field",
            ));
        }
        crate::external::run(
            &pdb2pqr,
            &[
                "--keep-chain".into(),
                "--nodebump".into(),
                "--noopt".into(),
                "--ff".into(),
                ff,
                "molecule.pdb".into(),
                "molecule.pqr".into(),
            ],
            work.path(),
            1200,
        )?;
    }
    crate::external::run(&executable, &["apbs.in".into()], work.path(), 1200)?;
    if !paths.dx_path.is_file() {
        let parallel = work.path().join("potential-PE0.dx");
        if parallel.is_file() {
            std::fs::rename(parallel, &paths.dx_path)?;
        }
    }
    let name = args["name"]
        .as_str()
        .map(str::to_owned)
        .unwrap_or(state.suggested_apbs_potential_name(id)?);
    let map = state.open_apbs_potential_path(&paths.dx_path, &name)?;
    let _ = scene(state)?;
    Ok(
        serde_json::json!({"objectId":map.get(),"name":name,"seconds":start.elapsed().as_secs_f64()}),
    )
}
