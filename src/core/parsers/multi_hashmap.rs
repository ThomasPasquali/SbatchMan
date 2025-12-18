use std::borrow::Borrow;
use std::collections::HashMap;
use std::hash::Hash;

/// A collection of hashmaps. Used to store multiple layers of key-value pairs,
/// where maps added later override earlier ones.
/// (ex. default variables, cluster variables, job variables).
pub struct MultiHashMap<K, V> {
  // Vector of hashmaps to search through. The later maps are searched first.
  maps: Vec<HashMap<K, V>>,
}

impl<K, V> MultiHashMap<K, V>
where
  K: std::cmp::Eq + std::hash::Hash,
{
  /// Creates a new MultiHashMap with the given vector of hashmaps.
  pub fn new() -> Self {
    Self {maps: Vec::new()}
  }

  /// Adds a new hashmap to the collection. This map will have the highest priority.
  pub fn push_map(&mut self, map: HashMap<K, V>) {
    self.maps.push(map);
  }

  pub fn pop_map(&mut self) -> Option<HashMap<K, V>> {
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
}