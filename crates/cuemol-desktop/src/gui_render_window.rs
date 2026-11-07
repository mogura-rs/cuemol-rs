use super::*;

pub(super) struct RenderWindow {
    pub window: Arc<Window>,
    pub visible: bool,
    ctx: egui::Context,
    input: egui_winit::State,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    renderer: egui_wgpu::Renderer,
    repaint_at: Option<Instant>,
}

impl RenderWindow {
    #[cfg(target_os = "macos")]
    pub(super) fn wants_keyboard_input(&self) -> bool {
        self.ctx.wants_keyboard_input()
    }

    #[cfg(target_os = "macos")]
    pub(super) fn menu_edit(&mut self, app: &mut GuiApplication, id: &str) {
        let editing = self.ctx.wants_keyboard_input();
        if !crate::application_menu::allowed(id, app.ray_panel.confirmation_open(), editing) {
            return;
        }
        let event = match id {
            "undo" => clipboard::edit_key(egui::Key::Z, false),
            "redo" => clipboard::edit_key(egui::Key::Z, true),
            "select-all" if editing => clipboard::edit_key(egui::Key::A, false),
            "copy" if editing => egui::Event::Copy,
            "cut" if editing => egui::Event::Cut,
            "paste" if editing => match app.system_clipboard.read() {
                Ok(text) => egui::Event::Paste(text),
                Err(error) => {
                    app.ray_panel.message = format!("Clipboard error: {error}");
                    return;
                }
            },
            _ => return,
        };
        self.input.egui_input_mut().events.push(event);
        self.window.request_redraw();
    }

    fn new(
        event_loop: &ActiveEventLoop,
        gpu: &GpuState,
        state: &DesktopAppState,
        proxy: EventLoopProxy<()>,
    ) -> Result<Self> {
        let width = state
            .config_value("render-window:width")
            .parse::<u32>()
            .unwrap_or(1000)
            .clamp(480, 8192);
        let height = state
            .config_value("render-window:height")
            .parse::<u32>()
            .unwrap_or(740)
            .clamp(400, 8192);
        let mut attributes = Window::default_attributes()
            .with_title("Rendering")
            .with_window_icon(application_icon())
            .with_visible(false)
            .with_inner_size(PhysicalSize::new(width, height))
            .with_min_inner_size(winit::dpi::LogicalSize::new(480., 400.));
        if let Some((x, y)) = state
            .config_value("render-window:x")
            .parse::<i32>()
            .ok()
            .zip(state.config_value("render-window:y").parse::<i32>().ok())
        {
            attributes = attributes.with_position(PhysicalPosition::new(x, y));
        }
        let window = Arc::new(event_loop.create_window(attributes).map_err(gui_error)?);
        let surface = gpu
            .instance
            .create_surface(window.clone())
            .map_err(gui_error)?;
        let size = window.inner_size();
        let config = surface
            .get_default_config(&gpu.adapter, size.width.max(1), size.height.max(1))
            .ok_or_else(|| {
                gui_error("Rendering window is incompatible with the graphics adapter")
            })?;
        surface.configure(&gpu.device, &config);
        let ctx = egui::Context::default();
        let mut fonts = egui::FontDefinitions::default();
        cuemol_ui::shell::configure_fonts(&mut fonts);
        ctx.set_fonts(fonts);
        ctx.set_request_repaint_callback(move |request| {
            if request.delay.is_zero() {
                let _ = proxy.send_event(());
            }
        });
        let input = egui_winit::State::new(
            ctx.clone(),
            egui::ViewportId::ROOT,
            event_loop,
            Some(window.scale_factor() as f32),
            None,
            Some(gpu.device.limits().max_texture_dimension_2d as usize),
        );
        let renderer = egui_wgpu::Renderer::new(&gpu.device, config.format, Default::default());
        Ok(Self {
            window,
            visible: false,
            ctx,
            input,
            surface,
            config,
            renderer,
            repaint_at: None,
        })
    }

    fn resize(&mut self, gpu: &GpuState, size: PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            return;
        }
        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(&gpu.device, &self.config);
    }

    fn record_geometry(&self, state: &mut DesktopAppState) {
        let size = self.window.inner_size();
        if size.width > 0 && size.height > 0 {
            state.put_config_value("render-window:width", size.width.to_string());
            state.put_config_value("render-window:height", size.height.to_string());
        }
        if let Ok(position) = self.window.outer_position() {
            state.put_config_value("render-window:x", position.x.to_string());
            state.put_config_value("render-window:y", position.y.to_string());
        }
    }

    fn draw(
        &mut self,
        gpu: &GpuState,
        app: &mut GuiApplication,
    ) -> std::result::Result<(), wgpu::SurfaceError> {
        let raw = self.input.take_egui_input(&self.window);
        let mut output = self.ctx.run(raw, |ctx| {
            cuemol_ui::shell::theme(ctx, app.light_theme);
            app.native_render_dialog(ctx, &gpu.camera);
            if ctx.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::Q)) {
                app.request_application_exit();
                app.main_focus_requested = true;
            }
            if ctx.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::W)) {
                app.close_native_render(&gpu.camera);
            }
        });
        self.repaint_at = output
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .and_then(|viewport| Instant::now().checked_add(viewport.repaint_delay));
        output.platform_output.commands.retain(|command| {
            if let egui::OutputCommand::CopyText(text) = command {
                if let Err(error) = app.system_clipboard.write(text.clone()) {
                    app.ray_panel.message = format!("Clipboard error: {error}");
                }
                false
            } else {
                true
            }
        });
        self.input
            .handle_platform_output(&self.window, output.platform_output);
        let jobs = self.ctx.tessellate(output.shapes, output.pixels_per_point);
        for (id, image) in &output.textures_delta.set {
            self.renderer
                .update_texture(&gpu.device, &gpu.queue, *id, image);
        }
        let frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(error) => {
                for id in &output.textures_delta.free {
                    self.renderer.free_texture(id);
                }
                return Err(error);
            }
        };
        let view = frame.texture.create_view(&Default::default());
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render-window-egui"),
            });
        let screen = ScreenDescriptor {
            size_in_pixels: [self.config.width, self.config.height],
            pixels_per_point: output.pixels_per_point,
        };
        let mut commands =
            self.renderer
                .update_buffers(&gpu.device, &gpu.queue, &mut encoder, &jobs, &screen);
        let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("render-window-egui"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        self.renderer
            .render(&mut pass.forget_lifetime(), &jobs, &screen);
        commands.push(encoder.finish());
        submit_egui_commands(
            &gpu.queue,
            &mut self.renderer,
            commands,
            &output.textures_delta.free,
        );
        frame.present();
        Ok(())
    }
}

impl NativeRunner {
    pub(super) fn render_window_event(&mut self, event_loop: &ActiveEventLoop, event: WindowEvent) {
        let (Some(render), Some(gpu), Some(app)) =
            (&mut self.render_window, &mut self.gpu, &mut self.app)
        else {
            return;
        };
        if matches!(event, WindowEvent::CloseRequested) {
            render.record_geometry(&mut app.state);
            app.close_native_render(&gpu.camera);
            render.window.set_visible(false);
            render.visible = false;
            if let Some(window) = &self.window {
                window.request_redraw();
            }
            return;
        }
        let modifiers = render.input.egui_input().modifiers;
        let paste = matches!(&event, WindowEvent::KeyboardInput { event, .. }
            if event.state == ElementState::Pressed
                && (event.logical_key == winit::keyboard::Key::Named(winit::keyboard::NamedKey::Paste)
                    || (modifiers.command && !modifiers.alt
                        && (event.logical_key.as_ref() == winit::keyboard::Key::Character("v")
                            || event.physical_key == winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::KeyV)))
                    || (modifiers.shift && event.logical_key == winit::keyboard::Key::Named(winit::keyboard::NamedKey::Insert))));
        let queued = render.input.egui_input().events.len();
        let response = render.input.on_window_event(&render.window, &event);
        if paste && render.ctx.wants_keyboard_input() {
            let mut index = 0;
            render.input.egui_input_mut().events.retain(|event| {
                let keep = index < queued || !matches!(event, egui::Event::Paste(_));
                index += 1;
                keep
            });
            match app.system_clipboard.read() {
                Ok(text) => render
                    .input
                    .egui_input_mut()
                    .events
                    .push(egui::Event::Paste(text)),
                Err(error) => app.ray_panel.message = format!("Clipboard error: {error}"),
            }
        }
        match event {
            WindowEvent::Focused(focused) => app.ray_panel.focused = focused,
            WindowEvent::Resized(size) => {
                render.resize(gpu, size);
                render.record_geometry(&mut app.state);
                render.window.request_redraw();
            }
            WindowEvent::Moved(_) => render.record_geometry(&mut app.state),
            WindowEvent::RedrawRequested if render.visible => {
                let before = (
                    app.snapshot_revision,
                    app.undo_stack.len(),
                    app.redo_stack.len(),
                    app.status.clone(),
                );
                match render.draw(gpu, app) {
                    Ok(()) => {}
                    Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                        render.resize(gpu, render.window.inner_size());
                        render.window.request_redraw();
                    }
                    Err(wgpu::SurfaceError::OutOfMemory) => event_loop.exit(),
                    Err(_) => {}
                }
                let changed = before
                    != (
                        app.snapshot_revision,
                        app.undo_stack.len(),
                        app.redo_stack.len(),
                        app.status.clone(),
                    )
                    || app.pending_script_view
                    || app.main_focus_requested;
                app.apply_script_view(&mut gpu.camera);
                if changed {
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
            }
            _ => {}
        }
        if response.repaint && render.visible {
            render.window.request_redraw();
        }
    }

    pub(super) fn sync_render_window(&mut self, event_loop: &ActiveEventLoop) {
        let (Some(app), Some(gpu)) = (&mut self.app, &mut self.gpu) else {
            return;
        };
        if app.render_visible && self.render_window.is_none() {
            match RenderWindow::new(event_loop, gpu, &app.state, self.event_proxy.clone()) {
                Ok(render) => self.render_window = Some(render),
                Err(error) => {
                    app.close_native_render(&gpu.camera);
                    app.error_alert = Some(format!("Cannot open Rendering window: {error}"));
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
            }
        }
        if std::mem::take(&mut app.main_focus_requested) {
            if let Some(window) = &self.window {
                window.set_minimized(false);
                window.focus_window();
                window.request_redraw();
            }
        }
        let Some(render) = &mut self.render_window else {
            return;
        };
        if render.visible != app.render_visible {
            render.visible = app.render_visible;
            render.window.set_visible(render.visible);
            if render.visible {
                render.window.request_redraw();
            }
        }
        if std::mem::take(&mut app.render_focus_requested) && render.visible {
            render.window.set_minimized(false);
            render.window.focus_window();
            render.window.request_redraw();
        }
        let now = Instant::now();
        if app.ray_job.is_some() || app.ray_movie_job.is_some() {
            if self.render_tick_at.is_none_or(|at| now >= at) {
                app.poll_native_render(&render.ctx);
                self.render_tick_at = Some(now + Duration::from_millis(50));
                if render.visible {
                    render.window.request_redraw();
                }
                if app.ray_job.is_none() && app.ray_movie_job.is_none() {
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
            }
        } else {
            self.render_tick_at = None;
        }
        if render.visible && render.repaint_at.is_some_and(|at| now >= at) {
            render.repaint_at = None;
            render.window.request_redraw();
        }
        let deadline = self
            .render_tick_at
            .into_iter()
            .chain(render.repaint_at.filter(|_| render.visible))
            .min();
        event_loop.set_control_flow(deadline.map_or(ControlFlow::Wait, ControlFlow::WaitUntil));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rendering_visibility_is_independent_of_other_tools_and_close_preserves_the_job() {
        let mut app = GuiApplication::new();
        app.native_render_window = true;
        app.tool_dialog = Some("Molecular superposition".into());
        app.open_native_render();
        assert!(app.render_visible);
        assert_eq!(app.tool_dialog.as_deref(), Some("Molecular superposition"));
        let (_sender, receiver) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        app.ray_job = Some(receiver);
        app.ray_cancel = Some(cancel.clone());
        app.ray_panel.editor.set_size([80, 60], None);
        app.ray_panel.editor.edited(0.);
        app.close_native_render(&CameraController::default());
        assert!(!app.render_visible);
        assert!(app.ray_job.is_some() && !cancel.load(Ordering::Relaxed));
        assert_eq!(app.tool_dialog.as_deref(), Some("Molecular superposition"));
        app.open_native_render();
        assert_eq!(app.ray_panel.editor.pixels(), [80, 60]);
    }
}
