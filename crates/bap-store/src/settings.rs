//! Preferences: a flat `key = value` file. TODO: the real keys.

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Settings {}

impl Settings {
    pub fn load() -> Settings {
        Settings::default()
    }
}
