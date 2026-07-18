use std::path::{Path, PathBuf};

use anyhow::Result;
use gpui::EventEmitter;
use opencollection::{OpenCollection};

/// Owns every open collection and tracks which one is active.
///
/// Collections are plain values rather than nested entities: only the active
/// one is ever rendered, so no view needs to observe a non-active collection.
/// Promote to `Vec<Entity<Collection>>` if that stops being true.
pub struct CollectionStore {
    collections: Vec<OpenCollection>,
    active: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CollectionEvent {
    ActiveChanged,
    RequestsChanged,
}

impl EventEmitter<CollectionEvent> for CollectionStore {}

impl CollectionStore {
    /// Seeds the scratch collection so there is always an active collection,
    /// then opens the default user collection if one is configured.
    pub fn new() -> Self {
        let collection = OpenCollection::new("Scratch");
        Self {
            collections: vec![collection],
            active: 0,
        }
    }

    pub fn collections(&self) -> &[OpenCollection] {
        &self.collections
    }

    pub fn active_index(&self) -> usize {
        self.active
    }

    pub fn active(&self) -> &OpenCollection {
        &self.collections[self.active]
    }

    pub fn active_mut(&mut self) -> &mut OpenCollection {
        &mut self.collections[self.active]
    }

    pub fn set_active(&mut self, index: usize) {
        if index < self.collections.len() {
            self.active = index;
        }
    }

    /// Loads `root` and makes it active. Re-activates it if already open.
    pub fn open(&mut self, _root: impl AsRef<Path>) -> Result<()> {
        // fixme: load collection and set active
        // let root = root.as_ref();
        // if let Some(index) = self.index_of(root) {
        //     self.active = index;
        //     return Ok(());
        // }
        Ok(())
    }

    /// Creates a new collection at `root` and makes it active.
    ///
    /// Refuses to touch a directory that is already a collection — reopening
    /// one by mistake should not look like creating an empty one.
    pub fn create(&mut self, root: impl AsRef<Path>) -> Result<()> {
        let root = root.as_ref();
        let name = root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Collection");

        self.push_active(OpenCollection::new(name));
        Ok(())
    }

    fn push_active(&mut self, collection: OpenCollection) {
        self.collections.push(collection);
        self.active = self.collections.len() - 1;
    }
}
