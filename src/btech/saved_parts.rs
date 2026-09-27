//! Split a unit's saved record into a rarely changing core and frequently changing live
//! state.
//!
//! A unit is saved as two JSON objects. The core holds construction, damage and settings,
//! which change rarely; the live state holds motion, heat, timers and the like, which the
//! heartbeat changes routinely. Each part is rewritten only when it changed, so a moving
//! unit writes a small live object instead of its whole record. Both parts use the unit's
//! ordinary serde field names, so merging them rebuilds the full record for loading.

/// A record saved as a rarely changing core and a frequently changing live part.
pub(crate) trait SavedParts {
    /// Live fields left out while at their default; each must load back as that default.
    #[cfg(test)]
    const DEFAULTED_LIVE_FIELDS: &'static [&'static str];
    /// Whether the rarely changing part matches `other`.
    fn same_saved_core(&self, other: &Self) -> bool;
    /// Whether the frequently changing part matches `other`.
    fn same_saved_live(&self, other: &Self) -> bool;
    /// The rarely changing part as a JSON object keyed by the record's own field names.
    fn saved_core_value(&self) -> serde_json::Result<serde_json::Value>;
    /// The frequently changing part as a JSON object keyed by the record's own field
    /// names, leaving out fields at their default value, which load back as that default.
    fn saved_live_value(&self) -> serde_json::Result<serde_json::Value>;
}

/// Serializes the rarely changing part of a record.
pub(crate) struct SavedCore<'a, T>(pub &'a T);

/// Serializes the frequently changing part of a record.
pub(crate) struct SavedLive<'a, T>(pub &'a T);

/// Rebuild one record from its saved parts. The parts never share a field, so an overlap
/// means the row is corrupt and is refused.
pub(crate) fn merge(core: &str, live: &str) -> serde_json::Result<serde_json::Value> {
    let mut record: serde_json::Map<String, serde_json::Value> = serde_json::from_str(core)?;
    let live: serde_json::Map<String, serde_json::Value> = serde_json::from_str(live)?;
    for (field, value) in live {
        if record.insert(field.clone(), value).is_some() {
            return Err(serde::de::Error::custom(format!(
                "unit field {field} is saved in both the core and live parts"
            )));
        }
    }
    Ok(serde_json::Value::Object(record))
}

/// Whether a field holds its default value and can be left out of the live part.
pub(crate) fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    *value == T::default()
}

/// Implement [`SavedParts`] for a struct from one classification of its fields.
///
/// Every field is listed exactly once: as `core`, as `live` (left out of the saved live
/// part while at its default, so its type needs `Default` and the field needs
/// `#[serde(default)]`), or as `live_always` (always written). The comparisons
/// destructure the whole struct, so adding a field fails to compile until it is
/// classified.
macro_rules! saved_parts {
    ($ty:ty {
        core: [$($core:ident),* $(,)?],
        live: [$($live:ident),* $(,)?],
        live_always: [$($always:ident),* $(,)?] $(,)?
    }) => {
        impl $crate::btech::saved_parts::SavedParts for $ty {
            #[cfg(test)]
            const DEFAULTED_LIVE_FIELDS: &'static [&'static str] = &[$(stringify!($live)),*];

            fn same_saved_core(&self, other: &Self) -> bool {
                let Self { $($core,)* $($live: _,)* $($always: _,)* } = self;
                true $(&& *$core == other.$core)*
            }

            fn same_saved_live(&self, other: &Self) -> bool {
                let Self { $($core: _,)* $($live,)* $($always,)* } = self;
                true $(&& *$live == other.$live)* $(&& *$always == other.$always)*
            }

            fn saved_core_value(&self) -> serde_json::Result<serde_json::Value> {
                serde_json::to_value(&$crate::btech::saved_parts::SavedCore(self))
            }

            fn saved_live_value(&self) -> serde_json::Result<serde_json::Value> {
                serde_json::to_value(&$crate::btech::saved_parts::SavedLive(self))
            }
        }

        impl serde::Serialize for $crate::btech::saved_parts::SavedCore<'_, $ty> {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                use serde::ser::SerializeMap;
                let mut map = serializer.serialize_map(None)?;
                $(map.serialize_entry(stringify!($core), &self.0.$core)?;)*
                map.end()
            }
        }

        impl serde::Serialize for $crate::btech::saved_parts::SavedLive<'_, $ty> {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                use serde::ser::SerializeMap;
                let mut map = serializer.serialize_map(None)?;
                $(
                    if !$crate::btech::saved_parts::is_default(&self.0.$live) {
                        map.serialize_entry(stringify!($live), &self.0.$live)?;
                    }
                )*
                $(map.serialize_entry(stringify!($always), &self.0.$always)?;)*
                map.end()
            }
        }
    };
}
pub(crate) use saved_parts;

/// Check that every defaulted live field of `record` loads back as its default when its
/// key is absent, which is what leaving it out of the saved live part relies on.
#[cfg(test)]
pub(crate) fn assert_defaulted_fields_load<T>(record: &T)
where
    T: SavedParts + serde::Serialize + serde::de::DeserializeOwned,
{
    let full = serde_json::to_value(record).unwrap();
    for field in T::DEFAULTED_LIVE_FIELDS {
        let mut value = full.clone();
        value.as_object_mut().unwrap().remove(*field);
        let loaded: T = serde_json::from_value(value)
            .unwrap_or_else(|error| panic!("{field} has no serde default: {error}"));
        let live: serde_json::Value = loaded.saved_live_value().unwrap();
        assert!(
            live.get(*field).is_none(),
            "{field} does not load as its default"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct Sample {
        name: String,
        armor: Vec<u8>,
        #[serde(default)]
        heat: u8,
        position: (i16, i16),
    }

    saved_parts!(Sample {
        core: [name, armor],
        live: [heat],
        live_always: [position],
    });

    /// Each part tracks only its own fields, and merging both parts rebuilds the record.
    #[test]
    fn parts_split_compare_and_merge_back() {
        let sample = Sample {
            name: "Atlas".into(),
            armor: vec![9, 9],
            heat: 3,
            position: (4, 5),
        };
        let mut moved = sample.clone();
        moved.heat = 7;
        moved.position = (5, 5);
        assert!(sample.same_saved_core(&moved));
        assert!(!sample.same_saved_live(&moved));
        let mut hit = sample.clone();
        hit.armor[0] = 2;
        assert!(!sample.same_saved_core(&hit));
        assert!(sample.same_saved_live(&hit));

        let core = moved.saved_core_value().unwrap().to_string();
        let live = moved.saved_live_value().unwrap().to_string();
        assert!(!core.contains("heat") && !live.contains("armor"));
        let merged: Sample = serde_json::from_value(merge(&core, &live).unwrap()).unwrap();
        assert_eq!(merged, moved);

        // A live field at its default is left out and loads back as that default.
        let mut cooled = moved.clone();
        cooled.heat = 0;
        let live = cooled.saved_live_value().unwrap().to_string();
        assert!(!live.contains("heat"));
        let merged: Sample = serde_json::from_value(merge(&core, &live).unwrap()).unwrap();
        assert_eq!(merged, cooled);
        assert_defaulted_fields_load(&cooled);

        // The parts never share a field; a row that does is corrupt.
        let whole = serde_json::to_string(&sample).unwrap();
        assert!(merge(&whole, &live).is_err());
    }
}
