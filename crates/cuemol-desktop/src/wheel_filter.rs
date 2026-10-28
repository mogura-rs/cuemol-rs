use winit::event::{DeviceEvent, DeviceId, ElementState, MouseScrollDelta, WindowEvent};

#[derive(Default)]
pub(super) struct X11WheelFilter {
    pub enabled: bool,
    pending: Option<(DeviceId, MouseScrollDelta, ElementState)>,
}

impl X11WheelFilter {
    pub fn clear(&mut self) {
        self.pending = None;
    }

    pub fn device_event(&mut self, device: DeviceId, event: &DeviceEvent) {
        if !self.enabled {
            return;
        }
        self.pending = match event {
            DeviceEvent::Button { button, state } => {
                let delta = match button {
                    4 => MouseScrollDelta::LineDelta(0.0, 1.0),
                    5 => MouseScrollDelta::LineDelta(0.0, -1.0),
                    6 => MouseScrollDelta::LineDelta(1.0, 0.0),
                    7 => MouseScrollDelta::LineDelta(-1.0, 0.0),
                    _ => return self.clear(),
                };
                Some((device, delta, *state))
            }
            _ => None,
        };
    }

    pub fn suppress(&mut self, event: &WindowEvent) -> bool {
        if let WindowEvent::MouseWheel {
            device_id, delta, ..
        } = event
        {
            // winit 0.30.13 loses the button state when converting X11 wheel events.
            // Raw button events precede the corresponding window event.
            self.enabled
                && self
                    .pending
                    .take()
                    .is_some_and(|(device, expected, state)| {
                        device == *device_id
                            && expected == *delta
                            && state == ElementState::Released
                    })
        } else {
            if matches!(
                event,
                WindowEvent::Focused(false) | WindowEvent::CursorLeft { .. }
            ) {
                self.clear();
            }
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::event::TouchPhase;

    #[test]
    fn x11_button_scroll_keeps_presses_and_valuator_motion() {
        let device = DeviceId::dummy();
        let wheel = |delta| WindowEvent::MouseWheel {
            device_id: device,
            delta,
            phase: TouchPhase::Moved,
        };
        let mut filter = X11WheelFilter {
            enabled: true,
            ..Default::default()
        };
        for (button, delta) in [
            (4, MouseScrollDelta::LineDelta(0.0, 1.0)),
            (5, MouseScrollDelta::LineDelta(0.0, -1.0)),
            (6, MouseScrollDelta::LineDelta(1.0, 0.0)),
            (7, MouseScrollDelta::LineDelta(-1.0, 0.0)),
        ] {
            for _ in 0..3 {
                filter.device_event(
                    device,
                    &DeviceEvent::Button {
                        button,
                        state: ElementState::Pressed,
                    },
                );
                assert!(!filter.suppress(&wheel(delta)));
                filter.device_event(
                    device,
                    &DeviceEvent::Button {
                        button,
                        state: ElementState::Released,
                    },
                );
                assert!(filter.suppress(&wheel(delta)));
                assert!(!filter.suppress(&wheel(delta)));
            }
        }
        filter.device_event(
            device,
            &DeviceEvent::Button {
                button: 4,
                state: ElementState::Released,
            },
        );
        filter.device_event(
            device,
            &DeviceEvent::MouseWheel {
                delta: MouseScrollDelta::LineDelta(0.0, 1.0),
            },
        );
        assert!(!filter.suppress(&wheel(MouseScrollDelta::LineDelta(0.0, 1.0))));
        filter.device_event(
            device,
            &DeviceEvent::Button {
                button: 4,
                state: ElementState::Released,
            },
        );
        filter.clear();
        assert!(!filter.suppress(&wheel(MouseScrollDelta::LineDelta(0.0, 1.0))));
        filter.enabled = false;
        filter.device_event(
            device,
            &DeviceEvent::Button {
                button: 4,
                state: ElementState::Released,
            },
        );
        assert!(!filter.suppress(&wheel(MouseScrollDelta::LineDelta(0.0, 1.0))));
    }
}
