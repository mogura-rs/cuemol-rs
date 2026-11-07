//! Portable RCSB search and entry metadata protocol.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const MAX_METADATA_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "lowercase")]
pub enum Accession {
    Pdb(String),
    Emdb(String),
}

impl Accession {
    pub fn parse(input: &str) -> Option<Self> {
        let text = input.trim().to_ascii_lowercase();
        if text.len() == 4
            && text.as_bytes()[0].is_ascii_digit()
            && text.bytes().all(|b| b.is_ascii_alphanumeric())
        {
            return Some(Self::Pdb(text));
        }
        let rest = text.strip_prefix("emd")?;
        let digits = rest
            .strip_prefix('-')
            .or_else(|| rest.strip_prefix('_'))
            .unwrap_or(rest);
        if digits.len() >= 4 && digits.bytes().all(|b| b.is_ascii_digit()) {
            Some(Self::Emdb(format!("EMD-{digits}")))
        } else {
            None
        }
    }

    pub fn id(&self) -> &str {
        match self {
            Self::Pdb(id) | Self::Emdb(id) => id,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntryKind {
    Xtal,
    Em,
    Other,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryInfo {
    pub id: String,
    pub title: String,
    pub methods: Vec<String>,
    pub resolution: Option<f64>,
    pub emdb_ids: Vec<String>,
    pub pdb_format_available: bool,
    pub kind: EntryKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmdbMapInfo {
    pub emdb_id: String,
    pub title: String,
    pub recommended_level: Option<f64>,
    pub size_bytes: Option<f64>,
    pub dims: Option<[f64; 3]>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmMapRequest {
    pub emdb_id: String,
    pub source: String,
    pub level: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadChoice {
    pub accession: Accession,
    pub coordinates: bool,
    pub coordinate_source: String,
    pub two_fofc: bool,
    pub fofc: bool,
    pub map_source: String,
    pub em_maps: Vec<EmMapRequest>,
}

#[derive(Debug, Clone)]
pub struct DownloadFile {
    pub kind: cuemol_desktop_core::GuiPdbFileKind,
    pub url: String,
    pub filename: String,
    pub name: String,
    pub level: Option<f64>,
}

impl DownloadChoice {
    pub fn files(&self) -> Result<Vec<DownloadFile>, String> {
        use cuemol_desktop_core::GuiPdbFileKind as Kind;
        let mut files = Vec::new();
        if self.coordinates || self.two_fofc || self.fofc {
            let Accession::Pdb(id) = &self.accession else {
                return Err(
                    "EMDB entries do not provide coordinates or crystallographic maps".into(),
                );
            };
            if Accession::parse(id) != Some(Accession::Pdb(id.clone())) {
                return Err("Invalid PDB accession".into());
            }
            if self.coordinates {
                let ext = match self.coordinate_source.as_str() {
                    "RCSB_CIF" => "cif",
                    "RCSB_PDB" => "pdb",
                    _ => return Err("Unknown coordinate source".into()),
                };
                files.push(DownloadFile {
                    kind: Kind::Coordinates,
                    url: format!("https://files.rcsb.org/download/{id}.{ext}"),
                    filename: format!("{id}.{ext}"),
                    name: id.clone(),
                    level: None,
                });
            }
            for (on, kind, suffix, label) in [
                (self.two_fofc, Kind::TwoFoFc, "2fo-fc", "2fofc"),
                (self.fofc, Kind::FoFc, "fo-fc", "fofc"),
            ] {
                if !on {
                    continue;
                }
                let (url,ext) = match self.map_source.as_str() {
                    "RCSB_CIF"=>(format!("https://files.rcsb.org/pub/pdb/validation_reports/{}/{id}/{id}_validation_{suffix}_map_coef.cif.gz",&id[1..3]),"cif.gz"),
                    "EBI_MTZ"=>(format!("https://www.ebi.ac.uk/pdbe/coordinates/files/{id}_map.mtz"),"mtz"),
                    _=>return Err("Unknown crystallographic map source".into()),
                };
                files.push(DownloadFile {
                    kind,
                    url,
                    filename: format!("{id}_{label}.{ext}"),
                    name: format!("{id}_{label}"),
                    level: None,
                });
            }
        }
        for map in &self.em_maps {
            let Some(Accession::Emdb(id)) = Accession::parse(&map.emdb_id) else {
                return Err("Invalid EMDB accession".into());
            };
            if map.level.is_some_and(|n| !n.is_finite()) {
                return Err("EMDB level must be finite".into());
            }
            let base = match map.source.as_str() {
                "WWPDB" => "https://files.wwpdb.org/pub/emdb/structures",
                "EBI" => "https://ftp.ebi.ac.uk/pub/databases/emdb/structures",
                _ => return Err("Unknown EMDB source".into()),
            };
            let number = id.strip_prefix("EMD-").unwrap();
            let name = format!("emd_{number}");
            let filename = format!("{name}.map.gz");
            files.push(DownloadFile {
                kind: Kind::Emdb,
                url: format!("{base}/{id}/map/{filename}"),
                filename,
                name,
                level: map.level,
            });
        }
        if files.is_empty() {
            return Err("Select coordinates or at least one map".into());
        }
        Ok(files)
    }
}

pub fn configure_downloaded(
    state: &mut cuemol_desktop_core::DesktopAppState,
    camera: &mut cuemol_qsys::Camera,
    loaded: &cuemol_desktop_core::GuiLoadedPath,
    file: &DownloadFile,
) -> cuemol_core::Result<()> {
    use cuemol_desktop_core::{GuiLoadedPath, GuiPdbFileKind as Kind, GuiRendererSetup};
    match loaded {
        GuiLoadedPath::Object { object_uid, .. } if file.kind == Kind::Coordinates => {
            state.set_object_common_properties(*object_uid, file.name.clone(), true, false)?;
            crate::commands::fit(state, camera, Some(*object_uid), None, true)?;
        }
        GuiLoadedPath::Map { map_uid, .. } if file.kind != Kind::Coordinates => {
            if file.kind == Kind::Emdb {
                state.set_density_map_sampling(*map_uid, 1, "em")?;
            }
            let renderer_type = if file.kind == Kind::Emdb {
                "isosurf"
            } else {
                "contour"
            };
            state.configure_loaded_bytes(
                loaded,
                &GuiRendererSetup {
                    object_name: file.name.clone(),
                    renderer_type: renderer_type.into(),
                    renderer_name: renderer_type.into(),
                    selection: None,
                    recenter: false,
                    map_center_policy: None,
                },
                None,
            )?;
            let renderer = state.density_map_summary(*map_uid).unwrap().renderers[0].uid;
            if file.kind == Kind::Emdb {
                if let Some(level) = file.level {
                    crate::nodes::set_renderer_property(state, renderer, "use_abslevel", "true")?;
                    crate::nodes::set_renderer_property(
                        state,
                        renderer,
                        "level",
                        &level.to_string(),
                    )?;
                }
                crate::file_commands::apply_map_center_policy(
                    state,
                    camera,
                    *map_uid,
                    &json!({"mapCenterPolicy":"moveViewCenter"}),
                )?;
            } else {
                let diff = file.kind == Kind::FoFc;
                crate::nodes::set_renderer_property(
                    state,
                    renderer,
                    "siglevel",
                    if diff { "3" } else { "1" },
                )?;
                crate::nodes::set_renderer_property(
                    state,
                    renderer,
                    "color",
                    if diff { "green" } else { "blue" },
                )?;
                if diff {
                    let negative = state.add_density_renderer(*map_uid, "contour")?;
                    crate::nodes::set_renderer_property(state, negative, "siglevel", "-3")?;
                    crate::nodes::set_renderer_property(state, negative, "color", "red")?;
                }
            }
        }
        _ => {
            return Err(cuemol_core::CueMolError::invalid_argument(
                "Downloaded entry has an unexpected file type",
            ))
        }
    }
    Ok(())
}

pub fn normalize_history(input: &str) -> Option<String> {
    let text = input.split_whitespace().collect::<Vec<_>>().join(" ");
    let value = Accession::parse(&text)
        .map(|a| a.id().to_owned())
        .unwrap_or(text);
    (!value.is_empty() && value.encode_utf16().count() <= 200).then_some(value)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchMethod {
    #[default]
    All,
    Xray,
    Em,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchSort {
    #[default]
    Score,
    Resolution,
    Date,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Query {
    Entries {
        pdb_ids: Vec<String>,
    },
    Search {
        query: String,
        method: SearchMethod,
        sort: SearchSort,
        start: usize,
        rows: usize,
    },
    Emdb {
        emdb_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Response {
    Entries {
        entries: Vec<EntryInfo>,
    },
    Search {
        total: usize,
        entries: Vec<EntryInfo>,
    },
    Emdb {
        info: EmdbMapInfo,
    },
}

#[derive(Debug, Clone)]
pub struct Request {
    pub url: String,
    pub body: Option<Value>,
    pub optional: bool,
}

enum Stage {
    Entries,
    Search,
    RankedEntries { total: usize, ids: Vec<String> },
    Map { id: String },
    Admin { info: EmdbMapInfo },
    Done,
}

pub struct Plan {
    stage: Stage,
    request: Option<Request>,
    result: Option<Response>,
}

fn entries_request(ids: &[String]) -> Request {
    let list = ids
        .iter()
        .map(|s| Value::String(s.to_ascii_uppercase()))
        .collect::<Vec<_>>();
    Request {
        url: "https://data.rcsb.org/graphql".into(),
        body: Some(
            json!({"query":format!("{{entries(entry_ids:{}){{rcsb_id struct{{title}} exptl{{method}} rcsb_entry_info{{resolution_combined}} rcsb_entry_container_identifiers{{emdb_ids}} pdbx_database_status{{pdb_format_compatible}}}}}}",json!(list))}),
        ),
        optional: false,
    }
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

pub fn parse_entries(data: &Value) -> Vec<EntryInfo> {
    data["data"]["entries"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let id = entry["rcsb_id"].as_str()?.to_ascii_lowercase();
            let methods = entry["exptl"]
                .as_array()?
                .iter()
                .filter_map(|m| m["method"].as_str())
                .map(str::to_owned)
                .collect::<Vec<_>>();
            let emdb_ids = strings(&entry["rcsb_entry_container_identifiers"]["emdb_ids"]);
            let upper = methods
                .iter()
                .map(|m| m.to_ascii_uppercase())
                .collect::<Vec<_>>();
            let kind = if !emdb_ids.is_empty() {
                EntryKind::Em
            } else if upper
                .iter()
                .any(|m| m.contains("X-RAY") || m.contains("NEUTRON"))
            {
                EntryKind::Xtal
            } else if upper.iter().any(|m| m.contains("ELECTRON MICROSCOPY")) {
                EntryKind::Em
            } else {
                EntryKind::Other
            };
            Some(EntryInfo {
                id,
                title: entry["struct"]["title"].as_str().unwrap_or_default().into(),
                methods,
                resolution: entry["rcsb_entry_info"]["resolution_combined"]
                    .get(0)
                    .and_then(Value::as_f64),
                emdb_ids,
                pdb_format_available: entry["pdbx_database_status"]["pdb_format_compatible"] != "N",
                kind,
            })
        })
        .collect()
}

fn numeric(value: Option<&Value>) -> Option<f64> {
    let number = match value? {
        Value::Null => 0.,
        Value::Bool(v) => u8::from(*v) as f64,
        Value::String(text) if text.trim().is_empty() => 0.,
        Value::String(text) => text.trim().parse().ok()?,
        value => value.as_f64()?,
    };
    number.is_finite().then_some(number)
}

pub fn parse_emdb(id: &str, data: &Value) -> EmdbMapInfo {
    let map = &data["map"];
    let contours = map["contour_list"]["contour"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|v| v.is_object())
        .collect::<Vec<_>>();
    let contour = contours
        .iter()
        .find(|v| v["primary"] == true)
        .or_else(|| contours.first());
    let dims = (|| {
        let value = &map["dimensions"];
        let dims = [
            numeric(value.get("col"))?,
            numeric(value.get("row"))?,
            numeric(value.get("sec"))?,
        ];
        dims.iter().all(|&n| n > 0.).then_some(dims)
    })();
    EmdbMapInfo {
        emdb_id: id.into(),
        title: String::new(),
        recommended_level: contour.and_then(|c| numeric(c.get("level"))),
        size_bytes: numeric(map.get("size_kbytes"))
            .filter(|n| *n > 0.)
            .map(|n| n * 1000.),
        dims,
    }
}

impl Plan {
    pub fn new(query: Query) -> Result<Self, String> {
        let mut plan = Self {
            stage: Stage::Done,
            request: None,
            result: None,
        };
        match query {
            Query::Entries { pdb_ids } => {
                if pdb_ids.len() > 100 {
                    return Err("Request at most 100 PDB entries at once".into());
                }
                if pdb_ids.is_empty() {
                    plan.result = Some(Response::Entries {
                        entries: Vec::new(),
                    });
                } else {
                    plan.stage = Stage::Entries;
                    plan.request = Some(entries_request(&pdb_ids));
                }
            }
            Query::Search {
                query,
                method,
                sort,
                start,
                rows,
            } => {
                let query = query.trim();
                if query.is_empty() {
                    plan.result = Some(Response::Search {
                        total: 0,
                        entries: Vec::new(),
                    });
                } else {
                    if !(1..=100).contains(&rows) {
                        return Err("Search page size must be between 1 and 100".into());
                    }
                    let mut node = json!({"type":"terminal","service":"full_text","parameters":{"value":query}});
                    if method != SearchMethod::All {
                        let value = if method == SearchMethod::Xray {
                            "X-RAY DIFFRACTION"
                        } else {
                            "ELECTRON MICROSCOPY"
                        };
                        node = json!({"type":"group","logical_operator":"and","nodes":[node,{"type":"terminal","service":"text","parameters":{"attribute":"exptl.method","operator":"exact_match","value":value}}]});
                    }
                    let mut options = json!({"paginate":{"start":start,"rows":rows}});
                    match sort {
                        SearchSort::Score => {}
                        SearchSort::Resolution => {
                            options["sort"] = json!([{"sort_by":"rcsb_entry_info.resolution_combined","direction":"asc"}])
                        }
                        SearchSort::Date => {
                            options["sort"] = json!([{"sort_by":"rcsb_accession_info.initial_release_date","direction":"desc"}])
                        }
                    }
                    plan.stage = Stage::Search;
                    plan.request = Some(Request {
                        url: "https://search.rcsb.org/rcsbsearch/v2/query".into(),
                        body: Some(
                            json!({"query":node,"return_type":"entry","request_options":options}),
                        ),
                        optional: false,
                    });
                }
            }
            Query::Emdb { emdb_id } => {
                let Some(Accession::Emdb(id)) = Accession::parse(&emdb_id) else {
                    return Err("Invalid EMDB accession".into());
                };
                plan.request = Some(Request {
                    url: format!("https://www.ebi.ac.uk/emdb/api/entry/map/{id}"),
                    body: None,
                    optional: false,
                });
                plan.stage = Stage::Map { id };
            }
        }
        Ok(plan)
    }

    pub fn request(&self) -> Option<&Request> {
        self.request.as_ref()
    }
    pub fn result(self) -> Result<Response, String> {
        self.result
            .ok_or_else(|| "Metadata request has not completed".into())
    }

    pub fn advance(&mut self, status: u16, bytes: &[u8]) -> Result<(), String> {
        let optional = self.request.as_ref().is_some_and(|r| r.optional);
        if matches!(self.stage, Stage::Done) {
            return Err("Metadata request already completed".into());
        }
        if !(200..300).contains(&status) && !optional {
            return Err(if status == 404 {
                "Entry not found".into()
            } else {
                format!("Metadata request returned HTTP {status}")
            });
        }
        if bytes.len() > MAX_METADATA_BYTES && !optional {
            return Err("Metadata exceeds 4 MiB".into());
        }
        let data = if status == 204
            || !(200..300).contains(&status)
            || (optional && bytes.len() > MAX_METADATA_BYTES)
        {
            Value::Null
        } else {
            match serde_json::from_slice(bytes) {
                Ok(v) => v,
                Err(_) if optional => Value::Null,
                Err(e) => return Err(format!("Invalid metadata JSON: {e}")),
            }
        };
        self.request = None;
        match std::mem::replace(&mut self.stage, Stage::Done) {
            Stage::Entries => {
                self.result = Some(Response::Entries {
                    entries: parse_entries(&data),
                })
            }
            Stage::Search => {
                let total = data["total_count"]
                    .as_u64()
                    .and_then(|n| usize::try_from(n).ok())
                    .unwrap_or(0);
                let ids = data["result_set"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|v| v["identifier"].as_str())
                    .map(str::to_ascii_lowercase)
                    .collect::<Vec<_>>();
                if ids.is_empty() {
                    self.result = Some(Response::Search {
                        total,
                        entries: Vec::new(),
                    });
                } else {
                    if ids.len() > 100 {
                        return Err("Search returned too many entries".into());
                    }
                    self.request = Some(entries_request(&ids));
                    self.stage = Stage::RankedEntries { total, ids };
                }
            }
            Stage::RankedEntries { total, ids } => {
                let mut by_id = parse_entries(&data)
                    .into_iter()
                    .map(|e| (e.id.clone(), e))
                    .collect::<std::collections::HashMap<_, _>>();
                let entries = ids.into_iter().filter_map(|id| by_id.remove(&id)).collect();
                self.result = Some(Response::Search { total, entries });
            }
            Stage::Map { id } => {
                let info = parse_emdb(&id, &data);
                self.request = Some(Request {
                    url: format!("https://www.ebi.ac.uk/emdb/api/entry/admin/{id}"),
                    body: None,
                    optional: true,
                });
                self.stage = Stage::Admin { info };
            }
            Stage::Admin { mut info } => {
                info.title = data["admin"]["title"].as_str().unwrap_or_default().into();
                self.result = Some(Response::Emdb { info });
            }
            Stage::Done => unreachable!(),
        }
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn execute(query: Query) -> Result<Response, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(15)))
        .build()
        .into();
    execute_with(query, |request| {
        use std::io::Read;
        let response = if let Some(body) = &request.body {
            agent.post(&request.url).send_json(body)
        } else {
            agent.get(&request.url).call()
        };
        match response {
            Ok(mut response) => {
                let status = response.status().as_u16();
                let mut bytes = Vec::new();
                response
                    .body_mut()
                    .as_reader()
                    .take(MAX_METADATA_BYTES as u64 + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|e| e.to_string())?;
                Ok((status, bytes))
            }
            Err(ureq::Error::StatusCode(status)) => Ok((status, Vec::new())),
            Err(e) => Err(e.to_string()),
        }
    })
}

pub fn execute_with(
    query: Query,
    mut fetch: impl FnMut(&Request) -> Result<(u16, Vec<u8>), String>,
) -> Result<Response, String> {
    let mut plan = Plan::new(query)?;
    while let Some(request) = plan.request() {
        let (status, bytes) = match fetch(request) {
            Ok(response) => response,
            Err(_) if request.optional => (0, Vec::new()),
            Err(error) => return Err(error),
        };
        plan.advance(status, &bytes)?;
    }
    plan.result()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "Requires public RCSB and EMDB services"]
    fn live_public_metadata() {
        let Response::Entries { entries } = execute(Query::Entries {
            pdb_ids: vec!["1crn".into(), "7a4m".into()],
        })
        .unwrap() else {
            panic!()
        };
        assert_eq!(entries.len(), 2);
        assert_eq!(
            entries.iter().find(|e| e.id == "1crn").unwrap().kind,
            EntryKind::Xtal
        );
        assert!(!entries
            .iter()
            .find(|e| e.id == "7a4m")
            .unwrap()
            .emdb_ids
            .is_empty());
        let Response::Search { total, entries } = execute(Query::Search {
            query: "crambin".into(),
            method: SearchMethod::All,
            sort: SearchSort::Resolution,
            start: 0,
            rows: 2,
        })
        .unwrap() else {
            panic!()
        };
        assert!(total >= entries.len());
        assert!(!entries.is_empty());
        let Response::Emdb { info } = execute(Query::Emdb {
            emdb_id: "EMD-11638".into(),
        })
        .unwrap() else {
            panic!()
        };
        assert!(info.dims.is_some());
        assert!(info.recommended_level.is_some());
    }

    #[test]
    fn accessions_distinguish_numeric_pdb_ids_and_prefixed_emdb_ids() {
        assert_eq!(
            Accession::parse(" 1CRN "),
            Some(Accession::Pdb("1crn".into()))
        );
        assert_eq!(
            Accession::parse("1234"),
            Some(Accession::Pdb("1234".into()))
        );
        for text in ["EMD-12345", "emd_12345", "EmD12345"] {
            assert_eq!(
                Accession::parse(text),
                Some(Accession::Emdb("EMD-12345".into()))
            );
        }
        for text in ["12345", "emd-123", "kinase", "abcd", ""] {
            assert!(Accession::parse(text).is_none(), "{text}");
        }
    }

    #[test]
    fn search_preserves_rank_and_classifies_emdb_before_method() {
        let query = Query::Search {
            query: "  kinase  ".into(),
            method: SearchMethod::Em,
            sort: SearchSort::Resolution,
            start: 25,
            rows: 25,
        };
        let mut calls = 0;
        let result = execute_with(query, |request| {
            calls += 1;
            let body = request.body.as_ref().unwrap();
            let response = if calls == 1 {
                assert!(request.url.contains("search.rcsb.org"));
                assert_eq!(body["query"]["nodes"][0]["parameters"]["value"], "kinase");
                assert_eq!(body["query"]["nodes"][1]["parameters"]["value"], "ELECTRON MICROSCOPY");
                assert_eq!(body["request_options"]["paginate"], json!({"start":25,"rows":25}));
                assert_eq!(body["request_options"]["sort"][0]["direction"], "asc");
                json!({"total_count":43,"result_set":[{"identifier":"6AXZ"},{"identifier":"1CRN"},{"identifier":"9ZZZ"}]})
            } else {
                assert_eq!(calls, 2);
                assert!(body["query"].as_str().unwrap().contains("[\"6AXZ\",\"1CRN\",\"9ZZZ\"]"));
                json!({"data":{"entries":[
                    {"rcsb_id":"1CRN","exptl":[{"method":"X-RAY DIFFRACTION"}],"struct":{"title":"Crambin"},"rcsb_entry_info":{"resolution_combined":[1.5]}},
                    {"rcsb_id":"9ZZZ","exptl":null},
                    {"rcsb_id":"6AXZ","exptl":[{"method":"ELECTRON CRYSTALLOGRAPHY"}],"rcsb_entry_container_identifiers":{"emdb_ids":["EMD-7018"]},"pdbx_database_status":{"pdb_format_compatible":"N"}}
                ]}})
            };
            Ok((200, serde_json::to_vec(&response).unwrap()))
        }).unwrap();
        let Response::Search { total, entries } = result else {
            panic!()
        };
        assert_eq!(total, 43);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].id, "6axz");
        assert_eq!(entries[0].kind, EntryKind::Em);
        assert!(!entries[0].pdb_format_available);
        assert_eq!(entries[1].kind, EntryKind::Xtal);
        assert_eq!(entries[1].resolution, Some(1.5));
        assert_eq!(entries[1].title, "Crambin");
    }

    #[test]
    fn missing_results_and_optional_admin_do_not_fail_successful_requests() {
        for query in [
            Query::Entries { pdb_ids: vec![] },
            Query::Search {
                query: " ".into(),
                method: SearchMethod::All,
                sort: SearchSort::Score,
                start: 0,
                rows: 25,
            },
        ] {
            execute_with(query, |_| panic!("empty query must not use the network")).unwrap();
        }
        let result = execute_with(
            Query::Search {
                query: "no hits".into(),
                method: SearchMethod::All,
                sort: SearchSort::Date,
                start: 0,
                rows: 25,
            },
            |r| {
                assert_eq!(
                    r.body.as_ref().unwrap()["request_options"]["sort"][0]["direction"],
                    "desc"
                );
                Ok((204, vec![]))
            },
        )
        .unwrap();
        assert_eq!(
            result,
            Response::Search {
                total: 0,
                entries: vec![]
            }
        );
        let result = execute_with(Query::Emdb { emdb_id:"emd_11638".into() }, |r| {
            if r.optional { return Err("admin offline".into()); }
            assert!(r.url.ends_with("/map/EMD-11638"));
            Ok((200, serde_json::to_vec(&json!({"map":{"size_kbytes":"1200","dimensions":{"col":"20","row":30,"sec":40},"contour_list":{"contour":[{"level":1.0},{"primary":true,"level":"2.75"}]}}})).unwrap()))
        }).unwrap();
        let Response::Emdb { info } = result else {
            panic!()
        };
        assert_eq!(info.recommended_level, Some(2.75));
        assert_eq!(info.dims, Some([20., 30., 40.]));
        assert_eq!(info.size_bytes, Some(1_200_000.));
        assert!(info.title.is_empty());
        assert_eq!(
            parse_emdb(
                "EMD-1",
                &json!({"map":{"contour_list":{"contour":[{"level":null}]}}})
            )
            .recommended_level,
            Some(0.)
        );
        assert!(
            parse_emdb("EMD-1", &json!({"map":{"contour_list":{"contour":[{}]}}}))
                .recommended_level
                .is_none()
        );
    }

    #[test]
    fn failed_or_oversized_metadata_is_not_treated_as_an_empty_result() {
        for status in [404, 503] {
            assert!(execute_with(
                Query::Entries {
                    pdb_ids: vec!["1crn".into()]
                },
                |_| Ok((status, vec![]))
            )
            .is_err());
        }
        for bytes in [b"not json".to_vec(), vec![b' '; MAX_METADATA_BYTES + 1]] {
            let mut plan = Plan::new(Query::Entries {
                pdb_ids: vec!["1crn".into()],
            })
            .unwrap();
            assert!(plan.advance(200, &bytes).is_err());
        }
        let query = Query::Search {
            query: "kinase".into(),
            method: SearchMethod::All,
            sort: SearchSort::Score,
            start: 0,
            rows: 101,
        };
        assert!(Plan::new(query).is_err());
    }
}
