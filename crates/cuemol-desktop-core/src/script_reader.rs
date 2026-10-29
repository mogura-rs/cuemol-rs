use super::*;
#[cfg(not(target_arch = "wasm32"))]
use cuemol_qsys::IOThread;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub(super) struct ReaderConfig {
    pub class: String,
    pub io: cuemol_qsys::InOutHandler,
    pub options: LDom2Node,
    pub expected_atoms: Option<usize>,
}

#[derive(Debug)]
pub(super) enum ReaderObject {
    Molecule(MolCoord),
    Density(DensityMap),
    Surface(cuemol_surface::SurfaceMesh),
    Lightweight(cuemol_lwview::LWObject),
    Trajectory(GuiTrajectoryFrames),
}

#[derive(Debug)]
pub(super) struct ReaderResult {
    pub object: ReaderObject,
    pub properties: BTreeMap<String, String>,
}

impl ReaderConfig {
    pub fn text(&self, name: &str) -> String {
        self.options.str_attr(name).to_string()
    }

    fn flag(&self, name: &str, default: bool) -> bool {
        self.options
            .find_child(name)
            .map_or(default, |node| qsc_bool(node.value()))
    }

    fn number(&self, name: &str, default: f64) -> Result<f64> {
        let value = self.text(name);
        if value.is_empty() {
            return Ok(default);
        }
        value
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite())
            .ok_or_else(|| {
                CueMolError::invalid_argument(format!("reader property {name} must be finite"))
            })
    }

    pub fn stride(&self) -> Result<usize> {
        let value = self.number("nevery", 1.)?;
        if value < 1. || value.fract() != 0. || value > usize::MAX as f64 {
            return Err(CueMolError::invalid_argument(
                "trajectory stride must be positive",
            ));
        }
        Ok(value as usize)
    }

    pub fn read(&self, data: Vec<u8>) -> Result<ReaderResult> {
        let data = self.io.decode_bytes(data)?;
        let text =
            || std::str::from_utf8(&data).map_err(|error| CueMolError::parse(error.to_string()));
        let mut properties = BTreeMap::new();
        let object = match self.class.as_str() {
            "PDBFileReader" | "QdfPdbReader" | "QdfMolReader" | "MmcifMolReader"
            | "SDFMolReader" | "MOL2MolReader" | "GROFileReader" | "PsfReader"
            | "AmberPrmtopReader" | "NAMDCoorReader" => {
                ReaderObject::Molecule(if self.class == "QdfPdbReader" {
                    cuemol_molstr::read_qdfpdb_str(text()?)?
                } else {
                    cuemol_molstr::qsys::read_molecule_bytes_with_options(
                        self.io.name(),
                        &data,
                        Some(&self.options),
                        None,
                        &trajectory_io::read_extra_qsc_molecule_with_options,
                    )?
                })
            }
            "BrixMapReader" => {
                ReaderObject::Density(BrixMapReader::new().read_density_map_bytes(&data)?)
            }
            "CCP4MapReader" => {
                let mut reader = Ccp4MapReader::new();
                reader.set_normalize(self.flag("normalize", false));
                reader.set_truncate_min(self.flag("truncate_min", false));
                reader.set_min(self.number("min", 0.)?);
                reader.set_truncate_max(self.flag("truncate_max", false));
                reader.set_max(self.number("max", 5.)?);
                reader.set_subsample(self.number("subsample", 1.)?.max(1.) as usize);
                reader.set_max_voxels(self.number("max_voxels", 0.)?);
                reader.set_use_header_stats(self.flag("use_header_stats", true));
                ReaderObject::Density(reader.read_density_map_bytes(&data)?)
            }
            "MmcifMapReader" => {
                let mut reader = cuemol_xtal::MmcifMapReader::new();
                reader.set_resolution(self.number("resolution", -1.)?);
                reader.set_grid_size(self.number("gridsize", 0.33)?);
                ReaderObject::Density(reader.build_density_map_from_str(text()?)?)
            }
            "MTZ2MapReader" => {
                let mut reader = Mtz2MapReader::new();
                reader.set_resolution(self.number("resolution", -1.)?);
                reader.set_grid_size(self.number("gridsize", 0.33)?);
                reader.set_column_f(self.text("clmn_F"));
                reader.set_column_phi(self.text("clmn_PHI"));
                reader.set_column_weight(self.text("clmn_WT"));
                reader.read_bytes(&data)?;
                properties.insert("min_res".into(), reader.min_res().to_string());
                properties.insert("max_res".into(), reader.max_res().to_string());
                properties.insert("_column_info".into(), reader.column_info_json());
                ReaderObject::Density(reader.build_density_map()?)
            }
            "OpenDXPotReader" => {
                let mut reader = cuemol_surface::OpenDxPotReader::new();
                reader.set_smooth(self.number("smooth", -1.)?);
                ReaderObject::Density(density_map_from_electrostatic_map(
                    &reader.read_str(text()?)?,
                ))
            }
            "XplorMapReader" => {
                ReaderObject::Density(XplorMapReader::new().read_density_map_str(text()?)?)
            }
            "QdfDenMapReader" => ReaderObject::Density(QdfDenMapReader::new().read_bytes(&data)?),
            "QdfPotReader" => ReaderObject::Density(density_map_from_electrostatic_map(
                &cuemol_surface::QdfPotReader::new().read_bytes(&data)?,
            )),
            "PLYFileReader" => {
                ReaderObject::Surface(cuemol_surface::PlyFileReader::new().read_bytes(&data)?)
            }
            "MSMSFileReader" => {
                let path = self.text("vertex_file");
                if path.trim().is_empty() {
                    return Err(CueMolError::invalid_argument(
                        "MSMS vertex_file is not configured",
                    ));
                }
                let vertices = cuemol_molstr::read_text_maybe_compressed(Path::new(&path))?;
                ReaderObject::Surface(cuemol_surface::read_msms_surface_str(&vertices, text()?)?)
            }
            "QdfSurfReader" => ReaderObject::Surface(
                cuemol_surface::QdfSurfReader::new()
                    .read_bytes(&data)?
                    .mesh()
                    .clone(),
            ),
            "QdfLWObjReader" => {
                ReaderObject::Lightweight(cuemol_lwview::read_qdf_lwobj_bytes(&data)?)
            }
            "DCDTrajReader" | "XtcTrajReader" | "TrrTrajReader" | "AmberNetCDFReader" => {
                let indexed = cuemol_mdtools::IndexedTrajectory::from_bytes_with_name(
                    &data,
                    self.io.path(),
                    self.stride()?,
                    self.expected_atoms,
                )?;
                let correct = matches!(
                    (self.class.as_str(), indexed.format_name()),
                    ("DCDTrajReader", "DCD")
                        | ("XtcTrajReader", "XTC")
                        | ("TrrTrajReader", "TRR")
                        | ("AmberNetCDFReader", "NETCDF")
                );
                if !correct {
                    return Err(CueMolError::invalid_argument(
                        "trajectory reader does not match the source format",
                    ));
                }
                let frames = if self.flag("lazy_load", self.class != "AmberNetCDFReader") {
                    GuiTrajectoryFrames::File(Arc::new(indexed))
                } else {
                    let mut block = cuemol_mdtools::TrajBlock::new(indexed.atom_count());
                    for frame in 0..indexed.frame_count() {
                        let frame = indexed.read_frame(frame)?;
                        block.append_frame(frame.coordinates, frame.cell)?;
                    }
                    GuiTrajectoryFrames::Owned(Arc::new(block))
                };
                ReaderObject::Trajectory(frames)
            }
            class => {
                return Err(CueMolError::invalid_argument(format!(
                    "unsupported asynchronous reader {class}"
                )))
            }
        };
        Ok(ReaderResult { object, properties })
    }
}

#[derive(Debug)]
struct Task {
    config: ReaderConfig,
    #[cfg(not(target_arch = "wasm32"))]
    worker: IOThread<ReaderResult>,
    #[cfg(target_arch = "wasm32")]
    data: Vec<u8>,
}

#[derive(Debug, Default)]
struct Tasks {
    next: i32,
    pending: BTreeMap<i32, Task>,
}

#[derive(Debug, Clone, Default)]
pub(super) struct AsyncReaders(Arc<Mutex<Tasks>>);

impl AsyncReaders {
    pub fn start(&self, config: ReaderConfig) -> Result<i32> {
        let mut tasks = self.0.lock().unwrap();
        let id = tasks
            .next
            .checked_add(1)
            .ok_or_else(|| CueMolError::invalid_argument("too many asynchronous loads"))?;
        tasks.next = id;
        #[cfg(not(target_arch = "wasm32"))]
        let worker = {
            let reader = config.clone();
            IOThread::spawn(move |input| {
                let mut data = Vec::new();
                input.read_to_end(&mut data)?;
                reader.read(data)
            })
        };
        tasks.pending.insert(
            id,
            Task {
                config,
                #[cfg(not(target_arch = "wasm32"))]
                worker,
                #[cfg(target_arch = "wasm32")]
                data: Vec::new(),
            },
        );
        Ok(id)
    }

    pub fn supply(&self, id: i32, data: &[u8]) -> Result<()> {
        let mut tasks = self.0.lock().unwrap();
        let task = tasks
            .pending
            .get_mut(&id)
            .ok_or_else(|| CueMolError::not_found("asynchronous load"))?;
        #[cfg(not(target_arch = "wasm32"))]
        if !task.worker.supply_data(data) {
            return Err(CueMolError::invalid_argument(
                "asynchronous input is closed",
            ));
        }
        #[cfg(target_arch = "wasm32")]
        task.data.extend_from_slice(data);
        Ok(())
    }

    pub fn wait(&self, id: i32) -> Result<Option<(ReaderConfig, ReaderResult)>> {
        let task = self.0.lock().unwrap().pending.remove(&id);
        task.map(|task| {
            #[cfg(not(target_arch = "wasm32"))]
            let result = task.worker.wait()?;
            #[cfg(target_arch = "wasm32")]
            let result = task.config.read(task.data)?;
            Ok((task.config, result))
        })
        .transpose()
    }
}
