use boa_engine::JsObject;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};

#[derive(Clone, Debug, Default)]
pub(crate) struct Identity(Arc<Token>);

#[derive(Debug, Default)]
struct Token {
    resolved: AtomicBool,
}

pub(crate) type Shared = Rc<RefCell<Services>>;

thread_local! {
    static SERVICES: RefCell<Vec<(Weak<Token>, Shared)>> = {
        // Drop retained JavaScript callbacks before the thread's collector.
        boa_engine::gc::force_collect();
        RefCell::new(Vec::new())
    };
}

impl Identity {
    pub(crate) fn resolve(&self) -> Shared {
        self.0.resolved.store(true, Ordering::Relaxed);
        SERVICES.with(|services| {
            let mut services = services.borrow_mut();
            services.retain(|(owner, _)| owner.strong_count() != 0);
            let owner = Arc::downgrade(&self.0);
            if let Some((_, existing)) = services.iter().find(|(key, _)| key.ptr_eq(&owner)) {
                return existing.clone();
            }
            let value = Rc::new(RefCell::new(Services::default()));
            services.push((owner, value.clone()));
            value
        })
    }
}

impl Drop for Identity {
    fn drop(&mut self) {
        if Arc::strong_count(&self.0) != 1 || !self.0.resolved.load(Ordering::Relaxed) {
            return;
        }
        let owner = Arc::downgrade(&self.0);
        let _ = SERVICES.try_with(|services| {
            if let Ok(mut services) = services.try_borrow_mut() {
                services.retain(|(key, _)| !key.ptr_eq(&owner));
            }
        });
    }
}

pub(crate) struct Services {
    pub(crate) msg_log: cuemol_core::LMsgLog,
    pub(crate) process_manager: cuemol_core::ProcessManager,
    pub(crate) python_bridge: Rc<RefCell<cuemol_py::PythonBridge>>,
    pub(crate) python_objects: super::script_python::Objects,
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) python_connection: Option<cuemol_py::PythonConnection>,
    pub(crate) event_manager: cuemol_qsys::ScrEventManager,
    pub(crate) mol_client_manager: cuemol_molclient::MolClientManager,
    pub(crate) scene_listeners: BTreeMap<i32, (u64, JsObject)>,
    pub(crate) next_scene_listener_id: i32,
    pub(crate) event_listener: Option<JsObject>,
}

impl Default for Services {
    fn default() -> Self {
        Self {
            msg_log: cuemol_core::LMsgLog::new(),
            process_manager: cuemol_core::ProcessManager::new(),
            python_bridge: Rc::new(RefCell::new(cuemol_py::PythonBridge::with_uv_execution())),
            python_objects: super::script_python::Objects::default(),
            #[cfg(not(target_arch = "wasm32"))]
            python_connection: None,
            event_manager: cuemol_qsys::ScrEventManager::new(),
            mol_client_manager: cuemol_molclient::MolClientManager::new(),
            scene_listeners: BTreeMap::new(),
            next_scene_listener_id: 0,
            event_listener: None,
        }
    }
}

impl Drop for Services {
    fn drop(&mut self) {
        self.process_manager.kill_all();
    }
}
