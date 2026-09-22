use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, ts_rs::TS)]
#[ts(export, export_to = "../../src/generated/resources/")]
pub struct ResourceData {
    pub catalog: Catalog,
    pub names: BTreeMap<String, LocalizedNames>,
}

#[derive(Serialize, Deserialize, ts_rs::TS)]
#[ts(export_to = "../../src/generated/resources/")]
pub struct Catalog {
    pub version: String,
    pub champions: BTreeMap<String, Champion>,
    pub items: BTreeMap<String, Item>,
}

#[derive(Serialize, Deserialize, ts_rs::TS)]
#[ts(export_to = "../../src/generated/resources/")]
pub struct Champion {
    pub key: u32,
    pub name: String,
    pub title: String,
    pub bundled: bool,
}

#[derive(Serialize, Deserialize, ts_rs::TS)]
#[ts(export_to = "../../src/generated/resources/")]
pub struct Item {
    pub name: String,
    pub gold: u32,
    pub finished: bool,
    pub bundled: bool,
}

#[derive(Serialize, Deserialize, ts_rs::TS)]
#[ts(export_to = "../../src/generated/resources/")]
pub struct LocalizedNames {
    pub champions: BTreeMap<String, String>,
    pub items: BTreeMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_catalog_satisfies_the_wire_contract_and_invalid_keys_are_rejected() {
        let catalog: serde_json::Value =
            serde_json::from_str(include_str!("../../../src/data/catalog.json")).unwrap();
        let mut value = serde_json::json!({"catalog":catalog,"names":{}});
        let resources: ResourceData = serde_json::from_value(value.clone()).unwrap();
        assert!(!resources.catalog.champions.is_empty());
        assert!(!resources.catalog.items.is_empty());
        let champion = value["catalog"]["champions"]
            .as_object_mut()
            .unwrap()
            .values_mut()
            .next()
            .unwrap();
        champion["key"] = serde_json::Value::Null;
        assert!(serde_json::from_value::<ResourceData>(value).is_err());
    }
}
