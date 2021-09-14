use serde::Serialize;
use serde::de::DeserializeOwned;
use anyhow::{Context, Result, anyhow};

pub struct UndoStack {
    records: Vec<(Vec<u8>, String)>
}

impl UndoStack {
    pub fn new()->UndoStack {
        UndoStack {
            records: Vec::new(),
        }
    }
    pub fn push<O: Serialize>(&mut self, instance: &O, text: &str)->Result<()> {
        let bytes = bincode::serialize(instance)?;
        self.records.push((bytes, text.to_owned()));
        Ok(())
    }

    pub fn apply<O: Serialize + DeserializeOwned>(&mut self, instance: &mut O, redo: &mut Self)->Result<()> {
        let (bytes, text) = self.records.pop().ok_or_else(|| anyhow!("Empty undo stack"))?;
        let redo_bytes = bincode::serialize(&instance).context("Serializing redo record")?;
        redo.records.push((redo_bytes, text));
        *instance = bincode::deserialize(&bytes).context("Deserializing undo record")?;
        Ok(())
    }

    pub fn is_empty(&self)->bool {
        self.records.is_empty()
    }

    pub fn clear(&mut self) {
        self.records.clear();
    }

}