use anyhow::Result;

use super::types::RuntimeState;

impl RuntimeState {
    pub fn to_json(&self) -> Result<String> {
        let json = serde_json::to_string_pretty(self)?;
        Ok(json)
    }

    pub fn from_json(json: &str) -> Result<Self> {
        let state: Self = serde_json::from_str(json)?;
        Ok(state)
    }

    pub fn write_status_file(&self, path: &std::path::Path) -> Result<()> {
        let json = self.to_json()?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn read_status_file(path: &std::path::Path) -> Result<Self> {
        let json = std::fs::read_to_string(path)?;
        Self::from_json(&json)
    }
}
