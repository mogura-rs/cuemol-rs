use super::{FetchMetadata, FetchPlan};
use cuemol_core::{CueMolError, Result};
use serde_json::{json, Value};

pub(super) fn canonical_id(input: &str) -> Option<String> {
    let text = input.trim().to_ascii_lowercase();
    let digits = text.strip_prefix("emd").map_or(text.as_str(), |rest| {
        rest.strip_prefix('-')
            .or_else(|| rest.strip_prefix('_'))
            .unwrap_or(rest)
    });
    (digits.len() >= 4 && digits.bytes().all(|b| b.is_ascii_digit()))
        .then(|| format!("EMD-{digits}"))
}

pub(super) fn prepare(args: &Value) -> Result<FetchPlan> {
    let input = args["id"]
        .as_str()
        .ok_or_else(|| CueMolError::invalid_argument("An accession is required"))?;
    if let Some(id) = canonical_id(input) {
        return Ok(emdb_plan(args, &id, args["level"].is_null()));
    }
    let pdb = input.trim().to_ascii_uppercase();
    if pdb.len() != 4
        || !pdb.as_bytes()[0].is_ascii_digit()
        || !pdb.bytes().all(|b| b.is_ascii_alphanumeric())
    {
        return Err(CueMolError::invalid_argument(format!(
            "{input} is neither an EMDB code nor a PDB ID"
        )));
    }
    Ok(FetchPlan {
        url: "https://data.rcsb.org/graphql".into(),
        filename: "entry.json".into(),
        name: pdb.clone(),
        arguments: args.clone(),
        map_coefficients: false,
        emdb_id: None,
        request_body: Some(
            json!({"query":format!("{{entries(entry_ids:[\"{pdb}\"]){{rcsb_id exptl{{method}} rcsb_entry_container_identifiers{{emdb_ids}}}}}}") }),
        ),
        metadata: Some(FetchMetadata::Pdb { id: pdb }),
    })
}

fn emdb_plan(args: &Value, id: &str, metadata: bool) -> FetchPlan {
    let number = id.strip_prefix("EMD-").unwrap();
    let base = if args["source"] == "EBI" {
        "https://ftp.ebi.ac.uk/pub/databases/emdb/structures"
    } else {
        "https://files.wwpdb.org/pub/emdb/structures"
    };
    FetchPlan {
        url: if metadata {
            format!("https://www.ebi.ac.uk/emdb/api/entry/map/{id}")
        } else {
            format!("{base}/{id}/map/emd_{number}.map.gz")
        },
        filename: format!("emd_{number}.map.gz"),
        name: args["name"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| format!("emd_{number}")),
        arguments: args.clone(),
        map_coefficients: false,
        emdb_id: Some(id.into()),
        request_body: None,
        metadata: metadata.then(|| FetchMetadata::Emdb { id: id.into() }),
    }
}

fn numeric(value: &Value) -> Option<f64> {
    let number = match value {
        Value::Null => 0.,
        Value::Bool(v) => u8::from(*v) as f64,
        Value::String(text) if text.trim().is_empty() => 0.,
        Value::String(text) => text.trim().parse().ok()?,
        _ => value.as_f64()?,
    };
    number.is_finite().then_some(number)
}

pub(super) fn advance(plan: &FetchPlan, status: u16, bytes: &[u8]) -> Result<Option<FetchPlan>> {
    match &plan.metadata {
        Some(FetchMetadata::Pdb { id }) => {
            if !(200..300).contains(&status) {
                return Err(CueMolError::Io(format!("HTTP {status} from RCSB")));
            }
            let data: Value =
                serde_json::from_slice(bytes).map_err(|e| CueMolError::parse(e.to_string()))?;
            let entry = data["data"]["entries"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|entry| {
                    entry["rcsb_id"]
                        .as_str()
                        .is_some_and(|value| value.eq_ignore_ascii_case(id))
                        && entry["exptl"].is_array()
                })
                .ok_or_else(|| CueMolError::not_found(format!("No PDB entry {id}")))?;
            let emdb = entry["rcsb_entry_container_identifiers"]["emdb_ids"]
                .as_array()
                .and_then(|ids| ids.first())
                .and_then(Value::as_str)
                .and_then(canonical_id)
                .ok_or_else(|| CueMolError::not_found(format!("{id} has no EMDB map")))?;
            Ok(Some(emdb_plan(
                &plan.arguments,
                &emdb,
                plan.arguments["level"].is_null(),
            )))
        }
        Some(FetchMetadata::Emdb { id }) => {
            if status == 404 {
                return Err(CueMolError::not_found(format!("{id} not found")));
            }
            let mut args = plan.arguments.clone();
            if (200..300).contains(&status) {
                if let Ok(data) = serde_json::from_slice::<Value>(bytes) {
                    if let Some(contours) = data["map"]["contour_list"]["contour"].as_array() {
                        if let Some(level) = contours
                            .iter()
                            .find(|contour| contour["primary"] == true)
                            .or_else(|| contours.first())
                            .and_then(|contour| contour.get("level"))
                            .and_then(numeric)
                        {
                            args["level"] = json!(level);
                        }
                    }
                }
            }
            Ok(Some(emdb_plan(&args, id, false)))
        }
        None if (200..300).contains(&status) => Ok(None),
        None => Err(CueMolError::Io(format!("Download returned HTTP {status}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_sequence_retains_primary_level_and_supports_fallbacks() {
        for text in ["EMD-11638", "emd_11638", "emd11638", "11638"] {
            assert_eq!(canonical_id(text).as_deref(), Some("EMD-11638"));
        }
        let plan = prepare(&json!({"id":"7a4m","source":"EBI"})).unwrap();
        assert!(plan.request_body.is_some());
        let entry = json!({"data":{"entries":[{"rcsb_id":"7A4M","exptl":[{"method":"ELECTRON MICROSCOPY"}],"rcsb_entry_container_identifiers":{"emdb_ids":["EMD-11638"]}}]}});
        let plan = advance(&plan, 200, &serde_json::to_vec(&entry).unwrap())
            .unwrap()
            .unwrap();
        assert!(plan.url.ends_with("/entry/map/EMD-11638"));
        let map = json!({"map":{"contour_list":{"contour":[{"level":3.},{"primary":true,"level":"1.25"}]}}});
        let final_plan = advance(&plan, 200, &serde_json::to_vec(&map).unwrap())
            .unwrap()
            .unwrap();
        assert!(final_plan.url.starts_with("https://ftp.ebi.ac.uk/"));
        assert!(final_plan.url.ends_with("/EMD-11638/map/emd_11638.map.gz"));
        assert_eq!(final_plan.arguments["level"], 1.25);
        assert!(advance(&final_plan, 200, &[]).unwrap().is_none());
        assert!(advance(&plan, 404, &[]).is_err());
        for status in [0, 200, 503] {
            let fallback = advance(&plan, status, b"invalid").unwrap().unwrap();
            assert!(fallback.metadata.is_none());
            assert!(fallback.arguments["level"].is_null());
        }
        assert!(prepare(&json!({"id":"EMD-11638","level":0.}))
            .unwrap()
            .metadata
            .is_none());
        assert!(prepare(&json!({"id":"not-an-id"})).is_err());
        assert!(canonical_id("emd--1234").is_none());
    }
}
