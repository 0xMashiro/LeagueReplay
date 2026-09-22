use serde_json::{json, Value};
use std::{fs::OpenOptions, io::Write, path::PathBuf};

pub(super) struct Diagnostics {
    path: PathBuf,
    last_sample: Option<Value>,
}

impl Diagnostics {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            last_sample: None,
        }
    }

    pub fn sample(&mut self, value: Value) {
        if self.last_sample.as_ref() != Some(&value) {
            self.write("sample", value.clone());
            self.last_sample = Some(value);
        }
    }

    pub fn write(&self, event: &str, data: Value) {
        // Best effort: diagnostic IO must never prevent recording a game.
        if let Err(error) = self.append(event, data) {
            eprintln!("Cannot write collection diagnostics: {error}");
        }
    }

    fn append(&self, event: &str, data: Value) -> std::io::Result<()> {
        if self.path.metadata().is_ok_and(|m| m.len() >= 1_048_576) {
            let previous = self.path.with_extension("previous.jsonl");
            if previous.exists() {
                std::fs::remove_file(&previous)?;
            }
            std::fs::rename(&self.path, previous)?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        writeln!(
            file,
            "{}",
            json!({"at":crate::domain::now(),"event":event,"data":data})
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unchanged_samples_are_quiet_and_rotation_keeps_one_previous_file() {
        let dir = std::env::temp_dir().join(format!("collection-log-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("collection.jsonl");
        let mut log = Diagnostics::new(path.clone());
        for _ in 0..10 {
            log.sample(json!({"state":"offline"}));
        }
        assert_eq!(std::fs::read_to_string(&path).unwrap().lines().count(), 1);
        OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_len(1_048_576)
            .unwrap();
        log.write("fetch", json!({"game":"HN1_1","result":"client.NotFound"}));
        assert_eq!(
            path.with_extension("previous.jsonl")
                .metadata()
                .unwrap()
                .len(),
            1_048_576
        );
        let line: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(line["event"], "fetch");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
