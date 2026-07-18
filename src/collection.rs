use std::path::{Path, PathBuf};

use anyhow::Result;
use gpui::EventEmitter;
use serde_yaml::Value;

use crate::model::RequestDraft;
use crate::opencollection;

/// A single request as it exists on disk.
///
/// `draft` is zack's model; `raw` is the parsed OpenCollection document it came
/// from, kept so saving does not drop keys zack does not understand yet.
#[derive(Clone, Debug)]
pub struct Request {
    pub path: PathBuf,
    pub relative_path: PathBuf,
    pub draft: RequestDraft,
    pub raw: Value,
    pub parse_error: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CollectionKind {
    User,
    /// The always-present scratch pad. Backed by a real directory in the app
    /// data dir, so every load/save path treats it like any other collection.
    Scratch,
}

#[derive(Clone, Debug)]
pub struct Collection {
    pub root: PathBuf,
    pub kind: CollectionKind,
    pub name: String,
    pub requests: Vec<Request>,
    pub selected: Option<PathBuf>,
}

impl Collection {
    pub fn load(root: impl AsRef<Path>, kind: CollectionKind) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        let (name, requests) = opencollection::load(&root)?;
        let selected = requests.first().map(|request| request.path.clone());
        Ok(Self {
            root,
            kind,
            name,
            requests,
            selected,
        })
    }

    pub fn is_scratch(&self) -> bool {
        self.kind == CollectionKind::Scratch
    }

    pub fn selected_request(&self) -> Option<&Request> {
        let path = self.selected.as_ref()?;
        self.requests.iter().find(|request| &request.path == path)
    }

    pub fn selected_request_mut(&mut self) -> Option<&mut Request> {
        let path = self.selected.clone()?;
        self.requests.iter_mut().find(|request| request.path == path)
    }

    pub fn select(&mut self, path: PathBuf) {
        self.selected = Some(path);
    }

    pub fn create_request(&mut self, name: &str) -> Result<PathBuf> {
        let request = opencollection::create_request(&self.root, name)?;
        let path = request.path.clone();
        self.requests.push(request);
        self.requests
            .sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
        self.selected = Some(path.clone());
        Ok(path)
    }

    /// Writes `draft` into the selected request and persists it.
    pub fn save_selected(&mut self, draft: &RequestDraft) -> Result<()> {
        let request = self
            .selected_request_mut()
            .ok_or_else(|| anyhow::anyhow!("No request is selected"))?;
        request.draft = draft.clone();
        opencollection::save_request(request)
    }
}

/// Owns every open collection and tracks which one is active.
///
/// Collections are plain values rather than nested entities: only the active
/// one is ever rendered, so no view needs to observe a non-active collection.
/// Promote to `Vec<Entity<Collection>>` if that stops being true.
pub struct CollectionStore {
    collections: Vec<Collection>,
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
        let mut store = Self {
            collections: Vec::new(),
            active: 0,
        };

        match scratch_root().and_then(|root| {
            opencollection::init_collection_root(&root, "Scratch")?;
            Collection::load(&root, CollectionKind::Scratch)
        }) {
            Ok(scratch) => store.collections.push(scratch),
            Err(error) => {
                // Fall back to an empty in-memory scratch so the UI still has
                // something to render.
                eprintln!("failed to open scratch collection: {error}");
                store.collections.push(Collection {
                    root: PathBuf::new(),
                    kind: CollectionKind::Scratch,
                    name: "Scratch".to_string(),
                    requests: Vec::new(),
                    selected: None,
                });
            }
        }

        if let Some(root) = default_collection_path() {
            let _ = store.open(root);
        }

        store
    }

    pub fn collections(&self) -> &[Collection] {
        &self.collections
    }

    pub fn active_index(&self) -> usize {
        self.active
    }

    pub fn active(&self) -> &Collection {
        &self.collections[self.active]
    }

    pub fn active_mut(&mut self) -> &mut Collection {
        &mut self.collections[self.active]
    }

    pub fn set_active(&mut self, index: usize) {
        if index < self.collections.len() {
            self.active = index;
        }
    }

    /// Loads `root` and makes it active. Re-activates it if already open.
    pub fn open(&mut self, root: impl AsRef<Path>) -> Result<()> {
        let root = root.as_ref();
        if let Some(index) = self
            .collections
            .iter()
            .position(|collection| collection.root == root)
        {
            self.active = index;
            return Ok(());
        }

        opencollection::ensure_collection_root(root)?;
        let collection = Collection::load(root, CollectionKind::User)?;
        self.collections.push(collection);
        self.active = self.collections.len() - 1;
        Ok(())
    }
}

fn scratch_root() -> Result<PathBuf> {
    let base = dirs::data_dir()
        .ok_or_else(|| anyhow::anyhow!("could not determine the app data directory"))?;
    Ok(base.join("zack").join("scratch"))
}

fn default_collection_path() -> Option<PathBuf> {
    std::env::var_os("ZACK_COLLECTION")
        .map(PathBuf::from)
        .or_else(|| {
            let sample = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("sample-collection");
            sample.is_dir().then_some(sample)
        })
}
