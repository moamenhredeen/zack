use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use gpui::EventEmitter;
use opencollection::{Folder, HttpRequest, Item, OpenCollection};

/// The root document at the top of a collection directory.
///
/// Zack stores collections unbundled — one file per request — so reading and
/// writing address the directory itself. This name is only used to recognise a
/// directory that already holds a collection.
const COLLECTION_FILE: &str = "opencollection.yml";

/// Where a request lives in the item tree: the index at each level, descending
/// into folders.
///
/// The document is the model, so a request is identified by its position in it
/// rather than by a copy. `ItemIter` only walks shared references, so this is
/// also what makes an edit-in-place lookup possible.
pub type ItemPath = Vec<usize>;

/// An open collection: the parsed document, plus the two things the document
/// does not model — where it came from on disk, and what the user is editing.
pub struct Collection {
    pub doc: OpenCollection,
    /// `None` for the scratch collection, which has no location yet.
    pub root: Option<PathBuf>,
    pub selected: Option<ItemPath>,
}

impl Collection {
    fn new(doc: OpenCollection, root: Option<PathBuf>) -> Self {
        let mut collection = Self {
            doc,
            root,
            selected: None,
        };
        collection.selected = collection.requests().first().map(|(path, _)| path.clone());
        collection
    }

    pub fn name(&self) -> String {
        self.doc
            .info
            .as_ref()
            .and_then(|info| info.name.clone())
            .unwrap_or_else(|| "Untitled".to_string())
    }

    /// Every HTTP request in the tree, paired with its path.
    ///
    /// Only HTTP requests are listed: they are the only kind Zack can edit or
    /// send today. Other request kinds stay in the document untouched.
    pub fn requests(&self) -> Vec<(ItemPath, &HttpRequest)> {
        let mut found = Vec::new();
        collect_requests(
            self.doc.items.as_deref().unwrap_or_default(),
            &mut Vec::new(),
            &mut found,
        );
        found
    }

    /// The item at `path`, borrowed from the live document.
    ///
    /// `save_item` matches items by pointer identity, so the borrow has to come
    /// from the document itself — a clone would be rejected as foreign.
    pub fn item_at(&self, path: &[usize]) -> Option<&Item> {
        item_at(self.doc.items.as_deref()?, path)
    }

    pub fn request_at(&self, path: &[usize]) -> Option<&HttpRequest> {
        match self.item_at(path)? {
            Item::Http(request) => Some(request),
            _ => None,
        }
    }

    pub fn request_at_mut(&mut self, path: &[usize]) -> Option<&mut HttpRequest> {
        match item_at_mut(self.doc.items.as_mut()?, path)? {
            Item::Http(request) => Some(request),
            _ => None,
        }
    }

    /// Appends a request at the top level and selects it.
    pub fn create_request(&mut self, name: &str) {
        let items = self.doc.items.get_or_insert_with(Vec::new);
        items.push(Item::Http(
            HttpRequest::get("https://httpbin.org/get").name(name),
        ));
        self.selected = Some(vec![items.len() - 1]);
    }

    /// Writes the whole tree: the root document plus one file per item.
    ///
    /// Saving back into the directory the collection was loaded from also
    /// prunes the files of items that are gone, so this is the write that makes
    /// a deletion stick. `save_item` is the cheaper path for an ordinary edit.
    pub fn save(&self) -> Result<()> {
        let root = self.root()?;
        self.doc.save(root)?;
        Ok(())
    }

    /// Writes back only the item at `path`, leaving every other file alone.
    ///
    /// An item that has never been saved has no file to write to yet, so it
    /// falls back to a full save, which places one. Callers do not need to know
    /// which case they are in.
    pub fn save_item(&self, path: &[usize]) -> Result<()> {
        let root = self.root()?;
        let item = self
            .item_at(path)
            .ok_or_else(|| anyhow!("no request at {path:?}"))?;

        if item.source().is_none() {
            return self.save();
        }

        self.doc.save_item(item, root)?;
        Ok(())
    }

    fn root(&self) -> Result<&Path> {
        self.root
            .as_deref()
            .ok_or_else(|| anyhow!("this collection has no location yet — create one first"))
    }
}

/// Owns every open collection and tracks which one is active.
pub struct CollectionStore {
    collections: Vec<Collection>,
    active: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CollectionEvent {
    ActiveChanged,
}

impl EventEmitter<CollectionEvent> for CollectionStore {}

impl CollectionStore {
    /// Seeds the scratch collection so there is always an active collection.
    pub fn new() -> Self {
        let mut scratch = unbundled("Scratch");
        scratch.items = Some(vec![Item::Http(
            HttpRequest::get("https://httpbin.org/get").name("New request"),
        )]);
        Self {
            collections: vec![Collection::new(scratch, None)],
            active: 0,
        }
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

    /// A collection by index, for acting on one a tab targets without
    /// disturbing which collection the sidebar is showing.
    pub fn collection(&self, index: usize) -> Option<&Collection> {
        self.collections.get(index)
    }

    pub fn collection_mut(&mut self, index: usize) -> Option<&mut Collection> {
        self.collections.get_mut(index)
    }

    pub fn set_active(&mut self, index: usize) {
        if index < self.collections.len() {
            self.active = index;
        }
    }

    /// Loads `root` and makes it active. Re-activates it if already open.
    pub fn open(&mut self, root: impl AsRef<Path>) -> Result<()> {
        let root = root.as_ref();
        if let Some(index) = self.index_of(root) {
            self.active = index;
            return Ok(());
        }

        // The directory, not the file inside it: that is what selects the
        // unbundled layout, and it is what anchors the per-item paths that
        // `save_item` later writes back to.
        let doc = OpenCollection::load(root)?;
        self.push_active(Collection::new(doc, Some(root.to_path_buf())));
        Ok(())
    }

    /// Creates a new collection at `root` and makes it active.
    ///
    /// Refuses to touch a directory that is already a collection — reopening
    /// one by mistake should not look like creating an empty one.
    pub fn create(&mut self, root: impl AsRef<Path>) -> Result<()> {
        let root = root.as_ref();
        if root.join(COLLECTION_FILE).exists() {
            return Err(anyhow!(
                "{} is already a collection — open it instead",
                root.display()
            ));
        }

        let name = root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Collection");

        let collection = Collection::new(unbundled(name), Some(root.to_path_buf()));
        collection.save()?;
        self.push_active(collection);
        Ok(())
    }

    fn index_of(&self, root: &Path) -> Option<usize> {
        self.collections
            .iter()
            .position(|collection| collection.root.as_deref() == Some(root))
    }

    fn push_active(&mut self, collection: Collection) {
        self.collections.push(collection);
        self.active = self.collections.len() - 1;
    }
}

/// A new collection, stored as a directory tree rather than one file.
///
/// The flag is what `save` and `save_item` dispatch on, so it is set at every
/// point Zack creates a document — without it, saving one request would rewrite
/// the whole collection as a single file.
fn unbundled(name: &str) -> OpenCollection {
    let mut doc = OpenCollection::new(name);
    doc.bundled = Some(false);
    doc
}

fn collect_requests<'a>(
    items: &'a [Item],
    prefix: &mut ItemPath,
    found: &mut Vec<(ItemPath, &'a HttpRequest)>,
) {
    for (index, item) in items.iter().enumerate() {
        prefix.push(index);
        match item {
            Item::Http(request) => found.push((prefix.clone(), request)),
            Item::Folder(Folder {
                items: Some(children),
                ..
            }) => collect_requests(children, prefix, found),
            _ => {}
        }
        prefix.pop();
    }
}

fn item_at<'a>(items: &'a [Item], path: &[usize]) -> Option<&'a Item> {
    let (index, rest) = path.split_first()?;
    let item = items.get(*index)?;
    if rest.is_empty() {
        return Some(item);
    }
    match item {
        Item::Folder(folder) => item_at(folder.items.as_deref()?, rest),
        _ => None,
    }
}

fn item_at_mut<'a>(items: &'a mut [Item], path: &[usize]) -> Option<&'a mut Item> {
    let (index, rest) = path.split_first()?;
    let item = items.get_mut(*index)?;
    if rest.is_empty() {
        return Some(item);
    }
    match item {
        Item::Folder(folder) => item_at_mut(folder.items.as_mut()?, rest),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencollection::HttpRequestDetails;

    fn nested() -> OpenCollection {
        OpenCollection::new("Test")
            .item(HttpRequest::get("https://example.com/one").name("One"))
            .item(
                Folder::new("Nested")
                    .item(HttpRequest::post("https://example.com/two").name("Two")),
            )
    }

    #[test]
    fn lists_requests_inside_folders_with_their_paths() {
        let collection = Collection::new(nested(), None);
        let paths: Vec<_> = collection
            .requests()
            .into_iter()
            .map(|(path, request)| (path, request.info.as_ref().unwrap().name.clone().unwrap()))
            .collect();

        assert_eq!(
            paths,
            vec![
                (vec![0], "One".to_string()),
                (vec![1, 0], "Two".to_string()),
            ]
        );
    }

    #[test]
    fn edits_a_request_inside_a_folder_in_place() {
        let mut collection = Collection::new(nested(), None);
        collection.selected = Some(vec![1, 0]);

        collection
            .request_at_mut(&[1, 0])
            .unwrap()
            .http
            .as_mut()
            .unwrap()
            .url = Some("https://example.com/edited".to_string());

        let (_, request) = &collection.requests()[1];
        assert_eq!(
            request.http.as_ref().unwrap().url.as_deref(),
            Some("https://example.com/edited")
        );
    }

    #[test]
    fn round_trips_through_disk() {
        let root = std::env::temp_dir().join("zack-store-round-trip");
        let _ = std::fs::remove_dir_all(&root);

        let mut store = CollectionStore::new();
        store.create(&root).unwrap();
        store.active_mut().create_request("Added");
        store.active().save().unwrap();

        // Unbundled: the request is its own file beside the root document.
        assert!(root.join("Added.yml").is_file());

        let mut reopened = CollectionStore::new();
        reopened.open(&root).unwrap();
        let requests = reopened.active().requests();

        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0].1.info.as_ref().unwrap().name.as_deref(),
            Some("Added")
        );
        assert_eq!(reopened.active().name(), "zack-store-round-trip");

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn saving_one_item_leaves_the_other_files_alone() {
        let root = std::env::temp_dir().join("zack-store-save-item");
        let _ = std::fs::remove_dir_all(&root);

        let mut store = CollectionStore::new();
        store.create(&root).unwrap();
        store.active_mut().create_request("First");
        store.active_mut().create_request("Second");
        store.active().save().unwrap();

        // Reopen so both items carry the source `save_item` writes back to.
        let mut store = CollectionStore::new();
        store.open(&root).unwrap();

        let untouched = std::fs::read_to_string(root.join("Second.yml")).unwrap();
        std::fs::write(root.join("Second.yml"), format!("{untouched}# sentinel\n")).unwrap();

        store.active_mut().request_at_mut(&[0]).unwrap().http = Some(HttpRequestDetails {
            url: Some("https://example.com/edited".to_string()),
            ..HttpRequestDetails::default()
        });
        store.active().save_item(&[0]).unwrap();

        assert!(
            std::fs::read_to_string(root.join("First.yml"))
                .unwrap()
                .contains("https://example.com/edited")
        );
        // Rewriting the whole tree would have dropped the sentinel.
        assert!(
            std::fs::read_to_string(root.join("Second.yml"))
                .unwrap()
                .contains("# sentinel")
        );

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn saving_a_never_saved_item_places_its_file() {
        let root = std::env::temp_dir().join("zack-store-save-new-item");
        let _ = std::fs::remove_dir_all(&root);

        let mut store = CollectionStore::new();
        store.create(&root).unwrap();
        store.active_mut().create_request("Fresh");

        // No file yet, so this has to fall back to a full save rather than fail.
        store.active().save_item(&[0]).unwrap();
        assert!(root.join("Fresh.yml").is_file());

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn refuses_to_create_over_an_existing_collection() {
        let root = std::env::temp_dir().join("zack-store-existing");
        let _ = std::fs::remove_dir_all(&root);

        let mut store = CollectionStore::new();
        store.create(&root).unwrap();
        assert!(store.create(&root).is_err());

        std::fs::remove_dir_all(&root).unwrap();
    }
}
