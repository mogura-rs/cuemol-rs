use egui::{Color32, Context, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Activity {
    Explorer,
    View,
    Selection,
    Crystal,
    Actions,
    Agent,
}
impl Activity {
    pub const ALL: [Self; 6] = [
        Self::Explorer,
        Self::View,
        Self::Selection,
        Self::Crystal,
        Self::Actions,
        Self::Agent,
    ];
    pub fn title(self) -> &'static str {
        match self {
            Self::Explorer => "Explorer",
            Self::View => "View",
            Self::Selection => "Selection",
            Self::Crystal => "Crystal",
            Self::Actions => "Actions",
            Self::Agent => "AI Agent",
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Self::Explorer => "explorer",
            Self::View => "view",
            Self::Selection => "selection",
            Self::Crystal => "crystal",
            Self::Actions => "actions",
            Self::Agent => "agent",
        }
    }
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|view| view.key() == value)
    }
    fn icon(self) -> Icon {
        match self {
            Self::Explorer => Icon::Explorer,
            Self::View => Icon::Camera,
            Self::Selection => Icon::Selection,
            Self::Crystal => Icon::Crystal,
            Self::Actions => Icon::History,
            Self::Agent => Icon::Agent,
        }
    }
}
#[derive(Debug, Clone, Copy)]
pub enum Icon {
    Explorer,
    Camera,
    Selection,
    Crystal,
    History,
    Agent,
    Settings,
    New,
    Open,
    Save,
    Reload,
    Render,
    Undo,
    Redo,
    Rotate,
    Fit,
    Measure,
    Lasso,
    Angle,
    Torsion,
    Bond,
    File,
    Fetch,
    Eye,
    EyeClosed,
    Atom,
    Console,
    Output,
    Sequence,
    Animation,
    Trajectory,
    Down,
    Right,
    Close,
    Play,
    Stop,
    Up,
    Delete,
    Color,
    Lock,
    Unlock,
}

pub fn theme(ctx: &Context, light: bool) {
    let key = egui::Id::new("cuemol-shell-theme");
    if ctx.data(|data| data.get_temp::<bool>(key)) == Some(light) {
        return;
    }
    ctx.data_mut(|data| data.insert_temp(key, light));
    let mut style = (*ctx.style()).clone();
    let color = |dark: [u8; 3], bright: [u8; 3]| {
        let v = if light { bright } else { dark };
        Color32::from_rgb(v[0], v[1], v[2])
    };
    let surface = color([37, 40, 48], [255, 255, 255]);
    let border = color([51, 55, 70], [208, 209, 214]);
    let text = color([201, 205, 214], [30, 30, 30]);
    let accent = color([95, 175, 215], [41, 121, 181]);
    let mut v = if light {
        egui::Visuals::light()
    } else {
        egui::Visuals::dark()
    };
    v.panel_fill = surface;
    v.window_fill = surface;
    v.extreme_bg_color = color([24, 26, 32], [255, 255, 255]);
    v.faint_bg_color = color([44, 47, 58], [247, 247, 248]);
    v.override_text_color = Some(text);
    v.window_stroke = Stroke::new(1., border);
    v.window_corner_radius = 4.into();
    v.selection.bg_fill = color([58, 63, 79], [216, 218, 224]);
    v.selection.stroke = Stroke::new(1., accent);
    v.hyperlink_color = accent;
    for widget in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        widget.corner_radius = 3.into();
        widget.fg_stroke = Stroke::new(1., text);
        widget.bg_stroke = Stroke::new(1., border);
    }
    v.widgets.noninteractive.bg_fill = surface;
    v.widgets.inactive.bg_fill = color([44, 47, 58], [247, 247, 248]);
    v.widgets.hovered.bg_fill = color([51, 55, 70], [228, 229, 232]);
    v.widgets.active.bg_fill = color([58, 63, 79], [216, 218, 224]);
    style.visuals = v;
    style.spacing.item_spacing = egui::vec2(4., 4.);
    style.spacing.button_padding = egui::vec2(6., 3.);
    style.spacing.interact_size = egui::vec2(24., 22.);
    style.spacing.indent = 14.;
    style.spacing.combo_width = 120.;
    style
        .text_styles
        .insert(egui::TextStyle::Body, egui::FontId::proportional(13.));
    style
        .text_styles
        .insert(egui::TextStyle::Button, egui::FontId::proportional(12.));
    style
        .text_styles
        .insert(egui::TextStyle::Small, egui::FontId::proportional(11.5));
    style
        .text_styles
        .insert(egui::TextStyle::Monospace, egui::FontId::monospace(13.));
    style
        .text_styles
        .insert(egui::TextStyle::Heading, egui::FontId::proportional(15.));
    ctx.set_style(style);
}
pub fn header(ui: &mut Ui, title: &str) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 30.), Sense::hover());
    ui.painter().rect_filled(
        rect,
        0.,
        if ui.visuals().dark_mode {
            Color32::from_rgb(33, 35, 43)
        } else {
            Color32::from_rgb(232, 233, 236)
        },
    );
    let icon = match title {
        "Explorer" => Icon::Explorer,
        "View" => Icon::Camera,
        "Selection" => Icon::Selection,
        "Crystal" => Icon::Crystal,
        "Actions" => Icon::History,
        "AI Agent" => Icon::Agent,
        _ => Icon::Settings,
    };
    paint_icon(
        ui,
        icon,
        Rect::from_center_size(
            rect.left_center() + egui::vec2(13., 0.),
            egui::vec2(14., 14.),
        ),
        ui.visuals().weak_text_color(),
    );
    ui.painter().text(
        rect.left_center() + egui::vec2(26., 0.),
        egui::Align2::LEFT_CENTER,
        title.to_uppercase(),
        egui::FontId::proportional(12.),
        ui.visuals().text_color(),
    );
}
pub fn activity_bar(ctx: &Context, active: &mut Option<Activity>) -> bool {
    activity_bar_observed(ctx, active, |_, _| {})
}

pub fn activity_bar_observed(
    ctx: &Context,
    active: &mut Option<Activity>,
    mut observe: impl FnMut(Activity, Rect),
) -> bool {
    let mut settings = false;
    egui::SidePanel::left("activity-bar")
        .exact_width(44.)
        .resizable(false)
        .frame(egui::Frame::NONE.fill(ctx.style().visuals.extreme_bg_color))
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing.y = 0.;
            ui.add_space(4.);
            for view in Activity::ALL {
                let response = icon_button(
                    ui,
                    view.icon(),
                    view.title(),
                    *active == Some(view),
                    egui::vec2(44., 44.),
                );
                observe(view, response.interact_rect);
                if response.clicked() {
                    *active = if *active == Some(view) {
                        None
                    } else {
                        Some(view)
                    };
                }
            }
            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                settings = icon_button(ui, Icon::Settings, "Settings", false, egui::vec2(44., 44.))
                    .clicked();
            });
        });
    settings
}
pub fn icon_button(ui: &mut Ui, icon: Icon, label: &str, selected: bool, size: Vec2) -> Response {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    if selected || response.hovered() {
        ui.painter().rect_filled(
            rect,
            3.,
            if selected {
                ui.visuals().selection.bg_fill
            } else {
                ui.visuals().widgets.hovered.bg_fill
            },
        );
    }
    if selected {
        ui.painter().rect_filled(
            Rect::from_min_size(
                rect.left_top() + egui::vec2(0., size.y * 0.25),
                egui::vec2(2., size.y * 0.5),
            ),
            1.,
            ui.visuals().hyperlink_color,
        );
    }
    paint_icon(
        ui,
        icon,
        Rect::from_center_size(
            rect.center(),
            egui::Vec2::splat((size.x.min(size.y) - 6.).clamp(12., 22.)),
        ),
        ui.visuals().text_color(),
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            ui.is_enabled(),
            selected,
            label,
        )
    });
    response.on_hover_text(label)
}
pub fn toolbar_button(ui: &mut Ui, icon: Icon, label: &str, collapsed: bool) -> Response {
    if collapsed {
        return icon_button(ui, icon, label, false, egui::vec2(28., 24.));
    }
    let font = egui::FontId::proportional(12.);
    let text = ui
        .painter()
        .layout_no_wrap(label.into(), font, ui.visuals().text_color());
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(text.size().x + 32., 24.), Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, 3., ui.visuals().widgets.hovered.bg_fill);
    }
    paint_icon(
        ui,
        icon,
        Rect::from_center_size(
            Pos2::new(rect.left() + 12., rect.center().y),
            egui::vec2(16., 16.),
        ),
        ui.visuals().text_color(),
    );
    ui.painter().galley(
        Pos2::new(rect.left() + 25., rect.center().y - text.size().y * 0.5),
        text,
        ui.visuals().text_color(),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    response
}

pub fn configure_fonts(fonts: &mut egui::FontDefinitions) {
    egui_phosphor::add_to_fonts(fonts, egui_phosphor::Variant::Regular);
}

pub fn paint_icon(ui: &Ui, icon: Icon, rect: Rect, color: Color32) {
    use egui_phosphor::regular as glyph;
    let text = match icon {
        Icon::Explorer => glyph::TREE_STRUCTURE,
        Icon::Camera => glyph::BINOCULARS,
        Icon::Selection => glyph::SELECTION,
        Icon::Crystal => glyph::CUBE,
        Icon::History => glyph::SCROLL,
        Icon::Agent => glyph::ROBOT,
        Icon::Settings => glyph::GEAR,
        Icon::New => glyph::PLUS,
        Icon::File => glyph::FILE,
        Icon::Open => glyph::FOLDER_OPEN,
        Icon::Fetch => glyph::CLOUD_ARROW_DOWN,
        Icon::Save => glyph::FLOPPY_DISK,
        Icon::Reload => glyph::ARROW_CLOCKWISE,
        Icon::Render => glyph::IMAGE,
        Icon::Undo => glyph::ARROW_U_UP_LEFT,
        Icon::Redo => glyph::ARROW_U_UP_RIGHT,
        Icon::Rotate => glyph::ARROWS_OUT_CARDINAL,
        Icon::Fit => glyph::CROSSHAIR,
        Icon::Measure => glyph::RULER,
        Icon::Lasso => glyph::LASSO,
        Icon::Angle => glyph::ANGLE,
        Icon::Torsion => glyph::ARROWS_CLOCKWISE,
        Icon::Bond => glyph::LINE_SEGMENT,
        Icon::Eye => glyph::EYE,
        Icon::EyeClosed => glyph::EYE_SLASH,
        Icon::Atom => glyph::ATOM,
        Icon::Console => glyph::TERMINAL_WINDOW,
        Icon::Output => glyph::TERMINAL,
        Icon::Sequence => glyph::LIST,
        Icon::Animation => glyph::FILM_STRIP,
        Icon::Trajectory => glyph::WAVES,
        Icon::Down => glyph::CARET_DOWN,
        Icon::Right => glyph::CARET_RIGHT,
        Icon::Close => glyph::X,
        Icon::Play => glyph::PLAY,
        Icon::Stop => glyph::STOP,
        Icon::Up => glyph::ARROW_UP,
        Icon::Delete => glyph::TRASH,
        Icon::Color => glyph::DROP,
        Icon::Lock => glyph::LOCK_SIMPLE,
        Icon::Unlock => glyph::LOCK_SIMPLE_OPEN,
    };
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        egui::FontId::proportional(rect.height()),
        color,
    );
}

pub const BOTTOM_TABS: [&str; 5] = [
    "Console",
    "Output",
    "Sequence",
    "Animation",
    "MD Trajectory",
];

pub fn compact_tabs(ui: &Ui) -> bool {
    let labels = BOTTOM_TABS
        .iter()
        .map(|label| {
            ui.painter()
                .layout_no_wrap(
                    label.to_uppercase(),
                    egui::FontId::proportional(12.),
                    ui.visuals().text_color(),
                )
                .size()
                .x
                + 34.
        })
        .sum::<f32>();
    labels + 20. + BOTTOM_TABS.len() as f32 * ui.spacing().item_spacing.x > ui.available_width()
}

pub fn tab_strip(ui: &mut Ui, add_tabs: impl FnOnce(&mut Ui)) {
    let width = (ui.available_width() - 20. - ui.spacing().item_spacing.x).max(0.);
    egui::ScrollArea::horizontal()
        .id_salt("bottom-tabs")
        .max_width(width)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            ui.horizontal(add_tabs);
        });
}

pub fn tab(ui: &mut Ui, label: &str, selected: bool, compact: bool) -> Response {
    let icon = match label {
        "Console" => Icon::Console,
        "Output" => Icon::Output,
        "Sequence" => Icon::Sequence,
        "Animation" => Icon::Animation,
        "MD Trajectory" => Icon::Trajectory,
        _ => Icon::Render,
    };
    let text = ui.painter().layout_no_wrap(
        label.to_uppercase(),
        egui::FontId::proportional(12.),
        ui.visuals().text_color(),
    );
    let width = if compact { 28. } else { text.size().x + 34. };
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 28.), Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, 0., ui.visuals().widgets.hovered.bg_fill);
    }
    if selected {
        ui.painter().line_segment(
            [rect.left_bottom(), rect.right_bottom()],
            Stroke::new(2., ui.visuals().hyperlink_color),
        );
    }
    paint_icon(
        ui,
        icon,
        Rect::from_center_size(
            if compact {
                rect.center()
            } else {
                rect.left_center() + egui::vec2(12., 0.)
            },
            egui::vec2(14., 14.),
        ),
        ui.visuals().weak_text_color(),
    );
    if !compact {
        ui.painter().galley(
            rect.left_center() + egui::vec2(24., -text.size().y * 0.5),
            text,
            ui.visuals().text_color(),
        );
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            ui.is_enabled(),
            selected,
            label,
        )
    });
    response.on_hover_text(label)
}

pub fn visibility(ui: &mut Ui, visible: &mut bool) -> Response {
    let mut response = icon_button(
        ui,
        if *visible { Icon::Eye } else { Icon::EyeClosed },
        if *visible { "Hide" } else { "Show" },
        false,
        egui::vec2(22., 22.),
    );
    if response.clicked() {
        *visible = !*visible;
        response.mark_changed();
    }
    response
}

pub fn tree_row(
    ui: &mut Ui,
    label: &str,
    icon: Icon,
    selected: bool,
    open: Option<&mut bool>,
    visible: Option<&mut bool>,
) -> Response {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 22.), Sense::hover());
    let left = rect.left() + 18.;
    let right = rect.right() - if visible.is_some() { 24. } else { 0. };
    let response = ui.interact(
        Rect::from_min_max(
            egui::pos2(left, rect.top()),
            egui::pos2(right, rect.bottom()),
        ),
        ui.id().with((label, "row")),
        Sense::click_and_drag(),
    );
    if selected || response.hovered() {
        ui.painter().rect_filled(
            rect,
            0.,
            if selected {
                ui.visuals().selection.bg_fill
            } else {
                ui.visuals().widgets.hovered.bg_fill
            },
        );
    }
    if let Some(open) = open {
        let chevron = Rect::from_min_size(rect.min, egui::vec2(18., 22.));
        if ui
            .interact(chevron, ui.id().with((label, "expand")), Sense::click())
            .clicked()
        {
            *open = !*open;
        }
        paint_icon(
            ui,
            if *open { Icon::Down } else { Icon::Right },
            Rect::from_center_size(chevron.center(), egui::vec2(12., 12.)),
            ui.visuals().text_color(),
        );
    }
    paint_icon(
        ui,
        icon,
        Rect::from_center_size(egui::pos2(left + 8., rect.center().y), egui::vec2(14., 14.)),
        ui.visuals().weak_text_color(),
    );
    ui.painter().with_clip_rect(response.rect).text(
        egui::pos2(left + 20., rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(13.),
        ui.visuals().text_color(),
    );
    if let Some(visible) = visible {
        let eye = Rect::from_min_size(egui::pos2(right, rect.top()), egui::vec2(24., 22.));
        let eye_response = ui.interact(eye, ui.id().with((label, "visible")), Sense::click());
        if eye_response.clicked() {
            *visible = !*visible;
        }
        paint_icon(
            ui,
            if *visible { Icon::Eye } else { Icon::EyeClosed },
            Rect::from_center_size(eye.center(), egui::vec2(14., 14.)),
            ui.visuals().weak_text_color(),
        );
        eye_response.on_hover_text(if *visible { "Hide" } else { "Show" });
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            ui.is_enabled(),
            selected,
            label,
        )
    });
    response.on_hover_text(label)
}

pub fn logo(ui: &mut Ui, size: f32) {
    let key = egui::Id::new("cuemol-rs-logo");
    let cached = ui
        .ctx()
        .data(|data| data.get_temp::<egui::TextureHandle>(key));
    let texture = cached.unwrap_or_else(|| {
        let image = image::load_from_memory(include_bytes!("../../../cuemol-rs.png"))
            .expect("application logo")
            .resize(256, 256, image::imageops::FilterType::Lanczos3)
            .into_rgba8();
        let color = egui::ColorImage::from_rgba_unmultiplied(
            [image.width() as usize, image.height() as usize],
            image.as_raw(),
        );
        let texture = ui
            .ctx()
            .load_texture("cuemol-rs-logo", color, egui::TextureOptions::LINEAR);
        ui.ctx()
            .data_mut(|data| data.insert_temp(key, texture.clone()));
        texture
    });
    ui.image((texture.id(), egui::vec2(size, size)));
}
