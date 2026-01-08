use std::borrow::Borrow;
use std::collections::hash_map::Iter as MapIter;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::iter::Rev;
use std::slice::Iter as SliceIter;

/// A collection of hashmaps. Used to store multiple layers of key-value pairs,
/// where maps added later override earlier ones.
/// (ex. default variables, cluster variables, job variables).
pub struct MultiHashMap<K, V> {
  // Vector of hashmaps to search through. The later maps are searched first.
  maps: Vec<HashMap<K, V>>,
}

impl<K, V> MultiHashMap<K, V>
where
  K: Eq + Hash,
{
  /// Creates a new MultiHashMap with the given vector of hashmaps.
  pub fn new() -> Self {
    Self { maps: Vec::new() }
  }

  /// Adds a new hashmap to the collection. This map will have the highest priority.
  pub fn push(&mut self, map: HashMap<K, V>) {
    self.maps.push(map);
  }

  pub fn pop(&mut self) -> Option<HashMap<K, V>> {
    self.maps.pop()
  }

  pub fn get<Q: ?Sized>(&self, key: &Q) -> Option<&V>
  where
    K: Borrow<Q>,
    Q: Hash + Eq,
  {
    // Search through the maps in reverse order to respect priority
    for map in self.maps.iter().rev() {
      if let Some(value) = map.get(key) {
        return Some(value);
      }
    }
    None
  }

  pub fn iter(&self) -> MultiHashMapIter<'_, K, V> {
    MultiHashMapIter {
      maps_iter: self.maps.iter().rev(),
      current_map_iter: None,
      visited_keys: HashSet::new(),
    }
  }
}

pub struct MultiHashMapIter<'a, K, V> {
  /// Iterate over maps in reverse order (highest priority first)
  maps_iter: Rev<SliceIter<'a, HashMap<K, V>>>,

  /// Iterator for the specific map we are currently looking at
  current_map_iter: Option<MapIter<'a, K, V>>,

  /// Track keys we've already yielded to prevent duplicates
  visited_keys: HashSet<&'a K>,
}

impl<'a, K, V> Iterator for MultiHashMapIter<'a, K, V>
where
  K: Eq + Hash,
{
  type Item = (&'a K, &'a V);

  fn next(&mut self) -> Option<Self::Item> {
    loop {
      // Ensure we have an active map iterator
      if let Some(map_iter) = &mut self.current_map_iter {
        // Try to get the next item from the current map
        for (key, value) in map_iter {
          // Check if we have seen this key in a higher priority map
          // .insert returns true if the key was NOT present
          if self.visited_keys.insert(key) {
            return Some((key, value));
          }
          // If insert returns false, this key was already yielded
          // by a "newer" map, so we skip this "older" version.
        }
      }

      // If current_map_iter is exhausted (or None), move to the next map
      match self.maps_iter.next() {
        Some(next_map) => {
          self.current_map_iter = Some(next_map.iter());
          // Loop continues immediately to try the new map
        }
        None => return None, // No maps left
      }
    }
  }
}

impl<K, V> std::fmt::Display for MultiHashMap<K, V>
where
  K: Eq + Hash + Ord + std::fmt::Debug,
  V: std::fmt::Debug,
{
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let mut entries: Vec<_> = self.iter().collect();
    entries.sort_by(|a, b| a.0.cmp(b.0));

    write!(f, "{{")?;
    for (i, (k, v)) in entries.iter().enumerate() {
      if i > 0 {
        write!(f, ", ")?;
      }
      write!(f, "{:?}: {:?}", k, v)?;
    }
    write!(f, "}}")
  }
}
