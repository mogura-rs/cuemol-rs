use std::collections::VecDeque;

struct Entry {
    family: String,
    scale: u32,
    texture: egui::TextureHandle,
    size: egui::Vec2,
}

#[derive(Default)]
pub(super) struct FontPreviews {
    entries: VecDeque<Entry>,
}

impl std::fmt::Debug for FontPreviews {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FontPreviews")
            .field("cached", &self.entries.len())
            .finish()
    }
}

impl FontPreviews {
    pub(super) fn clear(&mut self) {
        self.entries.clear();
    }

    pub(super) fn option(
        &mut self,
        ui: &mut egui::Ui,
        family: &str,
        selected: bool,
    ) -> egui::Response {
        let response = ui.add_sized(
            [ui.available_width().max(120.), 24.],
            egui::Button::selectable(selected, " "),
        );
        response.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::SelectableLabel,
                ui.is_enabled(),
                selected,
                family,
            )
        });
        if !ui.is_rect_visible(response.rect) {
            return response;
        }
        let scale = ui.ctx().pixels_per_point().clamp(0.5, 4.);
        if let Some(index) = self
            .entries
            .iter()
            .position(|entry| entry.family == family && entry.scale == scale.to_bits())
        {
            let entry = self.entries.remove(index).unwrap();
            self.entries.push_front(entry);
        } else if let Some(image) =
            cuemol_desktop_core::font_catalog::preview(family, family, scale)
        {
            let size = egui::vec2(image.width as f32 / scale, image.height as f32 / scale);
            let pixels = egui::ColorImage::new(
                [image.width as usize, image.height as usize],
                image
                    .alpha
                    .into_iter()
                    .map(egui::Color32::from_white_alpha)
                    .collect(),
            );
            let texture = ui.ctx().load_texture(
                "inspector-font-preview",
                pixels,
                egui::TextureOptions::LINEAR,
            );
            self.entries.push_front(Entry {
                family: family.into(),
                scale: scale.to_bits(),
                texture,
                size,
            });
            self.entries.truncate(64);
        }
        let painter = ui
            .painter()
            .with_clip_rect(response.rect.shrink2(egui::vec2(6., 0.)));
        if let Some(entry) = self
            .entries
            .front()
            .filter(|entry| entry.family == family && entry.scale == scale.to_bits())
        {
            let min = egui::pos2(
                response.rect.left() + 8.,
                response.rect.center().y - entry.size.y * 0.5,
            );
            painter.image(
                entry.texture.id(),
                egui::Rect::from_min_size(min, entry.size),
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1., 1.)),
                ui.visuals().text_color(),
            );
        } else {
            painter.text(
                response.rect.left_center() + egui::vec2(8., 0.),
                egui::Align2::LEFT_CENTER,
                family,
                egui::TextStyle::Body.resolve(ui.style()),
                ui.visuals().text_color(),
            );
        }
        response.on_hover_text(family)
    }
}
