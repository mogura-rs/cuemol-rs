use crate::DesktopAppState;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GuiViewOptions {
    pub trans_mms: bool,
    pub rot_mms: bool,
    pub swap_stereo_eyes: bool,
}

impl DesktopAppState {
    pub fn active_view_options(&self) -> GuiViewOptions {
        self.active_view_summary()
            .and_then(|view| self.command_manager().view(view.uid))
            .map(|view| GuiViewOptions {
                trans_mms: view.trans_mms(),
                rot_mms: view.rot_mms(),
                swap_stereo_eyes: view.swap_stereo_eyes(),
            })
            .unwrap_or_default()
    }

    pub fn set_active_view_options(&mut self, options: GuiViewOptions) {
        let id = self.active_view_summary().map(|view| view.uid);
        if let Some(view) = id.and_then(|id| self.command_manager_mut().view_mut(id)) {
            if view.trans_mms() != options.trans_mms {
                view.set_trans_mms(options.trans_mms);
            }
            if view.rot_mms() != options.rot_mms {
                view.set_rot_mms(options.rot_mms);
            }
            if view.swap_stereo_eyes() != options.swap_stereo_eyes {
                view.set_swap_stereo_eyes(options.swap_stereo_eyes);
            }
        }
    }
}
