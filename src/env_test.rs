use std::sync::{Mutex, MutexGuard, OnceLock};

pub(crate) fn env_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

pub(crate) struct EnvVarGuard {
    name: &'static str,
    previous: Option<std::ffi::OsString>,
    _lock: MutexGuard<'static, ()>,
}

impl EnvVarGuard {
    pub(crate) fn capture(name: &'static str) -> Self {
        let lock = env_lock();
        Self {
            name,
            previous: std::env::var_os(name),
            _lock: lock,
        }
    }

    pub(crate) fn set(&self, value: &str) {
        unsafe {
            std::env::set_var(self.name, value);
        }
    }

    pub(crate) fn unset(&self) {
        unsafe {
            std::env::remove_var(self.name);
        }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        match self.previous.take() {
            Some(value) => unsafe { std::env::set_var(self.name, value) },
            None => unsafe { std::env::remove_var(self.name) },
        }
    }
}
