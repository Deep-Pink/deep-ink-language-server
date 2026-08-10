use im::{HashMap, HashSet};
use std::hash::{BuildHasher, Hash, RandomState};

#[derive(Clone)]
pub struct Multimap<Key: Hash + Eq + Clone, Value: Hash + Eq + Clone, State = RandomState> {
    inner: HashMap<Key, HashSet<Value>, State>,
}

impl<Key: Hash + Eq + Clone, Value: Hash + Eq + Clone, State: BuildHasher + Default> Default
    for Multimap<Key, Value, State>
{
    fn default() -> Self {
        Self {
            inner: HashMap::<Key, HashSet<Value>, State>::default(),
        }
    }
}

impl<Key: Hash + Eq + Clone, Value: Hash + Eq + Clone, State: BuildHasher + Default>
    Multimap<Key, Value, State>
{
    pub fn new() -> Self {
        Default::default()
    }
}

impl<Key: Hash + Eq + Clone, Value: Hash + Eq + Clone, State: BuildHasher>
    Multimap<Key, Value, State>
{
    pub fn get(&self, k: &Key) -> HashSet<Value> {
        let result = self.inner.get(&k).cloned();
        result.unwrap_or_default()
    }

    pub fn update(&self, k: Key, v: Value) -> Multimap<Key, Value, State> {
        let mut new_entry = HashSet::new();
        new_entry.insert(v);
        let result = self
            .inner
            .update_with(k, new_entry, |old, new| old.union(new));
        Multimap { inner: result }
    }

    pub fn remove_entry(&self, k: &Key) -> Multimap<Key, Value, State> {
        Multimap {
            inner: self.inner.without(k),
        }
    }
}

impl<Key: Clone + Hash + Eq, Value: Hash + Clone + Eq, State: BuildHasher>
    Multimap<Key, Value, State>
{
    pub fn remove_item(&self, k: &Key, v: &Value) -> Multimap<Key, Value, State> {
        if let Some(entry) = self.inner.get(k) {
            return Multimap {
                inner: self.inner.update(k.clone(), entry.without(v)),
            };
        } else {
            Multimap {
                inner: self.inner.clone(),
            }
        }
    }
}
