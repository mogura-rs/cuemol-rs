use crate::Provider;
use zeroize::Zeroizing;

fn entry(provider: Provider) -> Result<keyring::Entry, String> {
    keyring::Entry::new("org.cuemol.agent", provider.name()).map_err(|e| e.to_string())
}
pub fn load(provider: Provider) -> Result<Zeroizing<String>, String> {
    match entry(provider).and_then(|entry|entry.get_password().map_err(|e|e.to_string())) {
        Ok(key) => Ok(Zeroizing::new(key)),
        Err(error) => std::env::var(provider.env_var()).ok().filter(|s| !s.trim().is_empty()).map(Zeroizing::new).ok_or_else(|| format!("No {} API key is available. Set it in AI Agent or use {}. Credential store: {error}", provider.name(), provider.env_var())),
    }
}
pub fn save(provider: Provider, key: &str) -> Result<(), String> {
    if key.trim().is_empty() {
        return Err("API key is empty".into());
    }
    entry(provider)?
        .set_password(key)
        .map_err(|e| e.to_string())
}
pub fn delete(provider: Provider) -> Result<(), String> {
    entry(provider)?
        .delete_credential()
        .map_err(|e| e.to_string())
}
