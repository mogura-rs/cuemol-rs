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
    ui.painter().text(
        rect.left_center() + egui::vec2(8., 0.),
        egui::Align2::LEFT_CENTER,
        title,
        egui::FontId::proportional(13.),
        ui.visuals().text_color(),
    );
}
pub fn activity_bar(ctx: &Context, active: &mut Option<Activity>) -> bool {
    let mut settings = false;
    egui::SidePanel::left("activity-bar")
        .exact_width(44.)
        .resizable(false)
        .frame(egui::Frame::NONE.fill(ctx.style().visuals.extreme_bg_color))
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing.y = 0.;
            for view in Activity::ALL {
                if icon_button(
                    ui,
                    view.icon(),
                    view.title(),
                    *active == Some(view),
                    egui::vec2(44., 44.),
                )
                .clicked()
                {
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
        Rect::from_center_size(rect.center(), egui::vec2(22., 22.)),
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
        return icon_button(ui, icon, label, false, egui::vec2(28., 26.));
    }
    let font = egui::FontId::proportional(12.);
    let text = ui
        .painter()
        .layout_no_wrap(label.into(), font, ui.visuals().text_color());
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(text.size().x + 32., 26.), Sense::click());
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

fn paint_icon(ui: &Ui, icon: Icon, rect: Rect, color: Color32) {
    let p = ui.painter();
    let s = Stroke::new(1.4, color);
    let at = |x: f32, y: f32| {
        Pos2::new(
            rect.left() + x * rect.width() / 24.,
            rect.top() + y * rect.height() / 24.,
        )
    };
    let line = |points: &[(f32, f32)]| {
        p.add(egui::Shape::line(
            points.iter().map(|&(x, y)| at(x, y)).collect(),
            s,
        ));
    };
    let box_ = |x: f32, y: f32, w: f32, h: f32| {
        p.rect_stroke(
            Rect::from_min_max(at(x, y), at(x + w, y + h)),
            2.,
            s,
            egui::StrokeKind::Inside,
        );
    };
    match icon {
        Icon::Explorer => {
            box_(4., 2., 13., 17.);
            line(&[(8., 6.), (20., 6.), (20., 22.), (8., 22.)]);
        }
        Icon::Camera | Icon::Render => {
            box_(2., 7., 20., 14.);
            line(&[(6., 7.), (8., 3.), (16., 3.), (18., 7.)]);
            p.circle_stroke(at(12., 14.), rect.width() * 0.19, s);
        }
        Icon::Lasso => {
            line(&[
                (7., 18.),
                (3., 12.),
                (4., 6.),
                (12., 3.),
                (20., 6.),
                (21., 13.),
                (15., 18.),
                (7., 18.),
                (8., 23.),
            ]);
        }
        Icon::Angle => {
            line(&[(5., 3.), (5., 20.), (22., 20.)]);
            line(&[(5., 12.), (11., 14.), (13., 20.)]);
        }
        Icon::Torsion => {
            line(&[(3., 17.), (9., 6.), (15., 18.), (21., 7.)]);
            for (x, y) in [(3., 17.), (9., 6.), (15., 18.), (21., 7.)] {
                p.circle_filled(at(x, y), 1.8, color);
            }
        }
        Icon::Bond => {
            line(&[(6., 18.), (18., 6.)]);
            p.circle_stroke(at(5., 19.), rect.width() * 0.14, s);
            p.circle_stroke(at(19., 5.), rect.width() * 0.14, s);
        }
        Icon::Selection => {
            for (x, y, sx, sy) in [
                (3., 3., 1., 1.),
                (21., 3., -1., 1.),
                (3., 21., 1., -1.),
                (21., 21., -1., -1.),
            ] {
                line(&[(x, y + sy * 5.), (x, y), (x + sx * 5., y)]);
            }
            line(&[(9., 9.), (17., 13.), (13., 14.), (12., 18.), (9., 9.)]);
        }
        Icon::Crystal => {
            line(&[
                (12., 2.),
                (22., 7.),
                (22., 17.),
                (12., 22.),
                (2., 17.),
                (2., 7.),
                (12., 2.),
                (12., 22.),
            ]);
            line(&[(2., 7.), (12., 12.), (22., 7.)]);
        }
        Icon::History => {
            p.circle_stroke(at(13., 12.), rect.width() * 0.38, s);
            line(&[(3., 3.), (3., 9.), (8., 9.)]);
            line(&[(13., 6.), (13., 12.), (17., 15.)]);
        }
        Icon::Agent => {
            box_(3., 7., 18., 14.);
            line(&[(12., 2.), (12., 7.)]);
            p.circle_filled(at(8., 13.), 1.5, color);
            p.circle_filled(at(16., 13.), 1.5, color);
            line(&[(8., 17.), (16., 17.)]);
        }
        Icon::Settings => {
            p.circle_stroke(at(12., 12.), rect.width() * 0.29, s);
            p.circle_stroke(at(12., 12.), rect.width() * 0.11, s);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.;
                line(&[
                    (12. + a.cos() * 8., 12. + a.sin() * 8.),
                    (12. + a.cos() * 11., 12. + a.sin() * 11.),
                ]);
            }
        }
        Icon::New => {
            box_(3., 3., 18., 18.);
            line(&[(12., 7.), (12., 17.)]);
            line(&[(7., 12.), (17., 12.)]);
        }
        Icon::Open => {
            line(&[
                (2., 19.),
                (2., 5.),
                (9., 5.),
                (11., 8.),
                (21., 8.),
                (21., 11.),
                (6., 11.),
                (2., 21.),
                (19., 21.),
                (22., 11.),
            ]);
        }
        Icon::Save => {
            box_(3., 3., 18., 18.);
            box_(7., 3., 10., 6.);
            box_(7., 14., 10., 7.);
        }
        Icon::Reload | Icon::Rotate => {
            p.circle_stroke(at(12., 12.), rect.width() * 0.36, s);
            line(&[(21., 3.), (21., 10.), (15., 10.)]);
        }
        Icon::Undo => {
            line(&[(9., 4.), (3., 10.), (9., 16.)]);
            line(&[(3., 10.), (15., 10.), (20., 13.), (20., 20.)]);
        }
        Icon::Redo => {
            line(&[(15., 4.), (21., 10.), (15., 16.)]);
            line(&[(21., 10.), (9., 10.), (4., 13.), (4., 20.)]);
        }
        Icon::Fit => {
            line(&[(2., 8.), (2., 2.), (8., 2.)]);
            line(&[(16., 2.), (22., 2.), (22., 8.)]);
            line(&[(2., 16.), (2., 22.), (8., 22.)]);
            line(&[(16., 22.), (22., 22.), (22., 16.)]);
            p.circle_stroke(at(12., 12.), rect.width() * 0.16, s);
        }
        Icon::Measure => {
            line(&[(2., 17.), (17., 2.), (22., 7.), (7., 22.), (2., 17.)]);
            for n in [6., 10., 14.] {
                line(&[(n, 19. - n), (n + 3., 22. - n)]);
            }
        }
    }
}
