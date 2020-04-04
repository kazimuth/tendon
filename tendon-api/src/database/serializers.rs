use crate::Map;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::hash::Hash;

pub fn serialize_map_once_cell<
    K: Clone + Serialize + PartialEq + Eq + Hash,
    V: Serialize,
    S: Serializer,
>(
    map: &Map<K, OnceCell<V>>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let mapped: Map<K, Option<&V>> = map.iter().map(|(k, c)| (k.clone(), c.get())).collect();

    mapped.serialize(serializer)
}

pub fn deserialize_map_once_cell<
    'de,
    K: Deserialize<'de> + PartialEq + Eq + Hash,
    V: Deserialize<'de>,
    D: Deserializer<'de>,
>(
    deserializer: D,
) -> Result<Map<K, OnceCell<V>>, D::Error> {
    let mapped = Map::<K, Option<V>>::deserialize(deserializer)?;
    let mut result = Map::default();
    for (k, v) in mapped {
        let cell = OnceCell::new();
        if let Some(v) = v {
            cell.set(v);
        }
        result.insert(k, cell);
    }
    Ok(result)
}
