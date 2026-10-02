//! In-memory index of bucket paths backing direct bucket serving.
//!
//! The server serves files at bucket paths by consulting this registry
//! instead of mounts compiled into the router at startup, so buckets created
//! after startup are available immediately — no restart.

use crate::db::Database;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tokio::sync::Mutex;

/// Absolute, normalized bucket path (`/photos/sub`) → public flag.
type BucketMap = HashMap<String, bool>;

/// Outcome of [`BucketRegistry::longest_match`]: the matched path prefix and
/// whether the bucket at that prefix is public.
pub type Match = (String, bool);

/// In-memory view of every bucket path, consulted when serving requests
/// directly at bucket paths.
///
/// Loaded at startup from the database, written through on API bucket
/// creates, and periodically reconciled so writers that bypass this process
/// (the CLI, another instance sharing the database) become visible without a
/// restart.
///
/// Readers clone the current `Arc` map (one atomic) and probe a handful of
/// segment prefixes — no database access and no lock held while serving.
/// Writers serialize behind an async mutex so a reconciliation refresh can
/// never overwrite a concurrent write-through with a stale snapshot; the map's
/// write lock is only ever held for a pointer swap, never for work that
/// scales with bucket count.
#[derive(Debug, Default)]
pub struct BucketRegistry {
    inner: RwLock<Arc<BucketMap>>,
    write: Mutex<()>,
}

impl BucketRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace the entire map.
    pub async fn set_all(&self, entries: impl IntoIterator<Item = (String, bool)>) {
        let _write = self.write.lock().await;
        self.swap(entries.into_iter().collect());
    }

    /// Insert or update a single bucket path (API write-through).
    ///
    /// The key is normalized the way the database stores it: a leading `/`
    /// and no trailing `/`, so lookups match `refresh`ed entries exactly.
    pub async fn upsert(&self, path: &str, public: bool) {
        let key = normalize_key(path);
        let _write = self.write.lock().await;
        let mut next = self.snapshot();
        next.insert(key, public);
        self.swap(next);
    }

    /// Remove a bucket path (used when a bucket is deleted).
    pub async fn remove(&self, path: &str) {
        let key = normalize_key(path);
        let _write = self.write.lock().await;
        let mut next = self.snapshot();
        next.remove(&key);
        self.swap(next);
    }

    /// Reload every bucket path from the database.
    ///
    /// The query runs while the write mutex is held, so a concurrent
    /// [`upsert`](Self::upsert) either lands before the snapshot (and is
    /// included) or applies after the swap (and is not lost). On error the
    /// current map is kept — a transient database failure never empties the
    /// registry; the next reconciliation retries.
    pub async fn refresh(&self, db: &Database) -> anyhow::Result<()> {
        let _write = self.write.lock().await;
        let entries = super::get_paths_with_privacy(db).await?;
        self.swap(entries.into_iter().collect());
        Ok(())
    }

    /// Longest segment-aligned prefix of `req_path` present in the registry.
    ///
    /// Only prefixes of the request path itself are probed, so matches fall
    /// on segment boundaries: `/photos` matches `/photos` and `/photos/a.jpg`
    /// but never `/photos-archive/a.jpg`. A private match is returned as well
    /// — callers must deny it instead of falling back to a public ancestor.
    pub fn longest_match(&self, req_path: &str) -> Option<Match> {
        let map = self.read();
        let mut candidate = req_path;
        while !candidate.is_empty() {
            if let Some(public) = map.get(candidate) {
                return Some((candidate.to_string(), *public));
            }
            match candidate.rsplit_once('/') {
                Some((parent, _)) if !parent.is_empty() => candidate = parent,
                _ => break,
            }
        }
        None
    }

    fn read(&self) -> Arc<BucketMap> {
        Arc::clone(
            &self
                .inner
                .read()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
        )
    }

    fn snapshot(&self) -> BucketMap {
        (*self.read()).clone()
    }

    /// Install a new map. The root path `/` is dropped: a bucket occupying
    /// the root would publicly serve the entire storage directory, and bucket
    /// creation rejects it anyway — never let a hand-edited row in.
    fn swap(&self, mut map: BucketMap) {
        map.remove("/");
        *self
            .inner
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Arc::new(map);
    }
}

/// Normalize a bucket path to the database form: leading `/`, no trailing `/`.
fn normalize_key(path: &str) -> String {
    let path = path.trim_end_matches('/');
    if path.is_empty() {
        "/".to_string()
    } else if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn registry_with(entries: &[(&str, bool)]) -> BucketRegistry {
        let registry = BucketRegistry::new();
        registry
            .set_all(
                entries
                    .iter()
                    .map(|(path, public)| (path.to_string(), *public)),
            )
            .await;
        registry
    }

    #[tokio::test]
    async fn matches_bucket_root_and_files_under_it() {
        let registry = registry_with(&[("/photos", true)]).await;

        assert_eq!(
            registry.longest_match("/photos"),
            Some(("/photos".to_string(), true))
        );
        assert_eq!(
            registry.longest_match("/photos/a.jpg"),
            Some(("/photos".to_string(), true))
        );
        assert_eq!(
            registry.longest_match("/photos/deep/dir/a.jpg"),
            Some(("/photos".to_string(), true))
        );
    }

    #[tokio::test]
    async fn matches_only_on_segment_boundaries() {
        let registry = registry_with(&[("/photos", true)]).await;

        assert_eq!(registry.longest_match("/photos-archive/a.jpg"), None);
        assert_eq!(registry.longest_match("/photosx"), None);
        assert_eq!(registry.longest_match("/other"), None);
        assert_eq!(registry.longest_match("/"), None);
        assert_eq!(registry.longest_match(""), None);
    }

    #[tokio::test]
    async fn longest_prefix_wins_for_nested_buckets() {
        let registry = registry_with(&[("/photos", true), ("/photos/sub", true)]).await;

        assert_eq!(
            registry.longest_match("/photos/sub/a.jpg"),
            Some(("/photos/sub".to_string(), true))
        );
        assert_eq!(
            registry.longest_match("/photos/other/a.jpg"),
            Some(("/photos".to_string(), true))
        );
    }

    #[tokio::test]
    async fn private_match_is_returned_so_callers_can_deny() {
        // A private child under a public parent must win the match — falling
        // back to the public ancestor would leak the private files.
        let registry = registry_with(&[("/pub", true), ("/pub/secret", false)]).await;

        assert_eq!(
            registry.longest_match("/pub/secret/f.txt"),
            Some(("/pub/secret".to_string(), false))
        );
        assert_eq!(
            registry.longest_match("/pub/open/f.txt"),
            Some(("/pub".to_string(), true))
        );
    }

    #[tokio::test]
    async fn upsert_and_remove_take_effect_immediately() {
        let registry = BucketRegistry::new();

        registry.upsert("/live-bucket/", true).await;
        assert_eq!(
            registry.longest_match("/live-bucket/f.txt"),
            Some(("/live-bucket".to_string(), true))
        );

        registry.remove("/live-bucket").await;
        assert_eq!(registry.longest_match("/live-bucket/f.txt"), None);
    }

    #[tokio::test]
    async fn root_path_is_never_registered() {
        let registry = BucketRegistry::new();

        registry.upsert("/", true).await;
        assert_eq!(registry.longest_match("/anything"), None);

        registry.set_all([("/".to_string(), true)]).await;
        assert_eq!(registry.longest_match("/anything"), None);
    }
}
