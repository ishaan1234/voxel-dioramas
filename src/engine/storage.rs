//! Tiny persistent key/value store: files on desktop, localStorage on the web.
//! Values are strings; use `get_json` / `set_json` for structured data.

pub struct Storage {
    #[cfg(not(target_arch = "wasm32"))]
    dir: std::path::PathBuf,
}

impl Default for Storage {
    fn default() -> Self {
        Self::new()
    }
}

impl Storage {
    pub fn new() -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let base = std::env::var_os("APPDATA")
                .or_else(|| std::env::var_os("XDG_DATA_HOME"))
                .map(std::path::PathBuf::from)
                .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local/share")))
                .unwrap_or_else(|| std::path::PathBuf::from("."));
            let dir = base.join("voxel-dioramas");
            let _ = std::fs::create_dir_all(&dir);
            Self { dir }
        }
        #[cfg(target_arch = "wasm32")]
        Self {}
    }

    fn sanitize(key: &str) -> String {
        key.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect()
    }

    pub fn get(&self, key: &str) -> Option<String> {
        let key = Self::sanitize(key);
        #[cfg(not(target_arch = "wasm32"))]
        {
            std::fs::read_to_string(self.dir.join(format!("{key}.json"))).ok()
        }
        #[cfg(target_arch = "wasm32")]
        {
            let ls = web_sys::window()?.local_storage().ok()??;
            ls.get_item(&format!("voxel-dioramas:{key}")).ok()?
        }
    }

    pub fn set(&mut self, key: &str, value: &str) {
        let key = Self::sanitize(key);
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = std::fs::write(self.dir.join(format!("{key}.json")), value);
        }
        #[cfg(target_arch = "wasm32")]
        if let Some(Ok(Some(ls))) = web_sys::window().map(|w| w.local_storage()) {
            let _ = ls.set_item(&format!("voxel-dioramas:{key}"), value);
        }
    }

    pub fn get_json<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        serde_json::from_str(&self.get(key)?).ok()
    }

    pub fn set_json<T: serde::Serialize>(&mut self, key: &str, value: &T) {
        if let Ok(s) = serde_json::to_string(value) {
            self.set(key, &s);
        }
    }

    /// Where desktop builds put exported files (postcards, clips).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn export_dir(&self) -> std::path::PathBuf {
        let d = self.dir.join("exports");
        let _ = std::fs::create_dir_all(&d);
        d
    }
}
