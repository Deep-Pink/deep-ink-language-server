use im::HashSet;
use std::hash::{BuildHasher, Hash, RandomState};

use crate::multimap::Multimap;

#[derive(Clone)]
pub struct ReferenceMap<
    Key: Clone + Eq + Hash,
    Value: Clone + Eq + Hash,
    State: BuildHasher + Default = RandomState,
> {
    forward: Multimap<Key, Value, State>,
    backward: Multimap<Value, Key, State>,
}

impl<'s, Key: Clone + Eq + Hash, Value: Clone + Eq + Hash, State: BuildHasher + Default + Clone>
    ReferenceMap<Key, Value, State>
{
    pub fn new() -> Self {
        ReferenceMap {
            forward: Multimap::new(),
            backward: Multimap::new(),
        }
    }

    pub fn update(&self, k: Key, v: Value) -> ReferenceMap<Key, Value, State> {
        let forward = &self.forward;
        let backward = &self.backward;
        let next_forward = forward.update(k.clone(), v.clone());
        let next_backward = backward.update(v.clone(), k.clone());
        ReferenceMap {
            forward: next_forward,
            backward: next_backward,
        }
    }

    pub fn get(&self, k: &Key) -> HashSet<Value> {
        self.forward.get(&k)
    }

    pub fn get_dependencies(&self, v: &Value) -> HashSet<Key> {
        self.backward.get(&v)
    }

    pub fn remove_item(&self, k: &Key, v: &Value) -> ReferenceMap<Key, Value, State> {
        let result_backward = self.backward.remove_item(v, k);
        let result_forward = self.forward.remove_item(k, v);
        ReferenceMap {
            forward: result_forward,
            backward: result_backward,
        }
    }

    pub fn remove_value(&self, v: &Value) -> ReferenceMap<Key, Value, State> {
        let backward_entries = self.backward.get(v);
        let mut result_forward = self.forward.clone();
        for e in backward_entries {
            result_forward = result_forward.remove_item(&e, v);
        }
        ReferenceMap {
            forward: result_forward,
            backward: self.backward.remove_entry(v),
        }
    }

    pub fn remove_entry(&self, k: &Key) -> ReferenceMap<Key, Value, State> {
        let mut result_backward = self.backward.clone();
        for d in self.forward.get(&k) {
            result_backward = result_backward.remove_item(&d, k);
        }
        let result_forward = self.forward.remove_entry(k);
        ReferenceMap {
            forward: result_forward,
            backward: result_backward,
        }
    }
}
