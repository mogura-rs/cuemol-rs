#[cfg(not(test))]
use clipboard_rs::{Clipboard as _, ClipboardContent, ClipboardContext};
use cuemol_command::clipboard::{self, Payload};
use cuemol_desktop_core::GuiClipboardPayload;

#[derive(Default)]
pub(crate) struct Clipboard {
    #[cfg(not(test))]
    handle: Option<ClipboardContext>,
    #[cfg(test)]
    text: String,
}

impl std::fmt::Debug for Clipboard {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Clipboard").finish_non_exhaustive()
    }
}

impl Clipboard {
    #[cfg(feature = "tauri-gui")]
    pub(crate) fn text_or_payload(&mut self) -> Result<String, String> {
        self.payload()
            .map(|payload| payload.encode())
            .or_else(|_| self.read())
    }

    #[cfg(feature = "tauri-gui")]
    pub(crate) fn write_text_or_payload(&mut self, text: String) -> Result<(), String> {
        if let Ok((kind, form)) = clipboard::header(&text) {
            if let Ok(payload) = clipboard::decode(&text) {
                return self.write_payload(&GuiClipboardPayload {
                    kind,
                    form,
                    bytes: payload.bytes,
                });
            }
        }
        self.write(text)
    }
    #[cfg(not(test))]
    fn handle(&mut self) -> Result<&ClipboardContext, String> {
        if self.handle.is_none() {
            self.handle = Some(ClipboardContext::new().map_err(|error| error.to_string())?);
        }
        Ok(self.handle.as_ref().unwrap())
    }

    pub(crate) fn read(&mut self) -> Result<String, String> {
        #[cfg(test)]
        {
            Ok(self.text.clone())
        }
        #[cfg(not(test))]
        {
            self.handle()?.get_text().map_err(|error| error.to_string())
        }
    }

    pub(crate) fn write(&mut self, text: String) -> Result<(), String> {
        #[cfg(test)]
        {
            self.text = text;
            Ok(())
        }
        #[cfg(not(test))]
        {
            self.handle()?
                .set_text(text)
                .map_err(|error| error.to_string())
        }
    }

    pub(crate) fn write_payload(&mut self, payload: &GuiClipboardPayload) -> Result<(), String> {
        #[cfg(test)]
        {
            self.write(clipboard::encode(payload))
        }
        #[cfg(not(test))]
        {
            let mut items = vec![ClipboardContent::Text(clipboard::encode(payload))];
            if !cfg!(target_os = "macos") {
                if let Some((format, _, _)) = clipboard::LEGACY_FORMATS
                    .iter()
                    .find(|(_, kind, form)| *kind == payload.kind && *form == payload.form)
                {
                    items.push(ClipboardContent::Other(
                        (*format).to_string(),
                        clipboard::legacy_bytes(payload, cfg!(target_os = "windows")),
                    ));
                }
            }
            self.handle()?.set(items).map_err(|error| error.to_string())
        }
    }

    pub(crate) fn payload(&mut self) -> Result<Payload, String> {
        #[cfg(not(test))]
        {
            let handle = self.handle()?;
            let formats = handle.available_formats().unwrap_or_default();
            for (format, kind, form) in clipboard::LEGACY_FORMATS {
                if formats.iter().any(|value| value == format) {
                    if let Ok(bytes) = handle.get_buffer(format) {
                        if !bytes.is_empty() {
                            return Ok(clipboard::from_legacy(kind, form, &bytes));
                        }
                    }
                }
            }
        }
        clipboard::decode(&self.read()?).map_err(|error| error.to_string())
    }

    pub(crate) fn kind(&mut self) -> Option<&'static str> {
        #[cfg(not(test))]
        {
            let formats = self.handle().ok()?.available_formats().unwrap_or_default();
            if let Some((_, kind, _)) = clipboard::LEGACY_FORMATS
                .iter()
                .find(|(format, _, _)| formats.iter().any(|value| value == format))
            {
                return Some(kind);
            }
        }
        clipboard::header(&self.read().ok()?)
            .ok()
            .map(|(kind, _)| kind)
    }
}
