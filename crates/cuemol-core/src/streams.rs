use crate::{ByteArray, CueMolError, LString, Result, Vector4};
use std::cell::RefCell;
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// Common input-stream interface corresponding to `qlib::InStream`.
pub trait InStream {
    fn ready(&mut self) -> Result<bool>;
    fn read_byte(&mut self) -> Result<i32>;
    fn read(&mut self, buffer: &mut [u8], offset: usize, len: usize) -> Result<usize>;
    fn skip(&mut self, len: usize) -> Result<usize>;
    fn close(&mut self) -> Result<()>;
    fn uri(&self) -> LString;

    fn read_fully(&mut self, buffer: &mut [u8], offset: usize, len: usize) -> Result<()> {
        if offset > buffer.len() || len > buffer.len() - offset {
            return Err(CueMolError::invalid_argument(
                "input buffer range is invalid",
            ));
        }
        let mut completed = 0;
        while completed < len {
            let count = self.read(buffer, offset + completed, len - completed)?;
            if count == 0 {
                return Err(CueMolError::Io(
                    "unexpected end of input stream".to_string(),
                ));
            }
            completed += count;
        }
        Ok(())
    }
}

/// Common output-stream interface corresponding to `qlib::OutStream`.
pub trait OutStream {
    fn write(&mut self, buffer: &[u8], offset: usize, len: usize) -> Result<usize>;
    fn write_byte(&mut self, value: u8) -> Result<()>;
    fn flush(&mut self) -> Result<()>;
    fn close(&mut self) -> Result<()>;
    fn uri(&self) -> LString;
}

#[derive(Debug)]
struct StrInState {
    bytes: Vec<u8>,
    offset: usize,
    closed: bool,
}

/// In-memory input stream. Clones share the same cursor, matching C++ copies.
#[derive(Debug, Clone)]
pub struct StrInStream {
    state: Rc<RefCell<StrInState>>,
}

impl StrInStream {
    pub fn new(value: impl AsRef<[u8]>) -> Self {
        Self {
            state: Rc::new(RefCell::new(StrInState {
                bytes: value.as_ref().to_vec(),
                offset: 0,
                closed: false,
            })),
        }
    }

    pub fn from_lstring(value: &LString) -> Self {
        Self::new(value.as_str().as_bytes())
    }

    pub fn from_byte_array(value: &ByteArray) -> Self {
        Self::new(value.as_bytes())
    }

    pub fn position(&self) -> usize {
        self.state.borrow().offset
    }
}

impl InStream for StrInStream {
    fn ready(&mut self) -> Result<bool> {
        let state = self.state.borrow();
        Ok(!state.closed && state.offset < state.bytes.len())
    }

    fn read_byte(&mut self) -> Result<i32> {
        let mut state = self.state.borrow_mut();
        if state.closed || state.offset >= state.bytes.len() {
            return Ok(-1);
        }
        let value = state.bytes[state.offset];
        state.offset += 1;
        Ok(i32::from(value))
    }

    fn read(&mut self, buffer: &mut [u8], offset: usize, len: usize) -> Result<usize> {
        validate_buffer_range(buffer.len(), offset, len)?;
        let mut state = self.state.borrow_mut();
        if state.closed {
            return Ok(0);
        }
        let count = len.min(state.bytes.len().saturating_sub(state.offset));
        buffer[offset..offset + count]
            .copy_from_slice(&state.bytes[state.offset..state.offset + count]);
        state.offset += count;
        Ok(count)
    }

    fn skip(&mut self, len: usize) -> Result<usize> {
        let mut state = self.state.borrow_mut();
        if state.closed {
            return Ok(0);
        }
        let count = len.min(state.bytes.len().saturating_sub(state.offset));
        state.offset += count;
        Ok(count)
    }

    fn close(&mut self) -> Result<()> {
        self.state.borrow_mut().closed = true;
        Ok(())
    }

    fn uri(&self) -> LString {
        LString::new()
    }
}

#[derive(Debug, Default)]
struct StrOutState {
    bytes: Vec<u8>,
    closed: bool,
}

/// In-memory output stream. Clones share the same accumulated buffer.
#[derive(Debug, Clone, Default)]
pub struct StrOutStream {
    state: Rc<RefCell<StrOutState>>,
}

impl StrOutStream {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_string(&self) -> LString {
        LString::from(String::from_utf8_lossy(&self.state.borrow().bytes).into_owned())
    }

    pub fn get_data(&self) -> Vec<u8> {
        self.state.borrow().bytes.clone()
    }

    pub fn get_byte_array(&self) -> Option<ByteArray> {
        let bytes = self.state.borrow();
        if bytes.bytes.is_empty() {
            None
        } else {
            Some(ByteArray::copy_from_u8(&bytes.bytes))
        }
    }
}

impl OutStream for StrOutStream {
    fn write(&mut self, buffer: &[u8], offset: usize, len: usize) -> Result<usize> {
        validate_buffer_range(buffer.len(), offset, len)?;
        let mut state = self.state.borrow_mut();
        if state.closed {
            return Ok(0);
        }
        state.bytes.extend_from_slice(&buffer[offset..offset + len]);
        Ok(len)
    }

    fn write_byte(&mut self, value: u8) -> Result<()> {
        let mut state = self.state.borrow_mut();
        if !state.closed {
            state.bytes.push(value);
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }

    fn close(&mut self) -> Result<()> {
        self.state.borrow_mut().closed = true;
        Ok(())
    }

    fn uri(&self) -> LString {
        LString::new()
    }
}

struct LineState {
    input: Box<dyn InStream>,
    buffer: Vec<u8>,
    delimiters: Vec<u8>,
    line_number: usize,
}

/// Delimiter-preserving line input stream. Clones share buffered state.
#[derive(Clone)]
pub struct LineStream {
    state: Rc<RefCell<LineState>>,
}

impl fmt::Debug for LineStream {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = self.state.borrow();
        formatter
            .debug_struct("LineStream")
            .field("line_number", &state.line_number)
            .field("buffered", &state.buffer.len())
            .finish()
    }
}

impl LineStream {
    pub fn new<S: InStream + 'static>(input: S) -> Self {
        Self {
            state: Rc::new(RefCell::new(LineState {
                input: Box::new(input),
                buffer: Vec::new(),
                delimiters: vec![b'\n'],
                line_number: 0,
            })),
        }
    }

    pub fn set_delim(&mut self, delimiters: impl AsRef<[u8]>) {
        self.state.borrow_mut().delimiters = delimiters.as_ref().to_vec();
    }

    pub fn get_delim(&self) -> LString {
        LString::from(String::from_utf8_lossy(&self.state.borrow().delimiters).into_owned())
    }

    pub fn get_line_no(&self) -> usize {
        self.state.borrow().line_number
    }

    pub fn set_line_no(&mut self, value: usize) {
        self.state.borrow_mut().line_number = value;
    }

    pub fn ready(&mut self) -> Result<bool> {
        let mut state = self.state.borrow_mut();
        if !state.buffer.is_empty() {
            return Ok(true);
        }
        state.input.ready()
    }

    pub fn read_line(&mut self) -> Result<LString> {
        let mut state = self.state.borrow_mut();
        loop {
            if let Some(position) = delimiter_position(&state.buffer, &state.delimiters) {
                let line = state.buffer.drain(..=position).collect::<Vec<_>>();
                state.line_number += 1;
                return Ok(LString::from(String::from_utf8_lossy(&line).into_owned()));
            }

            let mut chunk = [0_u8; 2047];
            let count = state.input.read(&mut chunk, 0, 2047)?;
            if count == 0 {
                let line = std::mem::take(&mut state.buffer);
                state.line_number += 1;
                return Ok(LString::from(String::from_utf8_lossy(&line).into_owned()));
            }
            state.buffer.extend_from_slice(&chunk[..count]);
        }
    }

    pub fn close(&mut self) -> Result<()> {
        self.state.borrow_mut().input.close()
    }
}

fn delimiter_position(buffer: &[u8], delimiters: &[u8]) -> Option<usize> {
    buffer.iter().position(|value| delimiters.contains(value))
}

enum FileInTarget {
    Closed,
    File(File),
    Stdin,
}

struct FileInState {
    target: FileInTarget,
    path: Option<PathBuf>,
}

/// Seekable file input stream. Clones share the file cursor.
#[derive(Clone)]
pub struct FileInStream {
    state: Rc<RefCell<FileInState>>,
}

impl fmt::Debug for FileInStream {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FileInStream")
            .field("path", &self.state.borrow().path)
            .finish()
    }
}

impl Default for FileInStream {
    fn default() -> Self {
        Self::new()
    }
}

impl FileInStream {
    pub fn new() -> Self {
        Self {
            state: Rc::new(RefCell::new(FileInState {
                target: FileInTarget::Closed,
                path: None,
            })),
        }
    }

    pub fn stdin() -> Self {
        Self {
            state: Rc::new(RefCell::new(FileInState {
                target: FileInTarget::Stdin,
                path: None,
            })),
        }
    }

    pub fn open(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let file = File::open(path).map_err(|error| CueMolError::Io(error.to_string()))?;
        let mut state = self.state.borrow_mut();
        state.target = FileInTarget::File(file);
        state.path = Some(path.to_path_buf());
        Ok(())
    }

    pub fn get_file_pos(&self) -> Result<u64> {
        let mut state = self.state.borrow_mut();
        match &mut state.target {
            FileInTarget::File(file) => file
                .stream_position()
                .map_err(|error| CueMolError::Io(error.to_string())),
            _ => Err(CueMolError::invalid_argument(
                "input stream is not seekable",
            )),
        }
    }

    pub fn set_file_pos(&mut self, position: u64) -> Result<()> {
        let mut state = self.state.borrow_mut();
        match &mut state.target {
            FileInTarget::File(file) => {
                file.seek(SeekFrom::Start(position))
                    .map_err(|error| CueMolError::Io(error.to_string()))?;
                Ok(())
            }
            _ => Err(CueMolError::invalid_argument(
                "input stream is not seekable",
            )),
        }
    }
}

impl InStream for FileInStream {
    fn ready(&mut self) -> Result<bool> {
        let mut state = self.state.borrow_mut();
        match &mut state.target {
            FileInTarget::Closed => Ok(false),
            FileInTarget::Stdin => Ok(true),
            FileInTarget::File(file) => {
                let position = file
                    .stream_position()
                    .map_err(|error| CueMolError::Io(error.to_string()))?;
                let len = file
                    .metadata()
                    .map_err(|error| CueMolError::Io(error.to_string()))?
                    .len();
                Ok(position < len)
            }
        }
    }

    fn read_byte(&mut self) -> Result<i32> {
        let mut byte = [0_u8; 1];
        if self.read(&mut byte, 0, 1)? == 0 {
            Ok(-1)
        } else {
            Ok(i32::from(byte[0]))
        }
    }

    fn read(&mut self, buffer: &mut [u8], offset: usize, len: usize) -> Result<usize> {
        validate_buffer_range(buffer.len(), offset, len)?;
        let mut state = self.state.borrow_mut();
        let destination = &mut buffer[offset..offset + len];
        match &mut state.target {
            FileInTarget::Closed => Ok(0),
            FileInTarget::File(file) => file
                .read(destination)
                .map_err(|error| CueMolError::Io(error.to_string())),
            FileInTarget::Stdin => std::io::stdin()
                .read(destination)
                .map_err(|error| CueMolError::Io(error.to_string())),
        }
    }

    fn skip(&mut self, len: usize) -> Result<usize> {
        let mut state = self.state.borrow_mut();
        match &mut state.target {
            FileInTarget::File(file) => {
                let before = file
                    .stream_position()
                    .map_err(|error| CueMolError::Io(error.to_string()))?;
                let after = file
                    .seek(SeekFrom::Current(len as i64))
                    .map_err(|error| CueMolError::Io(error.to_string()))?;
                Ok(after.saturating_sub(before) as usize)
            }
            _ => Ok(0),
        }
    }

    fn close(&mut self) -> Result<()> {
        self.state.borrow_mut().target = FileInTarget::Closed;
        Ok(())
    }

    fn uri(&self) -> LString {
        self.state
            .borrow()
            .path
            .as_ref()
            .map(|path| LString::from(path.to_string_lossy().into_owned()))
            .unwrap_or_default()
    }
}

enum FileOutTarget {
    Closed,
    File(File),
    Stdout,
    Stderr,
}

struct FileOutState {
    target: FileOutTarget,
    path: Option<PathBuf>,
}

/// Seekable file output stream. Clones share the file cursor.
#[derive(Clone)]
pub struct FileOutStream {
    state: Rc<RefCell<FileOutState>>,
}

impl fmt::Debug for FileOutStream {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FileOutStream")
            .field("path", &self.state.borrow().path)
            .finish()
    }
}

impl Default for FileOutStream {
    fn default() -> Self {
        Self::new()
    }
}

impl FileOutStream {
    pub fn new() -> Self {
        Self {
            state: Rc::new(RefCell::new(FileOutState {
                target: FileOutTarget::Closed,
                path: None,
            })),
        }
    }

    pub fn stdout() -> Self {
        Self::standard(FileOutTarget::Stdout)
    }

    pub fn stderr() -> Self {
        Self::standard(FileOutTarget::Stderr)
    }

    fn standard(target: FileOutTarget) -> Self {
        Self {
            state: Rc::new(RefCell::new(FileOutState { target, path: None })),
        }
    }

    pub fn open(&mut self, path: impl AsRef<Path>, append: bool) -> Result<()> {
        let path = path.as_ref();
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .append(append)
            .truncate(!append)
            .open(path)
            .map_err(|error| CueMolError::Io(error.to_string()))?;
        let mut state = self.state.borrow_mut();
        state.target = FileOutTarget::File(file);
        state.path = Some(path.to_path_buf());
        Ok(())
    }

    pub fn get_file_pos(&self) -> Result<u64> {
        let mut state = self.state.borrow_mut();
        match &mut state.target {
            FileOutTarget::File(file) => file
                .stream_position()
                .map_err(|error| CueMolError::Io(error.to_string())),
            _ => Err(CueMolError::invalid_argument(
                "output stream is not seekable",
            )),
        }
    }

    pub fn set_file_pos(&mut self, position: u64) -> Result<()> {
        let mut state = self.state.borrow_mut();
        match &mut state.target {
            FileOutTarget::File(file) => {
                file.seek(SeekFrom::Start(position))
                    .map_err(|error| CueMolError::Io(error.to_string()))?;
                Ok(())
            }
            _ => Err(CueMolError::invalid_argument(
                "output stream is not seekable",
            )),
        }
    }
}

impl OutStream for FileOutStream {
    fn write(&mut self, buffer: &[u8], offset: usize, len: usize) -> Result<usize> {
        validate_buffer_range(buffer.len(), offset, len)?;
        let source = &buffer[offset..offset + len];
        let mut state = self.state.borrow_mut();
        match &mut state.target {
            FileOutTarget::Closed => Ok(0),
            FileOutTarget::File(file) => file
                .write(source)
                .map_err(|error| CueMolError::Io(error.to_string())),
            FileOutTarget::Stdout => std::io::stdout()
                .write(source)
                .map_err(|error| CueMolError::Io(error.to_string())),
            FileOutTarget::Stderr => std::io::stderr()
                .write(source)
                .map_err(|error| CueMolError::Io(error.to_string())),
        }
    }

    fn write_byte(&mut self, value: u8) -> Result<()> {
        let count = self.write(&[value], 0, 1)?;
        if count == 1 {
            Ok(())
        } else {
            Err(CueMolError::Io("output stream is closed".to_string()))
        }
    }

    fn flush(&mut self) -> Result<()> {
        let mut state = self.state.borrow_mut();
        match &mut state.target {
            FileOutTarget::Closed => Ok(()),
            FileOutTarget::File(file) => file
                .flush()
                .map_err(|error| CueMolError::Io(error.to_string())),
            FileOutTarget::Stdout => std::io::stdout()
                .flush()
                .map_err(|error| CueMolError::Io(error.to_string())),
            FileOutTarget::Stderr => std::io::stderr()
                .flush()
                .map_err(|error| CueMolError::Io(error.to_string())),
        }
    }

    fn close(&mut self) -> Result<()> {
        self.flush()?;
        self.state.borrow_mut().target = FileOutTarget::Closed;
        Ok(())
    }

    fn uri(&self) -> LString {
        self.state
            .borrow()
            .path
            .as_ref()
            .map(|path| LString::from(path.to_string_lossy().into_owned()))
            .unwrap_or_default()
    }
}

/// Values accepted by [`PrintStream::print`].
pub trait PrintValue {
    fn write_to(self, stream: &mut dyn OutStream) -> Result<()>;
}

fn write_all(stream: &mut dyn OutStream, bytes: &[u8]) -> Result<()> {
    let count = stream.write(bytes, 0, bytes.len())?;
    if count == bytes.len() {
        Ok(())
    } else {
        Err(CueMolError::Io("output stream write failed".to_string()))
    }
}

impl PrintValue for bool {
    fn write_to(self, stream: &mut dyn OutStream) -> Result<()> {
        write_all(stream, if self { b"true" } else { b"false" })
    }
}

impl PrintValue for char {
    fn write_to(self, stream: &mut dyn OutStream) -> Result<()> {
        write_all(stream, (self as u32).to_string().as_bytes())
    }
}

impl PrintValue for i32 {
    fn write_to(self, stream: &mut dyn OutStream) -> Result<()> {
        write_all(stream, self.to_string().as_bytes())
    }
}

impl PrintValue for f64 {
    fn write_to(self, stream: &mut dyn OutStream) -> Result<()> {
        write_all(stream, format!("{self:.6}").as_bytes())
    }
}

impl PrintValue for &str {
    fn write_to(self, stream: &mut dyn OutStream) -> Result<()> {
        write_all(stream, self.as_bytes())
    }
}

impl PrintValue for String {
    fn write_to(self, stream: &mut dyn OutStream) -> Result<()> {
        write_all(stream, self.as_bytes())
    }
}

impl PrintValue for &LString {
    fn write_to(self, stream: &mut dyn OutStream) -> Result<()> {
        write_all(stream, self.as_str().as_bytes())
    }
}

impl PrintValue for Vector4 {
    fn write_to(self, stream: &mut dyn OutStream) -> Result<()> {
        write_all(
            stream,
            format!(
                "({:.6}, {:.6}, {:.6}, {:.6})",
                self.x, self.y, self.z, self.w
            )
            .as_bytes(),
        )
    }
}

/// Formatted output wrapper corresponding to `qlib::PrintStream`.
pub struct PrintStream {
    output: Box<dyn OutStream>,
}

impl fmt::Debug for PrintStream {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PrintStream")
            .finish_non_exhaustive()
    }
}

impl PrintStream {
    pub fn new<S: OutStream + 'static>(output: S) -> Self {
        Self {
            output: Box::new(output),
        }
    }

    pub fn print<T: PrintValue>(&mut self, value: T) -> Result<()> {
        value.write_to(self.output.as_mut())
    }

    pub fn println<T: PrintValue>(&mut self, value: T) -> Result<()> {
        self.print(value)?;
        self.println_empty()
    }

    pub fn println_empty(&mut self) -> Result<()> {
        self.output.write_byte(b'\n')
    }

    pub fn format(&mut self, args: fmt::Arguments<'_>) -> Result<()> {
        write_all(self.output.as_mut(), args.to_string().as_bytes())
    }

    pub fn formatln(&mut self, args: fmt::Arguments<'_>) -> Result<()> {
        self.format(args)?;
        self.println_empty()
    }

    pub fn repeat(&mut self, value: &str, count: usize) -> Result<()> {
        for _ in 0..count {
            self.print(value)?;
        }
        Ok(())
    }

    pub fn flush(&mut self) -> Result<()> {
        self.output.flush()
    }

    pub fn close(&mut self) -> Result<()> {
        self.output.close()
    }
}

fn validate_buffer_range(buffer_len: usize, offset: usize, len: usize) -> Result<()> {
    if offset > buffer_len || len > buffer_len - offset {
        return Err(CueMolError::invalid_argument(
            "stream buffer range is invalid",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_stream_copies_share_state_and_roundtrip_bytes() {
        let mut input = StrInStream::new(b"abcdef");
        let mut copy = input.clone();
        assert_eq!(input.read_byte().unwrap(), i32::from(b'a'));
        let mut buffer = [0_u8; 6];
        assert_eq!(copy.read(&mut buffer, 2, 3).unwrap(), 3);
        assert_eq!(&buffer[2..5], b"bcd");
        assert_eq!(input.skip(1).unwrap(), 1);
        assert_eq!(copy.read_byte().unwrap(), i32::from(b'f'));
        assert!(!input.ready().unwrap());

        let mut output = StrOutStream::new();
        let output_copy = output.clone();
        output.write(b"abcdef", 1, 3).unwrap();
        output.write_byte(b'!').unwrap();
        assert_eq!(output_copy.get_data(), b"bcd!");
        assert_eq!(output_copy.get_string().as_str(), "bcd!");
        assert_eq!(output_copy.get_byte_array().unwrap().as_bytes(), b"bcd!");
    }

    #[test]
    fn line_stream_preserves_delimiters_and_shares_line_number() {
        let input = StrInStream::new(b"one\r\ntwo\nlast");
        let mut lines = LineStream::new(input);
        lines.set_delim(b"\r\n");
        let mut copy = lines.clone();
        assert_eq!(lines.read_line().unwrap().as_str(), "one\r");
        assert_eq!(copy.get_line_no(), 1);
        assert_eq!(copy.read_line().unwrap().as_str(), "\n");
        assert_eq!(lines.read_line().unwrap().as_str(), "two\n");
        assert_eq!(lines.read_line().unwrap().as_str(), "last");
        assert!(!lines.ready().unwrap());
        assert_eq!(lines.get_line_no(), 4);
    }

    #[test]
    fn print_stream_formats_reference_primitive_shapes() {
        let output = StrOutStream::new();
        let captured = output.clone();
        let mut print = PrintStream::new(output);
        print.print(true).unwrap();
        print.print(" ").unwrap();
        print.print(42_i32).unwrap();
        print.print(" ").unwrap();
        print.print(1.25_f64).unwrap();
        print.println_empty().unwrap();
        print.repeat("x", 3).unwrap();
        print.formatln(format_args!(" {}", "done")).unwrap();
        assert_eq!(
            captured.get_string().as_str(),
            "true 42 1.250000\nxxx done\n"
        );
    }

    #[test]
    fn file_streams_share_seek_position_and_append() {
        let path = std::env::temp_dir().join("cuemol_core_file_stream_test.bin");
        let mut output = FileOutStream::new();
        output.open(&path, false).unwrap();
        let mut output_copy = output.clone();
        output.write(b"abcd", 0, 4).unwrap();
        assert_eq!(output_copy.get_file_pos().unwrap(), 4);
        output_copy.set_file_pos(1).unwrap();
        output_copy.write(b"Z", 0, 1).unwrap();
        output.close().unwrap();

        let mut append = FileOutStream::new();
        append.open(&path, true).unwrap();
        append.write(b"!", 0, 1).unwrap();
        append.close().unwrap();

        let mut input = FileInStream::new();
        input.open(&path).unwrap();
        let mut input_copy = input.clone();
        let mut bytes = [0_u8; 8];
        assert_eq!(input.read(&mut bytes, 1, 2).unwrap(), 2);
        assert_eq!(&bytes[1..3], b"aZ");
        assert_eq!(input_copy.get_file_pos().unwrap(), 2);
        input_copy.set_file_pos(0).unwrap();
        assert_eq!(input_copy.read(&mut bytes, 0, 5).unwrap(), 5);
        assert_eq!(&bytes[..5], b"aZcd!");
        assert!(!input.ready().unwrap());
        let _ = std::fs::remove_file(path);
    }
}
