use super::*;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", content = "options", rename_all = "camelCase")]
pub enum GuiFileReaderOptions {
    Automatic,
    Molecule(GuiMoleculeReaderOptions),
    Ccp4(GuiCcp4MapReaderOptions),
    Mtz(GuiMtzMapReaderOptions),
    Amber,
    Namd,
    Msms { primary_vertices: bool },
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiFileOpenMetadata {
    #[serde(default)]
    pub object_class: String,
    pub target: GuiFileRendererTarget,
    pub reader: GuiFileReaderOptions,
    pub map_header: Option<GuiMapHeader>,
}

impl GuiFileOpenMetadata {
    pub fn inspect_compressed_cif(name: &str, bytes: &[u8]) -> Result<Option<Self>> {
        let mut writer = CifInspection::default();
        let result = cuemol_core::compressed::copy_bytes_to(bytes, &mut writer, 0);
        if !writer.found {
            result?;
        }
        Self::inspect(name, &writer.tail, None)
    }

    pub fn inspect(
        name: &str,
        prefix: &[u8],
        mtz: Option<&cuemol_xtal::mtz::MtzFile>,
    ) -> Result<Option<Self>> {
        use GuiFileReaderOptions as Reader;
        let extension = effective_extension(Path::new(name));
        let sniff = |kind| {
            cuemol_molstr::stream::sniff_bytes(prefix, kind) == cuemol_molstr::ContentVerdict::Yes
        };
        let (target, reader) = match extension.as_str() {
            "pdb" | "ent" => (
                GuiFileRendererTarget::Molecule,
                Reader::Molecule(molecule_reader_options_for_format("pdb").unwrap()),
            ),
            "cif" | "mmcif" => {
                if [
                    "_refln.pdbx_FWT",
                    "_refln.pdbx_DELFWT",
                    "_refln.pdbx_PHWT",
                    "_refln.pdbx_DELPHWT",
                ]
                .iter()
                .any(|tag| byte_index(prefix, tag.as_bytes()).is_some())
                {
                    (GuiFileRendererTarget::DensityMap, Reader::Automatic)
                } else {
                    (
                        GuiFileRendererTarget::Molecule,
                        Reader::Molecule(molecule_reader_options_for_format("mmcif").unwrap()),
                    )
                }
            }
            "prmtop" | "parm7" | "top" => (GuiFileRendererTarget::Molecule, Reader::Amber),
            "coor" => (GuiFileRendererTarget::Molecule, Reader::Namd),
            "map" if sniff(cuemol_molstr::HandlerSniffer::XplorMap) => {
                (GuiFileRendererTarget::DensityMap, Reader::Automatic)
            }
            "map" | "ccp4" | "mrc" => (
                GuiFileRendererTarget::DensityMap,
                Reader::Ccp4(GuiCcp4MapReaderOptions::default()),
            ),
            "mtz" => (
                GuiFileRendererTarget::DensityMap,
                Reader::Mtz(mtz_map_reader_options(
                    mtz.ok_or_else(|| CueMolError::parse("MTZ metadata is required"))?,
                )?),
            ),
            "face" | "vert" => (
                GuiFileRendererTarget::Surface,
                Reader::Msms {
                    primary_vertices: extension == "vert",
                },
            ),
            "obj" | "ply" => (GuiFileRendererTarget::Surface, Reader::Automatic),
            "dx" | "cns" | "brix" | "omap" => {
                (GuiFileRendererTarget::DensityMap, Reader::Automatic)
            }
            "sdf" | "mol" | "mol2" | "gro" | "psf" | "qdfpdb" => {
                (GuiFileRendererTarget::Molecule, Reader::Automatic)
            }
            "qdf" => {
                if sniff(cuemol_molstr::HandlerSniffer::QdfMol) {
                    (GuiFileRendererTarget::Molecule, Reader::Automatic)
                } else if sniff(cuemol_molstr::HandlerSniffer::QdfMap)
                    || sniff(cuemol_molstr::HandlerSniffer::QdfPot)
                {
                    (GuiFileRendererTarget::DensityMap, Reader::Automatic)
                } else if sniff(cuemol_molstr::HandlerSniffer::QdfSurf) {
                    (GuiFileRendererTarget::Surface, Reader::Automatic)
                } else {
                    return Ok(None);
                }
            }
            _ => return Ok(None),
        };
        let map_header = matches!(reader, Reader::Ccp4(_))
            .then(|| GuiMapHeader::from_bytes(prefix))
            .transpose()?;
        Ok(Some(Self {
            object_class: if target == GuiFileRendererTarget::DensityMap
                && (extension == "dx" || sniff(cuemol_molstr::HandlerSniffer::QdfPot))
            {
                "ElePotMap"
            } else {
                target.object_class()
            }
            .into(),
            target,
            reader,
            map_header,
        }))
    }
}

#[derive(Default)]
struct CifInspection {
    tail: Vec<u8>,
    found: bool,
}

impl std::io::Write for CifInspection {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        for chunk in bytes.chunks(65536) {
            self.tail.extend_from_slice(chunk);
            if [
                "_atom_site.",
                "_refln.pdbx_FWT",
                "_refln.pdbx_DELFWT",
                "_refln.pdbx_PHWT",
                "_refln.pdbx_DELPHWT",
            ]
            .iter()
            .any(|tag| byte_index(&self.tail, tag.as_bytes()).is_some())
            {
                self.found = true;
                return Err(std::io::Error::other("CIF table metadata found"));
            }
            if self.tail.len() > 64 {
                self.tail.drain(..self.tail.len() - 64);
            }
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl GuiFileReaderOptions {
    pub fn companion_label(&self) -> Option<&'static str> {
        match self {
            Self::Amber => Some("Coordinate file"),
            Self::Namd => Some("PSF topology file"),
            Self::Msms {
                primary_vertices: true,
            } => Some("Face file"),
            Self::Msms {
                primary_vertices: false,
            } => Some("Vertex file"),
            _ => None,
        }
    }

    pub fn companion_required(&self) -> bool {
        matches!(self, Self::Namd | Self::Msms { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn roundtrip(
        name: &str,
        bytes: &[u8],
        options: &GuiFileReaderOptions,
        companion: Option<&[u8]>,
    ) -> DesktopAppState {
        let json = serde_json::to_string(options).unwrap();
        let options = serde_json::from_str(&json).unwrap();
        let prepared =
            PreparedFile::parse_with_file_options(name, bytes, &options, companion).unwrap();
        let prepared = PreparedFile::decode(&prepared.encode().unwrap()).unwrap();
        let mut state = DesktopAppState::new();
        state.open_prepared(prepared).unwrap();
        state
    }
    #[test]
    fn cif_metadata_scan_discards_large_preambles_and_keeps_split_tags() {
        use std::io::Write;
        let mut writer = CifInspection::default();
        for _ in 0..128 {
            writer.write_all(&vec![b' '; 65536]).unwrap();
            assert!(writer.tail.len() <= 64);
        }
        writer.write_all(b"loop_\n_refln.pdbx_").unwrap();
        assert!(writer.write_all(b"FWT\n").is_err());
        assert!(writer.found);
        let metadata = GuiFileOpenMetadata::inspect("map.cif", &writer.tail, None)
            .unwrap()
            .unwrap();
        assert_eq!(metadata.target, GuiFileRendererTarget::DensityMap);
    }

    #[test]
    fn prepared_reader_options_preserve_models_coordinates_and_map_kind() {
        let atom =
            "ATOM      1  CA  GLY A   1       1.000   2.000   3.000  1.00 10.00           C  \n";
        let input = format!("MODEL        1\n{atom}ENDMDL\nMODEL        2\n{atom}ENDMDL\nEND\n");
        let mut metadata = GuiFileOpenMetadata::inspect("model.pdb", input.as_bytes(), None)
            .unwrap()
            .unwrap();
        let state = roundtrip("model.pdb", input.as_bytes(), &metadata.reader, None);
        assert_eq!(state.active_object_summaries()[0].atom_count, 1);
        let GuiFileReaderOptions::Molecule(options) = &mut metadata.reader else {
            panic!("molecule")
        };
        options.ignore_multiple_models = false;
        let state = roundtrip("model.pdb", input.as_bytes(), &metadata.reader, None);
        assert_eq!(state.active_object_summaries()[0].atom_count, 2);

        let topology = crate::reader_options::tests::topology();
        let coordinates = format!("Coordinates\n1\n{:12.7}{:12.7}{:12.7}\n", 1.25, -2.5, 3.75);
        let state = roundtrip(
            "model.prmtop",
            topology.as_bytes(),
            &GuiFileReaderOptions::Amber,
            Some(coordinates.as_bytes()),
        );
        let id = state.active_object_summaries()[0].uid;
        assert_eq!(
            state.scene_manager().object(id).unwrap().mol().atoms()[0].position,
            Vector4::new3(1.25, -2.5, 3.75)
        );

        let bytes = crate::reader_options::tests::test_map_bytes();
        let info = GuiFileOpenMetadata::inspect("density.mrc", &bytes[..1024], None)
            .unwrap()
            .unwrap();
        assert_eq!(info.map_header.unwrap().counts, [2, 2, 2]);
        let reader = GuiFileReaderOptions::Ccp4(GuiCcp4MapReaderOptions {
            map_type: GuiMapType::Em,
            subsample: 2,
            ..Default::default()
        });
        let state = roundtrip("density.mrc", &bytes, &reader, None);
        let map = &state.density_map_summaries(state.active_scene().unwrap())[0];
        use cuemol_qsys::ScalarObject;
        assert_eq!(state.density_maps[&map.uid].map.col_no(), 1);
        assert_eq!(state.density_map_kind(map.uid), Some(("em", "em")));
        assert_eq!(map.renderers[0].specific["region_mode_resolved"], "full");
    }
    #[test]
    fn prepared_companion_formats_require_inputs_and_surface_roundtrips_remain_portable() {
        let mut coor = 1i32.to_le_bytes().to_vec();
        for value in [1.25f64, -2.5, 3.75] {
            coor.extend_from_slice(&value.to_le_bytes());
        }
        assert!(PreparedFile::parse_with_file_options(
            "frame.coor",
            &coor,
            &GuiFileReaderOptions::Namd,
            None
        )
        .is_err());
        let psf=b"PSF\n\n       1 !NTITLE\n REMARKS tiny\n       1 !NATOM\n       1 SEG      1 LIG  C1   C       0.000000       12.0110           0\n       0 !NBOND: bonds\n";
        let state = roundtrip("frame.coor", &coor, &GuiFileReaderOptions::Namd, Some(psf));
        let id = state.active_object_summaries()[0].uid;
        assert_eq!(
            state.scene_manager().object(id).unwrap().mol().atoms()[0].position,
            Vector4::new3(1.25, -2.5, 3.75)
        );
        let vertices = b"# MSMS\n# vertices\n3\n0 0 0 0 0 1\n1 0 0 0 0 1\n0 1 0 0 0 1\n";
        let faces = b"# MSMS\n# faces\n1\n1 2 3\n";
        for primary_vertices in [false, true] {
            let (name, bytes, extra) = if primary_vertices {
                ("surface.vert", vertices.as_slice(), faces.as_slice())
            } else {
                ("surface.face", faces.as_slice(), vertices.as_slice())
            };
            let reader = GuiFileReaderOptions::Msms { primary_vertices };
            assert!(PreparedFile::parse_with_file_options(name, bytes, &reader, None).is_err());
            let mut state = roundtrip(name, bytes, &reader, Some(extra));
            let scene = state.active_scene_bytes().unwrap();
            let mut restored = DesktopAppState::new();
            restored.open_bytes("surface.qsc", &scene).unwrap();
            let surfaces = restored.active_surface_summaries();
            assert_eq!(surfaces.len(), 1);
            let loaded = restored
                .scene_manager()
                .scene(restored.active_scene().unwrap())
                .unwrap()
                .loaded_surface(surfaces[0].uid)
                .unwrap();
            let object = cuemol_surface::qdf::read_qdf_surface_bytes(&loaded.raw_data).unwrap();
            assert_eq!(object.mesh().vertex_count(), 3);
            assert_eq!(object.mesh().triangle_count(), 1);
        }
    }
    #[test]
    fn mtz_metadata_reads_header_slices_without_reflection_storage() {
        let size = 100_000_000usize;
        let offset = size - 480;
        let mut prefix = [0u8; 12];
        prefix[..4].copy_from_slice(b"MTZ ");
        prefix[4..8].copy_from_slice(&((offset / 4 + 1) as i32).to_le_bytes());
        let header = [
            "NCOL 5 1000000 0",
            "COLUMN H H 0 1",
            "COLUMN K H 0 1",
            "COLUMN L H 0 1",
            "COLUMN FWT F 0 1",
            "COLUMN PHWT P 0 1",
        ]
        .iter()
        .map(|line| format!("{line:<80}"))
        .collect::<String>();
        let metadata =
            cuemol_xtal::mtz::read_mtz_metadata_parts(&prefix, header.as_bytes(), size).unwrap();
        assert!(metadata.reflections.is_empty());
        let info = GuiFileOpenMetadata::inspect("density.mtz", &prefix, Some(&metadata))
            .unwrap()
            .unwrap();
        let GuiFileReaderOptions::Mtz(options) = info.reader else {
            panic!("MTZ")
        };
        assert_eq!(options.column_f, "FWT");
        assert_eq!(options.column_phi, "PHWT");
        assert_eq!(
            cuemol_xtal::mtz::mtz_header_offset(&prefix, size).unwrap(),
            offset
        );
        assert!(cuemol_xtal::mtz::mtz_header_offset(&prefix, offset).is_err());
    }
}
