//! Naming things: content says `meat`, the world says `ItemTypeId(1234)`.
//!
//! The port stores item kinds as numbers because the world is queried by number a thousand times a turn. Content
//! names them as strings because that is how JSON writes them. This catalog is the one place the two meet, and it
//! is built once from the loaded item types rather than being re-derived wherever a name is needed.
//!
//! The number is the item's **position in the loaded list**, which is why the catalog is the only thing allowed to
//! make one: two callers numbering the same list differently would produce two worlds that disagree about what a
//! `meat` is.

use std::collections::BTreeMap;

use crate::items::ItemType;

/// The item names the world's numbers refer to.
#[derive(Debug, Default)]
pub struct ItemCatalog {
    ids: BTreeMap<String, u32>,
    names: Vec<String>,
}

impl ItemCatalog {
    /// Builds the catalog from the item types as they were loaded.
    #[must_use]
    pub fn new(items: &[ItemType]) -> Self {
        let mut ids = BTreeMap::new();
        let mut names = Vec::with_capacity(items.len());
        for (position, item) in items.iter().enumerate() {
            #[allow(clippy::cast_possible_truncation)]
            let number = position as u32;
            ids.insert(item.id.clone(), number);
            names.push(item.id.clone());
        }
        Self { ids, names }
    }

    /// The number for a name, if the content has such an item.
    #[must_use]
    pub fn id_of(&self, name: &str) -> Option<u32> {
        self.ids.get(name).copied()
    }

    /// The name for a number, if it is one this catalog made.
    #[must_use]
    pub fn name_of(&self, id: u32) -> Option<&str> {
        self.names.get(id as usize).map(String::as_str)
    }

    /// How many items the catalog knows.
    #[must_use]
    pub fn len(&self) -> usize {
        self.names.len()
    }

    /// Whether the catalog is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}
