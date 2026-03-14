//! Shared I/O helpers for the agent framework.

/// Load a JSONL file into a Vec of deserialized records.
/// Returns an empty Vec on any read or parse error.
pub fn load_jsonl<T: serde::de::DeserializeOwned>(path: &std::path::Path) -> Vec<T> {
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(_) => return Vec::new(),
    };
    content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str::<T>(line).ok())
        .collect()
}
