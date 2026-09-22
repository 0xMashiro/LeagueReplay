/// Application failures cross IPC as codes, never as localized prose or raw diagnostics.
#[derive(Debug, serde::Serialize, ts_rs::TS)]
#[ts(export, export_to = "../../src/generated/ipc/")]
pub struct CommandError {
    pub code: String,
}

impl From<String> for CommandError {
    fn from(code: String) -> Self {
        Self { code }
    }
}

impl From<&str> for CommandError {
    fn from(code: &str) -> Self {
        Self { code: code.into() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn critical_errors_serialize_as_codes() {
        for code in [
            "ui.conflict",
            "client.Unavailable",
            "backup.changed",
            "backup.restoreFailed",
        ] {
            assert_eq!(
                serde_json::to_value(CommandError::from(code)).unwrap(),
                serde_json::json!({"code":code})
            );
        }
    }
}
