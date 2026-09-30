use crate::memory::store::Entry;
use serde::{Deserialize, Deserializer, Serializer};
use std::collections::BTreeMap;

pub fn serialize<S>(map: &BTreeMap<Vec<u8>, Entry>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    use serde::ser::SerializeMap;
    let mut serde_map = serializer.serialize_map(Some(map.len()))?;
    for (k, v) in map {
        let hex_key = hex_encode(k);
        serde_map.serialize_entry(&hex_key, v)?;
    }
    serde_map.end()
}

pub fn deserialize<'de, D>(deserializer: D) -> Result<BTreeMap<Vec<u8>, Entry>, D::Error>
where
    D: Deserializer<'de>,
{
    let string_map = BTreeMap::<String, Entry>::deserialize(deserializer)?;
    let mut byte_map = BTreeMap::new();
    for (k, v) in string_map {
        let bytes = hex_decode(&k).map_err(serde::de::Error::custom)?;
        byte_map.insert(bytes, v);
    }
    Ok(byte_map)
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn hex_decode(s: &str) -> Result<Vec<u8>, String> {
    if s.len() % 2 != 0 {
        return Err("Invalid hex string length".into());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| e.to_string()))
        .collect()
}
