//! The registry machinery every loader shares.
//!
//! The reference's `generic_factory<T>` does the same four things for ninety
//! types (`src/generic_factory.h`): it separates abstracts from concrete
//! prototypes, both live in one namespace for `copy-from` (`:295-313`), an object
//! whose base is not loaded yet is deferred and retried (`:305-312`), and an id
//! loaded twice keeps its place in the list (`:336-368`).
//!
//! What a factory does *not* do is interpret fields: `extend`, `delete`,
//! `relative` and `proportional` are applied by each loader, one field at a time
//! (`mapdata.cpp:1072`, `assign.cpp:60-75`, `map_extras.cpp:1521`). That is why
//! this type carries objects, not values: the per-type work belongs to the
//! loaders that follow.

use cdda_json::Object;
use std::collections::BTreeMap;

/// What happened to an object handed to a factory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertOutcome {
    /// A concrete prototype was added.
    Inserted,
    /// An object with `abstract` was stored as a template, not as a prototype.
    Abstract,
    /// A concrete prototype was replaced; its position in the list is kept.
    Replaced,
    /// The object names a `copy-from` base that is not loaded yet.
    Deferred,
}

/// One prototype, as loaded: its id, its base, the object it came from, and the
/// content source it came from.
#[derive(Debug, Clone)]
pub struct Prototype {
    /// The id, from the registry's id member.
    pub id: String,
    /// The `copy-from` base, if any, resolved or not.
    pub base: Option<String>,
    /// The object as it appeared in the file.
    pub json: Object,
    /// The source name the reference records for provenance, e.g. `core` or `dda`.
    pub src: String,
    /// True once the base has been found.
    pub resolved: bool,
}

/// A factory for one registry.
#[derive(Debug)]
pub struct Factory {
    type_name: String,
    entities: Vec<Prototype>,
    index: BTreeMap<String, usize>,
    abstracts: BTreeMap<String, Prototype>,
    deferred: Vec<usize>,
}

impl Factory {
    /// A factory for a registry, named the way the digest names it.
    pub fn new(type_name: impl Into<String>) -> Self {
        Factory {
            type_name: type_name.into(),
            entities: Vec::new(),
            index: BTreeMap::new(),
            abstracts: BTreeMap::new(),
            deferred: Vec::new(),
        }
    }

    /// The registry's name.
    #[must_use]
    pub fn type_name(&self) -> &str {
        &self.type_name
    }

    /// The concrete ids, in the order the factory first saw them.
    #[must_use]
    pub fn ids(&self) -> Vec<String> {
        self.entities
            .iter()
            .map(|entity| entity.id.clone())
            .collect()
    }

    /// The prototypes, in list order.
    #[must_use]
    pub fn entries(&self) -> &[Prototype] {
        &self.entities
    }

    /// Whether an id is a loaded prototype.
    #[must_use]
    pub fn is_valid(&self, id: &str) -> bool {
        self.index.contains_key(id)
    }

    /// The prototype for an id.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Prototype> {
        self.index.get(id).and_then(|at| self.entities.get(*at))
    }

    /// The template stored under an abstract name.
    #[must_use]
    pub fn abstract_of(&self, name: &str) -> Option<&Prototype> {
        self.abstracts.get(name)
    }

    /// Takes one object from a content file.
    ///
    /// An abstract goes into the abstract table. A concrete object whose base is
    /// already known is inserted; one whose base is not known yet is deferred,
    /// because the reference retries it after the next file loads
    /// (`generic_factory.h:305-312`).
    pub fn insert(&mut self, json: Object, src: &str) -> InsertOutcome {
        if let Some(name) = string_member(&json, "abstract") {
            self.abstracts.insert(
                name.clone(),
                Prototype {
                    id: name,
                    base: string_member(&json, "copy-from"),
                    json,
                    src: src.to_owned(),
                    resolved: true,
                },
            );
            return InsertOutcome::Abstract;
        }
        let Some(id) = string_member(&json, "id") else {
            return InsertOutcome::Deferred;
        };
        let base = string_member(&json, "copy-from");
        let resolved = self.base_ready(base.as_deref());
        let prototype = Prototype {
            id: id.clone(),
            base,
            json,
            src: src.to_owned(),
            resolved,
        };
        if let Some(at) = self.index.get(&id).copied() {
            if let Some(slot) = self.entities.get_mut(at) {
                *slot = prototype;
            }
            if !resolved {
                self.deferred.push(at);
            }
            return InsertOutcome::Replaced;
        }
        let at = self.entities.len();
        self.entities.push(prototype);
        self.index.insert(id, at);
        if !resolved {
            self.deferred.push(at);
            return InsertOutcome::Deferred;
        }
        InsertOutcome::Inserted
    }

    /// Retries deferred objects until none can be resolved, and reports the ids
    /// that remain. A cycle or a base that is simply absent both land here: the
    /// reference cannot tell them apart either, because it only knows that the
    /// base is not in the map.
    ///
    /// # Errors
    ///
    /// Never fails; an unresolvable object is data to report, not an I/O error.
    pub fn finalize(&mut self) -> Result<Vec<String>, std::convert::Infallible> {
        loop {
            let mut progressed = false;
            let mut still = Vec::new();
            for at in std::mem::take(&mut self.deferred) {
                let Some(prototype) = self.entities.get(at) else {
                    continue;
                };
                if self.base_ready(prototype.base.as_deref()) {
                    if let Some(slot) = self.entities.get_mut(at) {
                        slot.resolved = true;
                    }
                    progressed = true;
                } else {
                    still.push(at);
                }
            }
            self.deferred = still;
            if !progressed {
                break;
            }
        }
        Ok(self
            .deferred
            .iter()
            .filter_map(|at| self.entities.get(*at).map(|prototype| prototype.id.clone()))
            .collect())
    }

    /// Whether a base is present *and* itself resolved.
    ///
    /// A base that exists but is still waiting for its own base cannot be
    /// inherited from, which is what makes a cycle terminate as a pair of
    /// unresolved ids rather than as a stack overflow.
    fn base_ready(&self, base: Option<&str>) -> bool {
        match base {
            None => true,
            Some(base) => {
                if self.abstracts.contains_key(base) {
                    return true;
                }
                self.index
                    .get(base)
                    .and_then(|at| self.entities.get(*at))
                    .is_some_and(|prototype| prototype.resolved)
            }
        }
    }

    /// Forgets everything, as a reload does.
    pub fn reset(&mut self) {
        self.entities.clear();
        self.index.clear();
        self.abstracts.clear();
        self.deferred.clear();
    }
}

/// A string member of an object, if it is a string.
fn string_member(object: &Object, name: &str) -> Option<String> {
    match object.get(name) {
        Some(cdda_json::Value::Str(text)) => Some(text.clone()),
        _ => None,
    }
}
