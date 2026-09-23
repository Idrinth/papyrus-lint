//! Recursive YAML layering used to combine a preset with an
//! executable-adjacent base configuration.

/// Deep-merges `over` onto `base`: a mapping present in both merges key by
/// key recursively, while any other value in `over` replaces `base`'s value
/// entirely.
pub(crate) fn deep_merge(
    base: serde_norway::Value,
    over: serde_norway::Value,
) -> serde_norway::Value {
    match (base, over) {
        (serde_norway::Value::Mapping(mut base_map), serde_norway::Value::Mapping(over_map)) => {
            for (key, value) in over_map {
                let merged = match base_map.remove(&key) {
                    Some(base_value) => deep_merge(base_value, value),
                    None => value,
                };
                base_map.insert(key, merged);
            }
            serde_norway::Value::Mapping(base_map)
        }
        (_, over) => over,
    }
}

#[cfg(test)]
#[path = "yaml_merge_tests.rs"]
mod tests;
