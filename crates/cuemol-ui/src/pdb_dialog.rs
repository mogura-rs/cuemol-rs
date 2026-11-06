use cuemol_command::pdb::{
    self, Accession, DownloadChoice, EmMapRequest, EmdbMapInfo, EntryInfo, EntryKind, Query,
    Response, SearchMethod, SearchSort,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Default)]
enum Lookup {
    #[default]
    Idle,
    Loading,
    Pdb(EntryInfo),
    Emdb,
    Missing,
    Error(String),
}

#[derive(Debug)]
pub struct PdbDialog {
    pub text: String,
    pub history: Vec<String>,
    pub rects: Vec<(String, egui::Rect)>,
    history_update: Option<String>,
    accession: Option<Accession>,
    lookup: Lookup,
    deadline: Option<f64>,
    search_query: Option<String>,
    method: SearchMethod,
    sort: SearchSort,
    entries: Vec<EntryInfo>,
    total: usize,
    next_start: usize,
    searching: bool,
    search_error: String,
    maps: BTreeMap<String, Result<EmdbMapInfo, String>>,
    off: BTreeSet<String>,
    coordinates: bool,
    coordinate_source: String,
    two_fofc: bool,
    fofc: bool,
    map_source: String,
    em_source: String,
    serial: u64,
    pending: BTreeMap<u64, Query>,
    requests: Vec<(u64, Query)>,
    jobs: Vec<(u64, MetadataJob)>,
    ime_active: bool,
    cancelled: bool,
}

impl Default for PdbDialog {
    fn default() -> Self {
        Self {
            text: String::new(),
            history: Vec::new(),
            rects: Vec::new(),
            history_update: None,
            accession: None,
            lookup: Lookup::Idle,
            deadline: None,
            search_query: None,
            method: SearchMethod::All,
            sort: SearchSort::Score,
            entries: Vec::new(),
            total: 0,
            next_start: 0,
            searching: false,
            search_error: String::new(),
            maps: BTreeMap::new(),
            off: BTreeSet::new(),
            coordinates: true,
            coordinate_source: "RCSB_CIF".into(),
            two_fofc: false,
            fofc: false,
            map_source: "RCSB_CIF".into(),
            em_source: "WWPDB".into(),
            serial: 0,
            pending: BTreeMap::new(),
            requests: Vec::new(),
            jobs: Vec::new(),
            ime_active: false,
            cancelled: false,
        }
    }
}

impl PdbDialog {
    pub fn close(&mut self) {
        self.pending.clear();
        self.requests.clear();
        self.jobs.clear();
        self.deadline = None;
        self.searching = false;
    }

    pub fn reset(&mut self) {
        let history = std::mem::take(&mut self.history);
        let serial = self.serial;
        *self = Self {
            history,
            serial,
            ..Self::default()
        };
    }

    fn enqueue(&mut self, query: Query) {
        self.serial = self.serial.wrapping_add(1);
        self.pending.insert(self.serial, query.clone());
        self.requests.push((self.serial, query));
    }

    fn clear_lookup(&mut self) {
        self.pending
            .retain(|_, query| matches!(query, Query::Search { .. }));
        self.requests
            .retain(|(id, _)| self.pending.contains_key(id));
        self.jobs.retain(|(id, _)| self.pending.contains_key(id));
        self.deadline = None;
        self.accession = None;
        self.lookup = Lookup::Idle;
        self.maps.clear();
        self.off.clear();
    }

    fn edited(&mut self, now: f64) {
        self.close();
        self.clear_lookup();
        self.search_query = None;
        self.entries.clear();
        self.search_error.clear();
        self.accession = Accession::parse(&self.text);
        if self.accession.is_some() {
            self.lookup = Lookup::Loading;
            self.deadline = Some(now + 0.4);
        }
    }

    fn remember(&mut self, value: &str) {
        if let Some(value) = pdb::normalize_history(value) {
            self.history.retain(|entry| entry != &value);
            self.history.insert(0, value.clone());
            self.history.truncate(20);
            self.history_update = Some(value);
        }
    }

    pub fn take_cancelled(&mut self) -> bool {
        std::mem::take(&mut self.cancelled)
    }

    pub fn take_history_update(&mut self) -> Option<String> {
        self.history_update.take()
    }

    fn search(&mut self, more: bool) {
        let query = if more {
            self.search_query.clone().unwrap_or_default()
        } else {
            self.text.trim().to_owned()
        };
        if query.is_empty() || (more && self.searching) {
            return;
        }
        if !more {
            self.close();
            self.clear_lookup();
            self.entries.clear();
            self.next_start = 0;
            self.total = 0;
            self.search_query = Some(query.clone());
            self.remember(&query);
        }
        self.search_error.clear();
        self.searching = true;
        self.enqueue(Query::Search {
            query,
            method: self.method,
            sort: self.sort,
            start: self.next_start,
            rows: 25,
        });
    }

    fn select(&mut self, entry: EntryInfo) {
        self.clear_lookup();
        self.accession = Some(Accession::Pdb(entry.id.clone()));
        for id in &entry.emdb_ids {
            self.enqueue(Query::Emdb {
                emdb_id: id.clone(),
            });
        }
        self.lookup = Lookup::Pdb(entry);
    }

    fn accept(&mut self, ticket: u64, result: Result<Response, String>) {
        let Some(query) = self.pending.remove(&ticket) else {
            return;
        };
        match query {
            Query::Search { start, rows, .. } => {
                self.searching = false;
                match result {
                    Ok(Response::Search { total, entries }) => {
                        self.total = total;
                        self.next_start = start.saturating_add(rows);
                        for entry in entries {
                            if !self.entries.iter().any(|e| e.id == entry.id) {
                                self.entries.push(entry);
                            }
                        }
                    }
                    Err(error) => self.search_error = error,
                    _ => self.search_error = "Unexpected search response".into(),
                }
            }
            Query::Entries { pdb_ids } => match result {
                Ok(Response::Entries { entries }) => {
                    if let Some(entry) = entries.into_iter().find(|e| pdb_ids.contains(&e.id)) {
                        self.select(entry);
                    } else {
                        self.lookup = Lookup::Missing;
                    }
                }
                Err(error) => self.lookup = Lookup::Error(error),
                _ => self.lookup = Lookup::Error("Unexpected entry response".into()),
            },
            Query::Emdb { emdb_id } => {
                let direct = self.accession == Some(Accession::Emdb(emdb_id.clone()));
                match result {
                    Ok(Response::Emdb { info }) => {
                        if direct {
                            self.lookup = Lookup::Emdb;
                        }
                        self.maps.insert(emdb_id, Ok(info));
                    }
                    Err(error) => {
                        if direct {
                            self.lookup = if error == "Entry not found" {
                                Lookup::Missing
                            } else {
                                Lookup::Error(error.clone())
                            };
                        }
                        self.maps.insert(emdb_id, Err(error));
                    }
                    _ => {
                        self.maps
                            .insert(emdb_id, Err("Unexpected map response".into()));
                    }
                }
            }
        }
    }

    fn tick(&mut self, ctx: &egui::Context) {
        let completed = self
            .jobs
            .iter()
            .filter_map(|(id, job)| job.poll().map(|r| (*id, r)))
            .collect::<Vec<_>>();
        for (id, result) in completed {
            self.accept(id, result);
        }
        self.jobs.retain(|(id, _)| self.pending.contains_key(id));
        let now = ctx.input(|i| i.time);
        if self.deadline.is_some_and(|deadline| deadline <= now) {
            self.deadline = None;
            match self.accession.clone() {
                Some(Accession::Pdb(id)) => self.enqueue(Query::Entries { pdb_ids: vec![id] }),
                Some(Accession::Emdb(emdb_id)) => self.enqueue(Query::Emdb { emdb_id }),
                None => {}
            }
        } else if let Some(deadline) = self.deadline {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64((deadline - now).max(0.)));
        }
        for (id, query) in std::mem::take(&mut self.requests) {
            match MetadataJob::start(query, ctx.clone()) {
                Ok(job) => self.jobs.push((id, job)),
                Err(error) => self.accept(id, Err(error)),
            }
        }
    }

    fn xtal(&self) -> bool {
        matches!(&self.lookup, Lookup::Pdb(entry) if entry.kind == EntryKind::Xtal)
            || (matches!(self.lookup, Lookup::Error(_))
                && matches!(self.accession, Some(Accession::Pdb(_))))
    }

    fn map_ids(&self) -> Vec<String> {
        match &self.lookup {
            Lookup::Pdb(entry) if entry.kind == EntryKind::Em => entry.emdb_ids.clone(),
            Lookup::Emdb | Lookup::Error(_) => match &self.accession {
                Some(Accession::Emdb(id)) => vec![id.clone()],
                _ => vec![],
            },
            _ => vec![],
        }
    }

    pub fn choice(&self) -> Option<DownloadChoice> {
        let accession = self.accession.clone()?;
        if matches!(self.lookup, Lookup::Missing) {
            return None;
        }
        let coordinates = matches!(accession, Accession::Pdb(_)) && self.coordinates;
        let coordinate_source = if matches!(&self.lookup, Lookup::Pdb(entry) if !entry.pdb_format_available)
        {
            "RCSB_CIF".into()
        } else {
            self.coordinate_source.clone()
        };
        let two_fofc = self.xtal() && self.two_fofc;
        let fofc = self.xtal() && self.fofc;
        let em_maps = self
            .map_ids()
            .into_iter()
            .filter(|id| !self.off.contains(id))
            .map(|id| {
                let level = self
                    .maps
                    .get(&id)
                    .and_then(|v| v.as_ref().ok())
                    .and_then(|info| info.recommended_level);
                EmMapRequest {
                    emdb_id: id,
                    source: self.em_source.clone(),
                    level,
                }
            })
            .collect::<Vec<_>>();
        if !coordinates && !two_fofc && !fofc && em_maps.is_empty() {
            return None;
        }
        Some(DownloadChoice {
            accession,
            coordinates,
            coordinate_source,
            two_fofc,
            fofc,
            map_source: self.map_source.clone(),
            em_maps,
        })
    }

    fn record(&mut self, name: impl Into<String>, response: &egui::Response) {
        self.rects.push((name.into(), response.rect));
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> Option<DownloadChoice> {
        ui.set_width((ui.ctx().content_rect().width() - 32.).clamp(120., 560.));
        self.tick(ui.ctx());
        self.rects.clear();
        let now = ui.input(|i| i.time);
        for event in ui.input(|i| i.events.clone()) {
            if let egui::Event::Ime(event) = event {
                self.ime_active =
                    !matches!(event, egui::ImeEvent::Disabled | egui::ImeEvent::Commit(_));
            }
        }
        let mut enter = false;
        let mut submit = false;
        let height = (ui.ctx().content_rect().height() - 180.).clamp(120., 560.);
        egui::ScrollArea::vertical()
            .id_salt("pdb-body")
            .max_height(height)
            .show(ui, |ui| {
                ui.label("PDB or EMDB ID, or keywords");
                let mut recalled = None;
                let mut direction = 0;
                ui.horizontal(|ui| {
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.text)
                            .hint_text("1mbn, EMD-11638, or keywords")
                            .desired_width((ui.available_width() - 80.).max(120.)),
                    );
                    self.record("Get PDB identifier", &response);
                    if response.changed() {
                        self.edited(now);
                    }
                    if !self.ime_active && (response.has_focus() || response.lost_focus()) {
                        enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                        direction = ui.input(|i| {
                            if i.key_pressed(egui::Key::ArrowDown) {
                                1
                            } else if i.key_pressed(egui::Key::ArrowUp) {
                                -1
                            } else {
                                0
                            }
                        });
                    }
                    ui.menu_button("History", |ui| {
                        for value in &self.history {
                            if ui.button(value).clicked() {
                                recalled = Some(value.clone());
                                ui.close();
                            }
                        }
                    });
                });
                if let Some(value) = recalled {
                    self.text = value;
                    self.edited(now);
                    if self.accession.is_none() {
                        self.search(false);
                    }
                }
                let keywords =
                    Accession::parse(&self.text).is_none() && !self.text.trim().is_empty();
                if keywords {
                    let search = ui.add_enabled(!self.searching, egui::Button::new("Search PDB"));
                    self.record("Search PDB", &search);
                    if search.clicked()
                        || (enter && self.search_query.as_deref() != Some(self.text.trim()))
                    {
                        self.search(false);
                        enter = false;
                    }
                }
                if self.search_query.is_some() {
                    let old = (self.method, self.sort);
                    ui.horizontal(|ui| {
                        egui::ComboBox::from_id_salt("pdb-method")
                            .selected_text(match self.method {
                                SearchMethod::All => "All methods",
                                SearchMethod::Xray => "X-ray",
                                SearchMethod::Em => "Electron microscopy",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.method,
                                    SearchMethod::All,
                                    "All methods",
                                );
                                ui.selectable_value(&mut self.method, SearchMethod::Xray, "X-ray");
                                ui.selectable_value(
                                    &mut self.method,
                                    SearchMethod::Em,
                                    "Electron microscopy",
                                );
                            });
                        egui::ComboBox::from_id_salt("pdb-sort")
                            .selected_text(match self.sort {
                                SearchSort::Score => "Relevance",
                                SearchSort::Resolution => "Best resolution",
                                SearchSort::Date => "Newest",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.sort, SearchSort::Score, "Relevance");
                                ui.selectable_value(
                                    &mut self.sort,
                                    SearchSort::Resolution,
                                    "Best resolution",
                                );
                                ui.selectable_value(&mut self.sort, SearchSort::Date, "Newest");
                            });
                    });
                    if old != (self.method, self.sort) {
                        self.search(false);
                    }
                    ui.label(if self.searching {
                        "Searching…".into()
                    } else {
                        format!("{} / {} entries", self.entries.len(), self.total)
                    });
                    let mut pick = None;
                    if direction != 0 && !self.entries.is_empty() {
                        let current = self
                            .entries
                            .iter()
                            .position(|e| self.accession.as_ref().is_some_and(|a| a.id() == e.id))
                            .map_or(-1, |i| i as isize);
                        let index = (current + direction).clamp(0, self.entries.len() as isize - 1)
                            as usize;
                        pick = Some(self.entries[index].clone());
                    }
                    let mut rects = Vec::new();
                    egui::ScrollArea::vertical()
                        .id_salt("pdb-results")
                        .max_height(180.)
                        .show(ui, |ui| {
                            for entry in &self.entries {
                                let selected =
                                    self.accession.as_ref().is_some_and(|a| a.id() == entry.id);
                                let meta = entry
                                    .resolution
                                    .map_or(String::new(), |n| format!(" ({n:.1} Å)"));
                                let label = format!(
                                    "{}  {}{meta}",
                                    entry.id.to_ascii_uppercase(),
                                    entry.title
                                );
                                let response = ui
                                    .add_sized(
                                        [ui.available_width(), 24.],
                                        egui::Button::selectable(selected, label).truncate(),
                                    )
                                    .on_hover_text(&entry.title);
                                rects.push((format!("PDB result {}", entry.id), response.rect));
                                if response.clicked() {
                                    pick = Some(entry.clone());
                                }
                            }
                        });
                    self.rects.extend(rects);
                    if let Some(entry) = pick {
                        self.select(entry);
                    }
                    if self.next_start < self.total {
                        let more = ui.add_enabled(!self.searching, egui::Button::new("More"));
                        self.record("More PDB results", &more);
                        if more.clicked() {
                            self.search(true);
                        }
                    }
                    if !self.search_error.is_empty() {
                        ui.label(format!("Search failed: {}", self.search_error));
                    }
                }
                match &self.lookup {
                    Lookup::Idle => {}
                    Lookup::Loading => {
                        ui.label("Reading entry information…");
                    }
                    Lookup::Pdb(entry) => {
                        ui.label(&entry.title);
                        ui.label(format!(
                            "{}{}",
                            entry.methods.join(", "),
                            entry
                                .resolution
                                .map_or(String::new(), |n| format!(" | {n:.2} Å"))
                        ));
                    }
                    Lookup::Emdb => {
                        if let Some(info) = self
                            .accession
                            .as_ref()
                            .and_then(|a| self.maps.get(a.id()))
                            .and_then(|v| v.as_ref().ok())
                        {
                            if !info.title.is_empty() {
                                ui.label(&info.title);
                            }
                        }
                        ui.label("Cryo-EM map, no coordinates");
                    }
                    Lookup::Missing => {
                        ui.label("No entry was found.");
                        if ui.button("Search as keywords").clicked() {
                            self.search(false);
                        }
                    }
                    Lookup::Error(error) => {
                        ui.label(format!(
                    "Entry information unavailable: {error}. The files can still be downloaded."
                ));
                    }
                }
                if matches!(self.accession, Some(Accession::Pdb(_)))
                    && !matches!(self.lookup, Lookup::Missing)
                {
                    ui.group(|ui| {
                        let response = ui.checkbox(&mut self.coordinates, "Fetch coordinates");
                        self.record("Fetch coordinates", &response);
                        ui.add_enabled_ui(self.coordinates, |ui| {
                    let legacy =
                        !matches!(&self.lookup,Lookup::Pdb(entry) if !entry.pdb_format_available);
                    if !legacy {
                        self.coordinate_source = "RCSB_CIF".into();
                    }
                    egui::ComboBox::from_label("Coordinate format")
                        .selected_text(if self.coordinate_source == "RCSB_PDB" {
                            "PDB"
                        } else {
                            "mmCIF"
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.coordinate_source,
                                "RCSB_CIF".into(),
                                "mmCIF",
                            );
                            ui.add_enabled_ui(legacy, |ui| {
                                ui.selectable_value(
                                    &mut self.coordinate_source,
                                    "RCSB_PDB".into(),
                                    "PDB",
                                );
                            });
                        });
                    if !legacy {
                        ui.label("This entry requires mmCIF.");
                    }
                });
                    });
                }
                if self.xtal() {
                    ui.group(|ui| {
                        ui.strong("Crystallographic maps");
                        let response = ui.checkbox(&mut self.two_fofc, "2Fo-Fc");
                        self.record("Fetch 2Fo-Fc", &response);
                        let response = ui.checkbox(&mut self.fofc, "Fo-Fc");
                        self.record("Fetch Fo-Fc", &response);
                        egui::ComboBox::from_label("Map source")
                            .selected_text(&self.map_source)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.map_source,
                                    "RCSB_CIF".into(),
                                    "RCSB",
                                );
                                ui.selectable_value(&mut self.map_source, "EBI_MTZ".into(), "EBI");
                            });
                    });
                }
                let ids = self.map_ids();
                if !ids.is_empty() {
                    ui.group(|ui| {
                        ui.strong("Cryo-EM maps");
                        for id in ids {
                            let mut on = !self.off.contains(&id);
                            let response = ui.checkbox(&mut on, &id);
                            self.record(format!("Fetch {id}"), &response);
                            if on {
                                self.off.remove(&id);
                            } else {
                                self.off.insert(id.clone());
                            }
                            match self.maps.get(&id) {
                                Some(Ok(info)) => {
                                    let mut parts = Vec::new();
                                    if let Some(size) = info.size_bytes {
                                        parts.push(format!("{:.1} MB", size / 1e6));
                                    }
                                    if let Some(dims) = info.dims {
                                        parts.push(format!(
                                            "{} × {} × {}",
                                            dims[0], dims[1], dims[2]
                                        ));
                                    }
                                    parts.push(
                                        info.recommended_level
                                            .map_or("Default contour level".into(), |n| {
                                                format!("Recommended level {n}")
                                            }),
                                    );
                                    ui.small(parts.join(" | "));
                                }
                                Some(Err(_)) => {
                                    ui.small(
                                "Map information unavailable; the default contour level is used.",
                            );
                                }
                                None => {
                                    ui.small("Reading map information…");
                                }
                            }
                        }
                        egui::ComboBox::from_label("EMDB source")
                            .selected_text(&self.em_source)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.em_source, "WWPDB".into(), "wwPDB");
                                ui.selectable_value(&mut self.em_source, "EBI".into(), "EBI");
                            });
                    });
                }
            });
        let choice = self.choice();
        ui.separator();
        ui.horizontal(|ui| {
            let response = ui.add_enabled(choice.is_some(), egui::Button::new("Download and Open"));
            self.record("Fetch PDB", &response);
            submit |= response.clicked() || enter;
            let cancel = ui.button("Cancel");
            self.record("Cancel Get PDB", &cancel);
            self.cancelled = cancel.clicked()
                || (!self.ime_active && ui.input(|i| i.key_pressed(egui::Key::Escape)));
        });
        self.tick(ui.ctx());
        if submit {
            if let Some(choice) = choice {
                self.remember(choice.accession.id());
                return Some(choice);
            }
        }
        None
    }
}

#[derive(Debug)]
struct MetadataJob {
    receiver: std::sync::mpsc::Receiver<Result<Response, String>>,
    #[cfg(target_arch = "wasm32")]
    abort: web_sys::AbortController,
}

impl MetadataJob {
    fn poll(&self) -> Option<Result<Response, String>> {
        match self.receiver.try_recv() {
            Ok(result) => Some(result),
            Err(std::sync::mpsc::TryRecvError::Empty) => None,
            Err(_) => Some(Err("Metadata worker stopped".into())),
        }
    }

    fn start(query: Query, ctx: egui::Context) -> Result<Self, String> {
        let (sender, receiver) = std::sync::mpsc::channel();
        #[cfg(not(target_arch = "wasm32"))]
        {
            std::thread::Builder::new()
                .name("cuemol-entry-info".into())
                .spawn(move || {
                    let _ = sender.send(pdb::execute(query));
                    ctx.request_repaint();
                })
                .map_err(|e| e.to_string())?;
            Ok(Self { receiver })
        }
        #[cfg(target_arch = "wasm32")]
        {
            let abort = web_sys::AbortController::new().map_err(|e| format!("{e:?}"))?;
            let signal = abort.signal();
            wasm_bindgen_futures::spawn_local(async move {
                let result = fetch_metadata(query, signal).await;
                let _ = sender.send(result);
                ctx.request_repaint();
            });
            Ok(Self { receiver, abort })
        }
    }
}

#[cfg(target_arch = "wasm32")]
impl Drop for MetadataJob {
    fn drop(&mut self) {
        self.abort.abort();
    }
}

#[cfg(target_arch = "wasm32")]
async fn fetch_metadata(query: Query, signal: web_sys::AbortSignal) -> Result<Response, String> {
    use wasm_bindgen::{JsCast, JsValue};
    use wasm_bindgen_futures::JsFuture;
    let mut plan = pdb::Plan::new(query)?;
    while let Some(request) = plan.request() {
        if signal.aborted() {
            return Err("Metadata lookup cancelled".into());
        }
        let fetch = async {
            let options = web_sys::RequestInit::new();
            options.set_signal(Some(&signal));
            if let Some(body) = &request.body {
                options.set_method("POST");
                options.set_body(&JsValue::from_str(&body.to_string()));
                let headers = web_sys::Headers::new()?;
                headers.set("Content-Type", "application/json")?;
                options.set_headers(&headers);
            }
            let window = web_sys::window()
                .ok_or_else(|| JsValue::from_str("Browser window is unavailable"))?;
            let response = JsFuture::from(window.fetch_with_str_and_init(&request.url, &options))
                .await?
                .dyn_into::<web_sys::Response>()?;
            let status = response.status();
            if status == 204 || !response.ok() {
                return Ok((status, Vec::new()));
            }
            if response
                .headers()
                .get("Content-Length")?
                .and_then(|s| s.parse::<usize>().ok())
                .is_some_and(|n| n > pdb::MAX_METADATA_BYTES)
            {
                return Err(JsValue::from_str("Metadata exceeds 4 MiB"));
            }
            let bytes = JsFuture::from(response.array_buffer()?).await?;
            Ok::<_, JsValue>((status, js_sys::Uint8Array::new(&bytes).to_vec()))
        }
        .await;
        let (status, bytes) = match fetch {
            Ok(response) => response,
            Err(_) if request.optional => (0, Vec::new()),
            Err(error) => return Err(error.as_string().unwrap_or_else(|| format!("{error:?}"))),
        };
        plan.advance(status, &bytes)?;
    }
    plan.result()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, kind: EntryKind, maps: &[&str]) -> EntryInfo {
        EntryInfo {
            id: id.into(),
            title: id.into(),
            methods: vec![],
            resolution: None,
            emdb_ids: maps.iter().map(|s| s.to_string()).collect(),
            pdb_format_available: true,
            kind,
        }
    }

    #[test]
    fn download_and_cancel_stay_inside_the_window_after_results_expand() {
        for size in [egui::vec2(1280., 900.), egui::vec2(760., 650.)] {
            let ctx = egui::Context::default();
            crate::shell::theme(&ctx, false);
            let mut dialog = PdbDialog {
                text: "ribosome".into(),
                search_query: Some("ribosome".into()),
                ..Default::default()
            };
            let selected = entry("7a4m", EntryKind::Em, &["EMD-11638"]);
            dialog.entries = vec![selected.clone(); 25];
            dialog.total = 25;
            dialog.next_start = 25;
            dialog.accession = Some(Accession::Pdb("7a4m".into()));
            dialog.lookup = Lookup::Pdb(selected);
            dialog.maps.insert(
                "EMD-11638".into(),
                Ok(EmdbMapInfo {
                    emdb_id: "EMD-11638".into(),
                    title: String::new(),
                    recommended_level: Some(20.),
                    size_bytes: Some(2000.),
                    dims: Some([4., 4., 4.]),
                }),
            );
            for frame in 0..3 {
                let _ = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                        time: Some(frame as f64),
                        ..Default::default()
                    },
                    |ctx| {
                        let window = egui::Window::new("Get PDB")
                            .default_width(560.)
                            .auto_sized()
                            .show(ctx, |ui| {
                                dialog.show(ui);
                            })
                            .unwrap();
                        if frame > 0 {
                            assert!(
                                window.response.rect.width() <= 600.,
                                "{:?}",
                                window.response.rect
                            );
                            for name in ["Fetch PDB", "Cancel Get PDB"] {
                                let rect =
                                    dialog.rects.iter().find(|(key, _)| key == name).unwrap().1;
                                assert!(
                                    window.response.rect.contains_rect(rect),
                                    "{name}: {rect:?} outside {:?}",
                                    window.response.rect
                                );
                                assert!(
                                    ctx.content_rect().contains_rect(rect),
                                    "{name}: outside screen"
                                );
                            }
                        }
                    },
                );
            }
        }
    }

    #[test]
    fn stale_search_and_lookup_responses_cannot_replace_new_input() {
        let mut dialog = PdbDialog {
            text: "kinase".into(),
            ..Default::default()
        };
        dialog.search(false);
        let first = dialog.requests[0].0;
        dialog.text = "1crn".into();
        dialog.edited(1.);
        dialog.accept(
            first,
            Ok(Response::Search {
                total: 1,
                entries: vec![entry("7a4m", EntryKind::Em, &[])],
            }),
        );
        assert!(dialog.entries.is_empty());
        assert_eq!(dialog.accession, Some(Accession::Pdb("1crn".into())));
        dialog.enqueue(Query::Entries {
            pdb_ids: vec!["1crn".into()],
        });
        let first = dialog.requests.last().unwrap().0;
        dialog.text = "EMD-11638".into();
        dialog.edited(2.);
        dialog.accept(
            first,
            Ok(Response::Entries {
                entries: vec![entry("1crn", EntryKind::Xtal, &[])],
            }),
        );
        assert!(matches!(dialog.lookup, Lookup::Loading));
        assert_eq!(dialog.accession, Some(Accession::Emdb("EMD-11638".into())));
        dialog.close();
        assert!(dialog.pending.is_empty());
    }

    #[test]
    fn choice_preserves_queries_and_requires_the_correct_map_kind() {
        let mut dialog = PdbDialog {
            text: "ribosome".into(),
            ..Default::default()
        };
        dialog.search(false);
        let ticket = dialog.requests[0].0;
        let mut result = entry("7a4m", EntryKind::Em, &["EMD-11638", "EMD-11639"]);
        result.pdb_format_available = false;
        dialog.accept(
            ticket,
            Ok(Response::Search {
                total: 2,
                entries: vec![result.clone()],
            }),
        );
        assert!(dialog.choice().is_none());
        dialog.select(result);
        dialog.coordinate_source = "RCSB_PDB".into();
        dialog.two_fofc = true;
        let choice = dialog.choice().unwrap();
        assert_eq!(dialog.text, "ribosome");
        assert_eq!(choice.coordinate_source, "RCSB_CIF");
        assert!(!choice.two_fofc);
        assert_eq!(choice.em_maps.len(), 2);
        let (ticket, _) = dialog
            .requests
            .iter()
            .find(|(_, q)| matches!(q,Query::Emdb {emdb_id} if emdb_id=="EMD-11638"))
            .unwrap()
            .clone();
        dialog.accept(
            ticket,
            Ok(Response::Emdb {
                info: EmdbMapInfo {
                    emdb_id: "EMD-11638".into(),
                    title: String::new(),
                    recommended_level: Some(2.75),
                    size_bytes: None,
                    dims: None,
                },
            }),
        );
        dialog.off.insert("EMD-11639".into());
        let choice = dialog.choice().unwrap();
        assert_eq!(choice.em_maps.len(), 1);
        assert_eq!(choice.em_maps[0].level, Some(2.75));
        dialog.select(entry("1crn", EntryKind::Xtal, &[]));
        assert!(dialog.choice().unwrap().two_fofc);
        assert!(dialog.choice().unwrap().em_maps.is_empty());
        dialog.lookup = Lookup::Error("offline".into());
        assert!(dialog.choice().unwrap().two_fofc);
        dialog.lookup = Lookup::Missing;
        assert!(dialog.choice().is_none());
    }

    #[test]
    fn paging_uses_server_offsets_when_entries_are_missing_and_history_is_bounded() {
        let mut dialog = PdbDialog {
            text: "  membrane   protein ".into(),
            ..Default::default()
        };
        dialog.search(false);
        let ticket = dialog.requests[0].0;
        dialog.accept(
            ticket,
            Ok(Response::Search {
                total: 60,
                entries: vec![entry("1crn", EntryKind::Xtal, &[])],
            }),
        );
        dialog.search(true);
        assert!(matches!(
            dialog.requests.last().unwrap().1,
            Query::Search { start: 25, .. }
        ));
        assert_eq!(dialog.history[0], "membrane protein");
        for i in 0..30 {
            dialog.remember(&format!("query {i}"));
        }
        assert_eq!(dialog.history.len(), 20);
        dialog.remember("1CRN");
        dialog.remember("1crn");
        assert_eq!(
            dialog
                .history
                .iter()
                .filter(|s| s.as_str() == "1crn")
                .count(),
            1
        );
    }
}
