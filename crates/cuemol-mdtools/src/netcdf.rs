//! NetCDF 3 and AMBER binary trajectory reader.

use crate::trajectory::{TrajBlock, TrajBlockReader};
use cuemol_core::{CueMolError, Result};
use cuemol_qsys::{InOutHandler, ObjReader};
use std::collections::BTreeMap;
use std::path::Path;

const NC_BYTE: u32 = 1;
const NC_CHAR: u32 = 2;
const NC_SHORT: u32 = 3;
const NC_INT: u32 = 4;
const NC_FLOAT: u32 = 5;
const NC_DOUBLE: u32 = 6;
const NC_DIMENSION: u32 = 10;
const NC_VARIABLE: u32 = 11;
const NC_ATTRIBUTE: u32 = 12;

#[derive(Debug, Clone, PartialEq)]
pub enum NcValue {
    String(String),
    Number(f64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NcDim {
    pub name: String,
    pub size: u64,
}

impl NcDim {
    pub fn is_record(&self) -> bool {
        self.size == 0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NcVar {
    pub name: String,
    pub dimension_ids: Vec<usize>,
    pub data_type: u32,
    pub value_count: usize,
    pub byte_size: usize,
    pub begin: u64,
    pub is_record: bool,
    pub scale: f64,
    pub attributes: BTreeMap<String, NcValue>,
    relative_offset: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Netcdf3Frame {
    pub coordinates: Vec<f32>,
    pub cell: Option<[f64; 6]>,
}

/// NetCDF 3 classic, 64-bit offset, and CDF-5 input over an in-memory byte slice.
#[derive(Debug, Clone)]
pub struct Netcdf3InStream<'a> {
    input: &'a [u8],
    position: usize,
    header_64: bool,
    offset_64: bool,
    streaming: bool,
    record_count: u64,
    record_size: usize,
    record_data_start: usize,
    next_frame: u64,
    parsed: bool,
    dimensions: Vec<NcDim>,
    global_attributes: BTreeMap<String, NcValue>,
    variables: Vec<NcVar>,
    coordinate_variable: Option<usize>,
    cell_length_variable: Option<usize>,
    cell_angle_variable: Option<usize>,
    atom_count: usize,
    has_record_dimension: bool,
    convention: String,
}

impl<'a> Netcdf3InStream<'a> {
    pub fn new(input: &'a [u8]) -> Self {
        Self {
            input,
            position: 0,
            header_64: false,
            offset_64: false,
            streaming: false,
            record_count: 0,
            record_size: 0,
            record_data_start: 0,
            next_frame: 0,
            parsed: false,
            dimensions: Vec::new(),
            global_attributes: BTreeMap::new(),
            variables: Vec::new(),
            coordinate_variable: None,
            cell_length_variable: None,
            cell_angle_variable: None,
            atom_count: 0,
            has_record_dimension: false,
            convention: String::new(),
        }
    }

    pub fn parse_header(&mut self) -> Result<()> {
        if self.parsed {
            return Ok(());
        }
        let magic = self.read_exact(4)?;
        if &magic[..3] != b"CDF" {
            return Err(CueMolError::parse("NetCDF3: not a CDF file"));
        }
        match magic[3] {
            1 => {}
            2 => self.offset_64 = true,
            5 => {
                self.header_64 = true;
                self.offset_64 = true;
            }
            _ => {
                return Err(CueMolError::parse("NetCDF3: unsupported CDF version"));
            }
        }

        let record_count = if self.header_64 {
            self.read_u64()?
        } else {
            u64::from(self.read_u32()?)
        };
        let streaming_sentinel = if self.header_64 {
            u64::MAX
        } else {
            u64::from(u32::MAX)
        };
        self.streaming = record_count == streaming_sentinel;
        self.record_count = if self.streaming { 0 } else { record_count };

        self.dimensions = self.read_dimension_list()?;
        self.global_attributes = self.read_attribute_list()?;
        self.variables = self.read_variable_list()?;
        self.finish_header()?;
        self.parsed = true;
        Ok(())
    }

    pub fn convention(&self) -> &str {
        &self.convention
    }

    pub fn atom_count(&self) -> usize {
        self.atom_count
    }

    pub fn frame_count(&self) -> Option<usize> {
        (!self.streaming)
            .then(|| usize::try_from(self.record_count).ok())
            .flatten()
    }

    pub fn has_cell(&self) -> bool {
        self.cell_length_variable.is_some() && self.cell_angle_variable.is_some()
    }

    pub fn has_record_dimension(&self) -> bool {
        self.has_record_dimension
    }

    pub fn dimensions(&self) -> &[NcDim] {
        &self.dimensions
    }

    pub fn variables(&self) -> &[NcVar] {
        &self.variables
    }

    pub fn global_attributes(&self) -> &BTreeMap<String, NcValue> {
        &self.global_attributes
    }

    pub fn read_frame(&mut self) -> Result<Option<Netcdf3Frame>> {
        if !self.parsed {
            return Err(CueMolError::invalid_argument(
                "NetCDF3 header has not been parsed",
            ));
        }
        if !self.streaming && self.next_frame >= self.record_count {
            return Ok(None);
        }
        if self.record_size == 0 {
            return Err(CueMolError::parse("NetCDF3: empty record layout"));
        }

        let frame_index = usize::try_from(self.next_frame)
            .map_err(|_| CueMolError::parse("NetCDF3: frame index overflow"))?;
        let offset = frame_index
            .checked_mul(self.record_size)
            .and_then(|value| self.record_data_start.checked_add(value))
            .ok_or_else(|| CueMolError::parse("NetCDF3: record offset overflow"))?;
        if offset == self.input.len() && self.streaming {
            return Ok(None);
        }
        let end = offset
            .checked_add(self.record_size)
            .ok_or_else(|| CueMolError::parse("NetCDF3: record size overflow"))?;
        let record = self
            .input
            .get(offset..end)
            .ok_or_else(|| CueMolError::parse("NetCDF3: truncated record"))?;

        let coordinate_index = self
            .coordinate_variable
            .ok_or_else(|| CueMolError::parse("NetCDF3: missing coordinates variable"))?;
        let coordinates = self.extract_values(&self.variables[coordinate_index], record)?;
        let expected = self
            .atom_count
            .checked_mul(3)
            .ok_or_else(|| CueMolError::parse("NetCDF3: coordinate count overflow"))?;
        if coordinates.len() != expected {
            return Err(CueMolError::parse(format!(
                "NetCDF3: coordinates contain {} values, expected {expected}",
                coordinates.len()
            )));
        }

        let cell = match (self.cell_length_variable, self.cell_angle_variable) {
            (Some(length_index), Some(angle_index)) => {
                let lengths = self.extract_values(&self.variables[length_index], record)?;
                let angles = self.extract_values(&self.variables[angle_index], record)?;
                if lengths.len() != 3 || angles.len() != 3 {
                    return Err(CueMolError::parse(
                        "NetCDF3: cell variables must contain three values",
                    ));
                }
                Some([
                    f64::from(lengths[0]),
                    f64::from(lengths[1]),
                    f64::from(lengths[2]),
                    f64::from(angles[0]),
                    f64::from(angles[1]),
                    f64::from(angles[2]),
                ])
            }
            (None, None) => None,
            _ => {
                return Err(CueMolError::parse(
                    "NetCDF3: incomplete periodic cell variables",
                ));
            }
        };
        self.next_frame += 1;
        Ok(Some(Netcdf3Frame { coordinates, cell }))
    }

    fn finish_header(&mut self) -> Result<()> {
        self.coordinate_variable = self
            .variables
            .iter()
            .position(|variable| variable.name == "coordinates");
        self.cell_length_variable = self
            .variables
            .iter()
            .position(|variable| variable.name == "cell_lengths");
        self.cell_angle_variable = self
            .variables
            .iter()
            .position(|variable| variable.name == "cell_angles");
        self.atom_count = self
            .dimensions
            .iter()
            .find(|dimension| dimension.name == "atom")
            .map(|dimension| usize_from_u64(dimension.size, "atom dimension"))
            .transpose()?
            .unwrap_or(0);
        self.has_record_dimension = self.dimensions.iter().any(NcDim::is_record);
        self.convention = match self.global_attributes.get("Conventions") {
            Some(NcValue::String(value)) => value.trim_end_matches('\0').to_string(),
            _ => String::new(),
        };

        let record_variable_indices = self
            .variables
            .iter()
            .enumerate()
            .filter_map(|(index, variable)| variable.is_record.then_some(index))
            .collect::<Vec<_>>();
        let min_begin = record_variable_indices
            .iter()
            .map(|&index| self.variables[index].begin)
            .min()
            .unwrap_or(self.position as u64);
        self.record_data_start = usize_from_u64(min_begin, "record data offset")?;
        if self.record_data_start < self.position {
            return Err(CueMolError::parse(
                "NetCDF3: record data begins inside the header",
            ));
        }
        self.record_size = 0;
        for index in record_variable_indices {
            let relative = self.variables[index]
                .begin
                .checked_sub(min_begin)
                .ok_or_else(|| CueMolError::parse("NetCDF3: invalid record offset"))?;
            let relative = usize_from_u64(relative, "record variable offset")?;
            let byte_size = self.variables[index].byte_size;
            self.variables[index].relative_offset = relative;
            self.record_size = self
                .record_size
                .checked_add(padded_size(byte_size)?)
                .ok_or_else(|| CueMolError::parse("NetCDF3: record size overflow"))?;
        }
        for variable in self.variables.iter().filter(|variable| variable.is_record) {
            let end = variable
                .relative_offset
                .checked_add(variable.byte_size)
                .ok_or_else(|| CueMolError::parse("NetCDF3: record layout overflow"))?;
            if end > self.record_size {
                return Err(CueMolError::parse(
                    "NetCDF3: inconsistent record variable offsets",
                ));
            }
        }
        if self
            .coordinate_variable
            .is_some_and(|index| !self.variables[index].is_record)
        {
            return Err(CueMolError::parse(
                "NetCDF3: coordinates must be a record variable",
            ));
        }
        Ok(())
    }

    fn read_dimension_list(&mut self) -> Result<Vec<NcDim>> {
        let marker = self.read_u32()?;
        let count = self.read_header_count()?;
        if marker != 0 && marker != NC_DIMENSION {
            return Err(CueMolError::parse("NetCDF3: expected NC_DIMENSION tag"));
        }
        if marker == 0 && count != 0 {
            return Err(CueMolError::parse("NetCDF3: invalid absent dimension list"));
        }
        let mut dimensions = Vec::with_capacity(bounded_count(count, self.input.len())?);
        for _ in 0..count {
            dimensions.push(NcDim {
                name: self.read_name()?,
                size: self.read_header_count()?,
            });
        }
        Ok(dimensions)
    }

    fn read_attribute_list(&mut self) -> Result<BTreeMap<String, NcValue>> {
        let marker = self.read_u32()?;
        let count = self.read_header_count()?;
        if marker != 0 && marker != NC_ATTRIBUTE {
            return Err(CueMolError::parse("NetCDF3: expected NC_ATTRIBUTE tag"));
        }
        if marker == 0 && count != 0 {
            return Err(CueMolError::parse("NetCDF3: invalid absent attribute list"));
        }
        let mut attributes = BTreeMap::new();
        for _ in 0..count {
            let name = self.read_name()?;
            let value = self.read_attribute_value()?;
            attributes.insert(name, value);
        }
        Ok(attributes)
    }

    fn read_attribute_value(&mut self) -> Result<NcValue> {
        let data_type = self.read_u32()?;
        let count = self.read_header_count()?;
        let element_size = nc_type_size(data_type)?;
        let byte_count = usize_from_u64(
            count
                .checked_mul(element_size as u64)
                .ok_or_else(|| CueMolError::parse("NetCDF3: attribute size overflow"))?,
            "attribute size",
        )?;
        let bytes = self.read_exact(byte_count)?;
        let value = if data_type == NC_CHAR {
            NcValue::String(
                String::from_utf8(bytes.to_vec())
                    .map_err(|error| CueMolError::parse(format!("NetCDF3: {error}")))?,
            )
        } else {
            let number = if count == 0 {
                0.0
            } else {
                decode_numeric(data_type, bytes)?
            };
            NcValue::Number(number)
        };
        self.skip_padding(byte_count)?;
        Ok(value)
    }

    fn read_variable_list(&mut self) -> Result<Vec<NcVar>> {
        let marker = self.read_u32()?;
        let count = self.read_header_count()?;
        if marker != 0 && marker != NC_VARIABLE {
            return Err(CueMolError::parse("NetCDF3: expected NC_VARIABLE tag"));
        }
        if marker == 0 && count != 0 {
            return Err(CueMolError::parse("NetCDF3: invalid absent variable list"));
        }
        let mut variables = Vec::with_capacity(bounded_count(count, self.input.len())?);
        for _ in 0..count {
            let name = self.read_name()?;
            let dimension_count = self.read_header_count()?;
            let mut dimension_ids =
                Vec::with_capacity(bounded_count(dimension_count, self.dimensions.len())?);
            let mut is_record = false;
            for _ in 0..dimension_count {
                let id = usize_from_u64(self.read_header_count()?, "dimension id")?;
                let dimension = self
                    .dimensions
                    .get(id)
                    .ok_or_else(|| CueMolError::parse("NetCDF3: dimension id out of range"))?;
                is_record |= dimension.is_record();
                dimension_ids.push(id);
            }
            let attributes = self.read_attribute_list()?;
            let data_type = self.read_u32()?;
            let element_size = nc_type_size(data_type)?;
            let _stored_size = self.read_header_count()?;
            let begin = if self.offset_64 {
                self.read_u64()?
            } else {
                u64::from(self.read_u32()?)
            };

            let mut value_count = 1_u64;
            for &id in &dimension_ids {
                let dimension = &self.dimensions[id];
                if !dimension.is_record() {
                    value_count = value_count
                        .checked_mul(dimension.size)
                        .ok_or_else(|| CueMolError::parse("NetCDF3: variable size overflow"))?;
                }
            }
            let byte_size = value_count
                .checked_mul(element_size as u64)
                .ok_or_else(|| CueMolError::parse("NetCDF3: variable size overflow"))?;
            let scale = match attributes.get("scale_factor") {
                Some(NcValue::Number(value)) => *value,
                _ => 1.0,
            };
            variables.push(NcVar {
                name,
                dimension_ids,
                data_type,
                value_count: usize_from_u64(value_count, "variable value count")?,
                byte_size: usize_from_u64(byte_size, "variable byte size")?,
                begin,
                is_record,
                scale,
                attributes,
                relative_offset: 0,
            });
        }
        Ok(variables)
    }

    fn extract_values(&self, variable: &NcVar, record: &[u8]) -> Result<Vec<f32>> {
        if !matches!(variable.data_type, NC_FLOAT | NC_DOUBLE) {
            return Err(CueMolError::parse(
                "NetCDF3: coordinates and cell must be float or double",
            ));
        }
        let end = variable
            .relative_offset
            .checked_add(variable.byte_size)
            .ok_or_else(|| CueMolError::parse("NetCDF3: variable offset overflow"))?;
        let bytes = record
            .get(variable.relative_offset..end)
            .ok_or_else(|| CueMolError::parse("NetCDF3: truncated variable data"))?;
        let mut values = Vec::with_capacity(variable.value_count);
        for chunk in bytes.chunks_exact(nc_type_size(variable.data_type)?) {
            values.push((decode_numeric(variable.data_type, chunk)? * variable.scale) as f32);
        }
        Ok(values)
    }

    fn read_name(&mut self) -> Result<String> {
        let size = usize_from_u64(self.read_header_count()?, "name length")?;
        let bytes = self.read_exact(size)?;
        let value = String::from_utf8(bytes.to_vec())
            .map_err(|error| CueMolError::parse(format!("NetCDF3: {error}")))?;
        self.skip_padding(size)?;
        Ok(value)
    }

    fn read_header_count(&mut self) -> Result<u64> {
        if self.header_64 {
            self.read_u64()
        } else {
            Ok(u64::from(self.read_u32()?))
        }
    }

    fn read_u32(&mut self) -> Result<u32> {
        let bytes = self.read_exact(4)?;
        Ok(u32::from_be_bytes(
            bytes.try_into().expect("four-byte slice"),
        ))
    }

    fn read_u64(&mut self) -> Result<u64> {
        let bytes = self.read_exact(8)?;
        Ok(u64::from_be_bytes(
            bytes.try_into().expect("eight-byte slice"),
        ))
    }

    fn read_exact(&mut self, size: usize) -> Result<&'a [u8]> {
        let end = self
            .position
            .checked_add(size)
            .ok_or_else(|| CueMolError::parse("NetCDF3: position overflow"))?;
        let bytes = self
            .input
            .get(self.position..end)
            .ok_or_else(|| CueMolError::parse("NetCDF3: truncated input"))?;
        self.position = end;
        Ok(bytes)
    }

    fn skip_padding(&mut self, size: usize) -> Result<()> {
        let padding = (4 - size % 4) % 4;
        self.read_exact(padding).map(|_| ())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmberNetCDFReadOptions {
    pub skip_no: usize,
    pub expected_atom_count: Option<usize>,
    pub atom_indices: Option<Vec<usize>>,
}

impl Default for AmberNetCDFReadOptions {
    fn default() -> Self {
        Self {
            skip_no: 1,
            expected_atom_count: None,
            atom_indices: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmberNetCDFReader {
    reader: ObjReader,
    block_reader: TrajBlockReader,
    options: AmberNetCDFReadOptions,
}

impl AmberNetCDFReader {
    pub fn new() -> Self {
        Self {
            reader: ObjReader::new("ambnetcdftraj", "AMBER NetCDF trajectory (*.nc)", "*.nc"),
            block_reader: TrajBlockReader::new(),
            options: AmberNetCDFReadOptions::default(),
        }
    }

    pub fn reader(&self) -> &ObjReader {
        &self.reader
    }

    pub fn reader_mut(&mut self) -> &mut ObjReader {
        &mut self.reader
    }

    pub fn handler(&self) -> &InOutHandler {
        self.reader.handler()
    }

    pub fn block_reader(&self) -> &TrajBlockReader {
        &self.block_reader
    }

    pub fn block_reader_mut(&mut self) -> &mut TrajBlockReader {
        &mut self.block_reader
    }

    pub fn lazy_load(&self) -> bool {
        self.block_reader.lazy_load()
    }

    pub fn is_lazy_load(&self) -> bool {
        self.lazy_load()
    }

    pub fn set_lazy_load(&mut self, value: bool) {
        self.block_reader.set_lazy_load(value);
    }

    pub fn target_trajectory_uid(&self) -> u64 {
        self.block_reader.target_trajectory_uid()
    }

    pub fn targ_traj_uid(&self) -> u64 {
        self.target_trajectory_uid()
    }

    pub fn get_targ_traj_uid(&self) -> u64 {
        self.target_trajectory_uid()
    }

    pub fn set_targ_traj_uid(&mut self, value: u64) {
        self.block_reader.set_targ_traj_uid(value);
    }

    pub fn set_target_trajectory_uid(&mut self, value: u64) {
        self.set_targ_traj_uid(value);
    }

    pub fn options(&self) -> &AmberNetCDFReadOptions {
        &self.options
    }

    pub fn set_options(&mut self, value: AmberNetCDFReadOptions) {
        self.options = value;
    }

    pub fn skip_no(&self) -> usize {
        self.options.skip_no
    }

    pub fn get_skip_no(&self) -> usize {
        self.skip_no()
    }

    pub fn nevery(&self) -> usize {
        self.skip_no()
    }

    pub fn set_skip_no(&mut self, value: usize) {
        self.options.skip_no = value;
    }

    pub fn set_nevery(&mut self, value: usize) {
        self.set_skip_no(value);
    }

    pub fn create_default_obj(&self) -> TrajBlock {
        TrajBlock::default()
    }

    pub fn can_handle_content(&self, input: &[u8]) -> bool {
        sniff_amber_netcdf_content(input)
    }

    pub fn read_bytes(&self, input: &[u8]) -> Result<TrajBlock> {
        read_amber_netcdf_bytes_with_options(input, &self.options)
    }

    pub fn read_file(&self, path: impl AsRef<Path>) -> Result<TrajBlock> {
        read_amber_netcdf_file_with_options(path, &self.options)
    }
}

impl Default for AmberNetCDFReader {
    fn default() -> Self {
        Self::new()
    }
}

impl std::ops::Deref for AmberNetCDFReader {
    type Target = ObjReader;

    fn deref(&self) -> &Self::Target {
        &self.reader
    }
}

impl std::ops::DerefMut for AmberNetCDFReader {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.reader
    }
}

pub fn sniff_amber_netcdf_content(input: &[u8]) -> bool {
    input
        .get(..4)
        .is_some_and(|magic| &magic[..3] == b"CDF" && matches!(magic[3], 1 | 2 | 5))
}

pub fn read_amber_netcdf_file(path: impl AsRef<Path>) -> Result<TrajBlock> {
    read_amber_netcdf_file_with_options(path, &AmberNetCDFReadOptions::default())
}

pub fn read_amber_netcdf_file_with_options(
    path: impl AsRef<Path>,
    options: &AmberNetCDFReadOptions,
) -> Result<TrajBlock> {
    let input = crate::fileio::read_bytes_maybe_compressed(path)?;
    read_amber_netcdf_bytes_with_options(&input, options)
}

pub fn read_amber_netcdf_bytes(input: &[u8]) -> Result<TrajBlock> {
    read_amber_netcdf_bytes_with_options(input, &AmberNetCDFReadOptions::default())
}

pub fn read_amber_netcdf_bytes_with_options(
    input: &[u8],
    options: &AmberNetCDFReadOptions,
) -> Result<TrajBlock> {
    if options.skip_no == 0 {
        return Err(CueMolError::invalid_argument(
            "AMBER NetCDF frame skip must be at least one",
        ));
    }

    let mut netcdf = Netcdf3InStream::new(input);
    netcdf.parse_header()?;
    match netcdf.convention() {
        "AMBER" => {}
        "AMBERRESTART" => {
            return Err(CueMolError::parse(
                "AMBER NetCDF restart (AMBERRESTART) is not supported",
            ));
        }
        _ => {
            return Err(CueMolError::parse("not an AMBER NetCDF trajectory file"));
        }
    }
    let atom_count = netcdf.atom_count();
    if atom_count == 0 {
        return Err(CueMolError::parse("AMBER NetCDF: missing atom dimension"));
    }
    if let Some(expected) = options.expected_atom_count {
        if expected != atom_count {
            return Err(CueMolError::parse(format!(
                "AMBER NetCDF atom count {atom_count} does not match expected {expected}"
            )));
        }
    }
    let atom_indices = options
        .atom_indices
        .clone()
        .unwrap_or_else(|| (0..atom_count).collect());
    if atom_indices.iter().any(|&index| index >= atom_count) {
        return Err(CueMolError::invalid_argument(
            "AMBER NetCDF atom selection index out of range",
        ));
    }

    let mut block = TrajBlock::new(atom_indices.len());
    let mut frame_index = 0_usize;
    while let Some(frame) = netcdf.read_frame()? {
        if frame_index.is_multiple_of(options.skip_no) {
            let mut selected = Vec::with_capacity(atom_indices.len() * 3);
            for &atom_index in &atom_indices {
                let offset = atom_index * 3;
                selected.extend_from_slice(&frame.coordinates[offset..offset + 3]);
            }
            block.append_frame(selected, frame.cell)?;
        }
        frame_index += 1;
    }
    Ok(block)
}

fn nc_type_size(data_type: u32) -> Result<usize> {
    match data_type {
        NC_BYTE | NC_CHAR => Ok(1),
        NC_SHORT => Ok(2),
        NC_INT | NC_FLOAT => Ok(4),
        NC_DOUBLE => Ok(8),
        _ => Err(CueMolError::parse("NetCDF3: unknown data type")),
    }
}

fn decode_numeric(data_type: u32, bytes: &[u8]) -> Result<f64> {
    let value = match data_type {
        NC_BYTE => f64::from(bytes[0] as i8),
        NC_SHORT => f64::from(i16::from_be_bytes(
            bytes
                .get(..2)
                .and_then(|value| value.try_into().ok())
                .ok_or_else(|| CueMolError::parse("NetCDF3: truncated short value"))?,
        )),
        NC_INT => f64::from(i32::from_be_bytes(
            bytes
                .get(..4)
                .and_then(|value| value.try_into().ok())
                .ok_or_else(|| CueMolError::parse("NetCDF3: truncated integer value"))?,
        )),
        NC_FLOAT => f64::from(f32::from_be_bytes(
            bytes
                .get(..4)
                .and_then(|value| value.try_into().ok())
                .ok_or_else(|| CueMolError::parse("NetCDF3: truncated float value"))?,
        )),
        NC_DOUBLE => f64::from_be_bytes(
            bytes
                .get(..8)
                .and_then(|value| value.try_into().ok())
                .ok_or_else(|| CueMolError::parse("NetCDF3: truncated double value"))?,
        ),
        _ => return Err(CueMolError::parse("NetCDF3: value is not numeric")),
    };
    Ok(value)
}

fn usize_from_u64(value: u64, label: &str) -> Result<usize> {
    usize::try_from(value).map_err(|_| CueMolError::parse(format!("NetCDF3: {label} overflow")))
}

fn bounded_count(value: u64, maximum: usize) -> Result<usize> {
    let value = usize_from_u64(value, "list count")?;
    if value > maximum {
        return Err(CueMolError::parse("NetCDF3: unreasonable list count"));
    }
    Ok(value)
}

fn padded_size(size: usize) -> Result<usize> {
    size.checked_add((4 - size % 4) % 4)
        .ok_or_else(|| CueMolError::parse("NetCDF3: padded size overflow"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn push_u32(output: &mut Vec<u8>, value: u32) {
        output.extend_from_slice(&value.to_be_bytes());
    }

    fn push_u64(output: &mut Vec<u8>, value: u64) {
        output.extend_from_slice(&value.to_be_bytes());
    }

    fn push_name(output: &mut Vec<u8>, value: &str) {
        push_u32(output, value.len() as u32);
        output.extend_from_slice(value.as_bytes());
        output.resize(output.len().next_multiple_of(4), 0);
    }

    fn push_char_attribute(output: &mut Vec<u8>, name: &str, value: &str) {
        push_name(output, name);
        push_u32(output, NC_CHAR);
        push_u32(output, value.len() as u32);
        output.extend_from_slice(value.as_bytes());
        output.resize(output.len().next_multiple_of(4), 0);
    }

    fn push_name_cdf5(output: &mut Vec<u8>, value: &str) {
        push_u64(output, value.len() as u64);
        output.extend_from_slice(value.as_bytes());
        output.resize(output.len().next_multiple_of(4), 0);
    }

    fn push_char_attribute_cdf5(output: &mut Vec<u8>, name: &str, value: &str) {
        push_name_cdf5(output, name);
        push_u32(output, NC_CHAR);
        push_u64(output, value.len() as u64);
        output.extend_from_slice(value.as_bytes());
        output.resize(output.len().next_multiple_of(4), 0);
    }

    fn patch_u32(output: &mut [u8], position: usize, value: u32) {
        output[position..position + 4].copy_from_slice(&value.to_be_bytes());
    }

    fn patch_u64(output: &mut [u8], position: usize, value: u64) {
        output[position..position + 8].copy_from_slice(&value.to_be_bytes());
    }

    fn amber_fixture(atom_count: usize, frame_count: usize, version: u8) -> Vec<u8> {
        amber_fixture_with_convention(atom_count, frame_count, version, "AMBER")
    }

    fn amber_fixture_with_convention(
        atom_count: usize,
        frame_count: usize,
        version: u8,
        convention: &str,
    ) -> Vec<u8> {
        let mut output = b"CDF".to_vec();
        output.push(version);
        push_u32(&mut output, frame_count as u32);

        push_u32(&mut output, NC_DIMENSION);
        push_u32(&mut output, 5);
        for (name, size) in [
            ("spatial", 3),
            ("atom", atom_count as u32),
            ("frame", 0),
            ("cell_spatial", 3),
            ("cell_angular", 3),
        ] {
            push_name(&mut output, name);
            push_u32(&mut output, size);
        }

        push_u32(&mut output, NC_ATTRIBUTE);
        push_u32(&mut output, 2);
        push_char_attribute(&mut output, "Conventions", convention);
        push_char_attribute(&mut output, "ConventionVersion", "1.0");

        push_u32(&mut output, NC_VARIABLE);
        push_u32(&mut output, 3);
        let mut begin_positions = Vec::new();
        for (name, dimensions, units, byte_size) in [
            (
                "coordinates",
                vec![2_u32, 1, 0],
                "angstrom",
                atom_count * 12,
            ),
            ("cell_lengths", vec![2, 3], "angstrom", 12),
            ("cell_angles", vec![2, 4], "degree", 12),
        ] {
            push_name(&mut output, name);
            push_u32(&mut output, dimensions.len() as u32);
            for dimension in dimensions {
                push_u32(&mut output, dimension);
            }
            push_u32(&mut output, NC_ATTRIBUTE);
            push_u32(&mut output, 1);
            push_char_attribute(&mut output, "units", units);
            push_u32(&mut output, NC_FLOAT);
            push_u32(&mut output, byte_size as u32);
            begin_positions.push(output.len());
            if version == 1 {
                push_u32(&mut output, 0);
            } else {
                push_u64(&mut output, 0);
            }
        }

        let header_size = output.len() as u64;
        let starts = [
            header_size,
            header_size + (atom_count * 12) as u64,
            header_size + (atom_count * 12 + 12) as u64,
        ];
        for (position, start) in begin_positions.into_iter().zip(starts) {
            if version == 1 {
                patch_u32(&mut output, position, start as u32);
            } else {
                patch_u64(&mut output, position, start);
            }
        }

        for frame in 0..frame_count {
            for atom in 0..atom_count {
                for axis in 0..3 {
                    let value = (frame * 100) as f32 + atom as f32 + (axis + 1) as f32 * 0.1;
                    output.extend_from_slice(&value.to_be_bytes());
                }
            }
            for value in [10.0_f32, 11.0, 12.0, 90.0, 90.0, 90.0] {
                output.extend_from_slice(&value.to_be_bytes());
            }
        }
        output
    }

    fn cdf5_fixture() -> Vec<u8> {
        let mut output = b"CDF\x05".to_vec();
        push_u64(&mut output, 1);
        push_u32(&mut output, NC_DIMENSION);
        push_u64(&mut output, 3);
        for (name, size) in [("spatial", 3_u64), ("atom", 2), ("frame", 0)] {
            push_name_cdf5(&mut output, name);
            push_u64(&mut output, size);
        }
        push_u32(&mut output, NC_ATTRIBUTE);
        push_u64(&mut output, 1);
        push_char_attribute_cdf5(&mut output, "Conventions", "AMBER");
        push_u32(&mut output, NC_VARIABLE);
        push_u64(&mut output, 1);
        push_name_cdf5(&mut output, "coordinates");
        push_u64(&mut output, 3);
        for dimension in [2_u64, 1, 0] {
            push_u64(&mut output, dimension);
        }
        push_u32(&mut output, 0);
        push_u64(&mut output, 0);
        push_u32(&mut output, NC_FLOAT);
        push_u64(&mut output, 24);
        let begin_position = output.len();
        push_u64(&mut output, 0);
        let begin = output.len() as u64;
        patch_u64(&mut output, begin_position, begin);
        for value in [1.0_f32, 2.0, 3.0, 4.0, 5.0, 6.0] {
            output.extend_from_slice(&value.to_be_bytes());
        }
        output
    }

    #[test]
    fn parses_cdf1_and_cdf2_headers_and_records() {
        for version in [1, 2] {
            let input = amber_fixture(4, 3, version);
            let mut stream = Netcdf3InStream::new(&input);
            stream.parse_header().unwrap();
            assert_eq!(stream.convention(), "AMBER");
            assert_eq!(stream.atom_count(), 4);
            assert_eq!(stream.frame_count(), Some(3));
            assert!(stream.has_cell());
            assert!(stream.has_record_dimension());

            let frame = stream.read_frame().unwrap().unwrap();
            assert_eq!(frame.coordinates.len(), 12);
            assert!((frame.coordinates[11] - 3.3).abs() < 1.0e-5);
            assert_eq!(frame.cell, Some([10.0, 11.0, 12.0, 90.0, 90.0, 90.0]));
        }
    }

    #[test]
    fn parses_cdf5_64_bit_header_fields() {
        let input = cdf5_fixture();
        let mut stream = Netcdf3InStream::new(&input);
        stream.parse_header().unwrap();
        assert_eq!(stream.atom_count(), 2);
        assert_eq!(stream.frame_count(), Some(1));
        assert!(!stream.has_cell());
        assert_eq!(
            stream.read_frame().unwrap().unwrap().coordinates,
            [1.0, 2.0, 3.0, 4.0, 5.0, 6.0]
        );
    }

    #[test]
    fn reader_samples_selects_and_validates_atoms() {
        let input = amber_fixture(3, 5, 2);
        assert!(sniff_amber_netcdf_content(&input));
        let block = read_amber_netcdf_bytes_with_options(
            &input,
            &AmberNetCDFReadOptions {
                skip_no: 2,
                expected_atom_count: Some(3),
                atom_indices: Some(vec![2, 0]),
            },
        )
        .unwrap();
        assert_eq!(block.atom_count(), 2);
        assert_eq!(block.frame_count(), 3);
        assert!((block.frame(1).unwrap()[0] - 202.1).abs() < 1.0e-4);
        assert_eq!(block.cell(0), Some([10.0, 11.0, 12.0, 90.0, 90.0, 90.0]));

        assert!(read_amber_netcdf_bytes_with_options(
            &input,
            &AmberNetCDFReadOptions {
                expected_atom_count: Some(4),
                ..AmberNetCDFReadOptions::default()
            }
        )
        .is_err());
    }

    #[test]
    fn rejects_restart_invalid_options_and_truncation() {
        let input = amber_fixture(3, 2, 2);
        assert!(read_amber_netcdf_bytes(&input[..input.len() - 1]).is_err());
        assert!(read_amber_netcdf_bytes_with_options(
            &input,
            &AmberNetCDFReadOptions {
                skip_no: 0,
                ..AmberNetCDFReadOptions::default()
            }
        )
        .is_err());

        let restart = amber_fixture_with_convention(3, 2, 2, "AMBERRESTART");
        assert!(read_amber_netcdf_bytes(&restart).is_err());
    }
}
