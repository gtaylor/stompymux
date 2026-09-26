//! Copy-on-write ordered map that keeps world snapshots cheap.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::borrow::Borrow;
use std::collections::BTreeMap;
use std::fmt;
use std::ops::Index;
use std::sync::Arc;

/// Ordered map whose clone shares every entry with the original.
///
/// World transactions snapshot the whole world before a change and restore it on
/// error, so cloning must be cheap. Cloning a `SharedMap` bumps one reference count.
/// The first write after a clone copies the map's index (one pointer per entry) and
/// the one entry being changed; every other entry stays shared with the snapshot.
///
/// The API mirrors [`BTreeMap`]. Removal and replacement hand back the shared entry
/// instead of an owned value, so discarding it never copies the entry.
pub struct SharedMap<K, V>(Arc<BTreeMap<K, Arc<V>>>);

impl<K, V> SharedMap<K, V> {
    /// An empty map.
    pub fn new() -> Self {
        Self(Arc::new(BTreeMap::new()))
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// True when the map has no entries.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Entries in key order.
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = (&K, &V)> + ExactSizeIterator + Clone {
        self.0.iter().map(|(key, value)| (key, &**value))
    }

    /// Keys in order.
    pub fn keys(&self) -> impl DoubleEndedIterator<Item = &K> + ExactSizeIterator + Clone {
        self.0.keys()
    }

    /// Values in key order.
    pub fn values(&self) -> impl DoubleEndedIterator<Item = &V> + ExactSizeIterator + Clone {
        self.0.values().map(|value| &**value)
    }
}

impl<K: Ord, V> SharedMap<K, V> {
    /// Look up one entry.
    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.0.get(key).map(|value| &**value)
    }

    /// Entries whose keys fall within `range`, in key order.
    pub fn range<Q, R>(&self, range: R) -> impl DoubleEndedIterator<Item = (&K, &V)>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
        R: std::ops::RangeBounds<Q>,
    {
        self.0.range(range).map(|(key, value)| (key, &**value))
    }

    /// The entry with the smallest key.
    pub fn first_key_value(&self) -> Option<(&K, &V)> {
        self.0.first_key_value().map(|(key, value)| (key, &**value))
    }

    /// The entry with the largest key.
    pub fn last_key_value(&self) -> Option<(&K, &V)> {
        self.0.last_key_value().map(|(key, value)| (key, &**value))
    }

    /// True when both maps hold the very same allocation for `key`, which proves the
    /// entry is unchanged without comparing it. An absent key is never shared.
    pub fn shares_entry<Q>(&self, other: &Self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        match (self.0.get(key), other.0.get(key)) {
            (Some(mine), Some(theirs)) => Arc::ptr_eq(mine, theirs),
            _ => false,
        }
    }

    /// True when both maps are the same allocation, so no entry can differ.
    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// True when the key is present.
    pub fn contains_key<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.0.contains_key(key)
    }
}

impl<K: Ord + Clone, V: Clone> SharedMap<K, V> {
    /// Mutable access to one entry, copying it first if a snapshot still shares it.
    pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        if !self.0.contains_key(key) {
            return None;
        }
        Arc::make_mut(&mut self.0).get_mut(key).map(Arc::make_mut)
    }

    /// Mutable access to one entry, inserting `V::default()` first when it is absent.
    pub fn get_or_default(&mut self, key: K) -> &mut V
    where
        V: Default,
    {
        Arc::make_mut(Arc::make_mut(&mut self.0).entry(key).or_default())
    }

    /// Insert `source`'s entry for `key`, sharing it instead of copying it. Does nothing
    /// when `source` has no such entry.
    pub fn share_entry_from(&mut self, source: &Self, key: &K) {
        if let Some(value) = source.0.get(key) {
            Arc::make_mut(&mut self.0).insert(key.clone(), Arc::clone(value));
        }
    }

    /// Insert or replace an entry, returning the previous one.
    pub fn insert(&mut self, key: K, value: V) -> Option<Arc<V>> {
        Arc::make_mut(&mut self.0).insert(key, Arc::new(value))
    }

    /// Remove an entry, returning it.
    pub fn remove<Q>(&mut self, key: &Q) -> Option<Arc<V>>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        if !self.0.contains_key(key) {
            return None;
        }
        Arc::make_mut(&mut self.0).remove(key)
    }

    /// Remove every entry.
    pub fn clear(&mut self) {
        self.0 = Arc::new(BTreeMap::new());
    }

    /// Keep only the entries `keep` accepts. The map stays shared when nothing is removed.
    pub fn retain(&mut self, mut keep: impl FnMut(&K, &V) -> bool) {
        let doomed: Vec<K> = self
            .0
            .iter()
            .filter(|(key, value)| !keep(key, value))
            .map(|(key, _)| key.clone())
            .collect();
        if doomed.is_empty() {
            return;
        }
        let map = Arc::make_mut(&mut self.0);
        for key in doomed {
            map.remove(&key);
        }
    }

    /// Keep only the entries `keep` accepts, letting it edit the ones it keeps. This
    /// copies every entry a snapshot still shares.
    pub fn retain_mut(&mut self, mut keep: impl FnMut(&K, &mut V) -> bool) {
        Arc::make_mut(&mut self.0).retain(|key, value| keep(key, Arc::make_mut(value)));
    }

    /// Mutable access to every value. This copies every entry a snapshot still shares,
    /// so prefer [`SharedMap::get_mut`] on the entries that actually change.
    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut V> {
        Arc::make_mut(&mut self.0).values_mut().map(Arc::make_mut)
    }

    /// Mutable access to every entry, with the same copying cost as
    /// [`SharedMap::values_mut`].
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&K, &mut V)> {
        Arc::make_mut(&mut self.0)
            .iter_mut()
            .map(|(key, value)| (key, Arc::make_mut(value)))
    }
}

impl<K, V> Clone for SharedMap<K, V> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl<K, V> Default for SharedMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: fmt::Debug, V: fmt::Debug> fmt::Debug for SharedMap<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

/// Shared entries compare equal by pointer before any value comparison, so comparing a
/// map with an edited copy of itself only inspects the entries that changed.
impl<K: PartialEq, V: PartialEq> PartialEq for SharedMap<K, V> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
            || (self.0.len() == other.0.len()
                && self.0.iter().zip(other.0.iter()).all(
                    |((mine_key, mine), (their_key, theirs))| {
                        mine_key == their_key && (Arc::ptr_eq(mine, theirs) || mine == theirs)
                    },
                ))
    }
}

impl<K: Eq, V: Eq> Eq for SharedMap<K, V> {}

impl<K: Ord, V> From<BTreeMap<K, V>> for SharedMap<K, V> {
    fn from(map: BTreeMap<K, V>) -> Self {
        map.into_iter().collect()
    }
}

impl<K: Ord, V> FromIterator<(K, V)> for SharedMap<K, V> {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(entries: I) -> Self {
        Self(Arc::new(
            entries
                .into_iter()
                .map(|(key, value)| (key, Arc::new(value)))
                .collect(),
        ))
    }
}

impl<'a, K, V> IntoIterator for &'a SharedMap<K, V> {
    type Item = (&'a K, &'a V);
    type IntoIter = std::iter::Map<
        std::collections::btree_map::Iter<'a, K, Arc<V>>,
        fn((&'a K, &'a Arc<V>)) -> (&'a K, &'a V),
    >;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter().map(|(key, value)| (key, &**value))
    }
}

impl<'a, K: Clone, V: Clone> IntoIterator for &'a mut SharedMap<K, V> {
    type Item = (&'a K, &'a mut V);
    type IntoIter = std::iter::Map<
        std::collections::btree_map::IterMut<'a, K, Arc<V>>,
        fn((&'a K, &'a mut Arc<V>)) -> (&'a K, &'a mut V),
    >;

    /// Mutable iteration, with the same copying cost as [`SharedMap::values_mut`].
    fn into_iter(self) -> Self::IntoIter {
        Arc::make_mut(&mut self.0)
            .iter_mut()
            .map(|(key, value)| (key, Arc::make_mut(value)))
    }
}

impl<K, V, Q> Index<&Q> for SharedMap<K, V>
where
    K: Ord + Borrow<Q>,
    Q: Ord + ?Sized,
{
    type Output = V;

    fn index(&self, key: &Q) -> &V {
        self.get(key).expect("no entry found for key")
    }
}

impl<K: Serialize, V: Serialize> Serialize for SharedMap<K, V> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_map(self.iter())
    }
}

impl<'de, K: Ord + Deserialize<'de>, V: Deserialize<'de>> Deserialize<'de> for SharedMap<K, V> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        BTreeMap::<K, V>::deserialize(deserializer).map(Self::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A write after a clone leaves the snapshot and every untouched entry shared.
    #[test]
    fn writes_copy_only_the_changed_entry() {
        let mut map: SharedMap<u32, String> = [(1, "one".to_owned()), (2, "two".to_owned())]
            .into_iter()
            .collect();
        let snapshot = map.clone();
        map.get_mut(&1).unwrap().push('!');
        assert_eq!(snapshot[&1], "one");
        assert_eq!(map[&1], "one!");
        assert!(std::ptr::eq(&snapshot[&2], &map[&2]));
        assert!(map.get_mut(&3).is_none());
        assert!(Arc::ptr_eq(&map.0, &map.clone().0));
    }

    /// Removal and replacement return the shared entry; the snapshot keeps its own.
    #[test]
    fn remove_and_insert_leave_snapshots_intact() {
        let mut map: SharedMap<u32, String> = SharedMap::new();
        assert!(map.insert(1, "one".to_owned()).is_none());
        let snapshot = map.clone();
        assert_eq!(map.remove(&1).as_deref().map(String::as_str), Some("one"));
        assert!(map.remove(&1).is_none());
        assert!(map.is_empty());
        assert_eq!(snapshot.len(), 1);
        assert_ne!(map, snapshot);
    }

    /// Sharing is visible per entry, survives copying an entry across, and never
    /// changes what equality means.
    #[test]
    fn shared_entries_are_detected_and_copied_by_reference() {
        let mut map: SharedMap<u32, String> = [(1, "one".to_owned()), (2, "two".to_owned())]
            .into_iter()
            .collect();
        let snapshot = map.clone();
        assert!(map.ptr_eq(&snapshot));
        map.get_mut(&1).unwrap().push('!');
        assert!(!map.ptr_eq(&snapshot));
        assert!(!map.shares_entry(&snapshot, &1));
        assert!(map.shares_entry(&snapshot, &2));
        assert!(!map.shares_entry(&snapshot, &3));
        let mut rebuilt = snapshot.clone();
        rebuilt.share_entry_from(&map, &1);
        assert!(rebuilt.shares_entry(&map, &1));
        assert_eq!(rebuilt, map);
        let equal_but_unshared: SharedMap<u32, String> =
            [(1, "one!".to_owned()), (2, "two".to_owned())]
                .into_iter()
                .collect();
        assert_eq!(equal_but_unshared, map);
        assert_ne!(snapshot, map);
    }

    /// Serialization matches a plain `BTreeMap`, so persisted formats are unchanged.
    #[test]
    fn serializes_like_a_btree_map() {
        let plain = BTreeMap::from([("a".to_owned(), 1), ("b".to_owned(), 2)]);
        let shared = SharedMap::from(plain.clone());
        let text = serde_json::to_string(&shared).unwrap();
        assert_eq!(text, serde_json::to_string(&plain).unwrap());
        let back: SharedMap<String, i32> = serde_json::from_str(&text).unwrap();
        assert_eq!(back, shared);
        assert_eq!(back.get("b"), Some(&2));
    }
}
