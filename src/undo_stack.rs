use anyhow::{anyhow, Context, Result};
use serde::de::DeserializeOwned;
use serde::Serialize;

pub struct UndoStack {
    records: Vec<(Vec<u8>, String)>,
}

impl UndoStack {
    pub fn new() -> UndoStack {
        UndoStack {
            records: Vec::new(),
        }
    }
    pub fn push(&mut self, instance: &impl Serialize, text: &str) -> Result<()> {
        let bytes = serde_json::to_vec(instance)?;
        self.records.push((bytes, text.to_owned()));
        Ok(())
    }

    pub fn apply(
        &mut self,
        instance: &mut (impl Serialize + DeserializeOwned),
        redo: &mut Self,
    ) -> Result<()> {
        let (bytes, text) = self
            .records
            .pop()
            .ok_or_else(|| anyhow!("Empty undo stack"))?;
        let redo_bytes = serde_json::to_vec(&instance).context("Serializing redo record")?;
        redo.records.push((redo_bytes, text));
        *instance = serde_json::from_slice(&bytes).context("Deserializing undo record")?;
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn clear(&mut self) {
        self.records.clear();
    }
}
