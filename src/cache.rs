//! A per-game-folder cache for what is read from an install once per process (the exe's tables, the art's, the rules):
//! replays and tests load them thousands of times.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Values by game folder. Only successes are kept: a failed read is made again on the next call.
pub struct FolderCache<T>(Mutex<Vec<(PathBuf, Arc<T>)>>);

impl<T> FolderCache<T> {
    pub const fn new() -> Self {
        Self(Mutex::new(Vec::new()))
    }

    /// The value for `root`, made by `read` on the first call for that folder. The lock is held while reading, so
    /// threads starting together read a folder once.
    pub fn get_or_try<E>(&self, root: &Path, read: impl FnOnce() -> Result<T, E>) -> Result<Arc<T>, E> {
        let mut cache = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((_, v)) = cache.iter().find(|(r, _)| r == root) {
            return Ok(v.clone());
        }
        let v = Arc::new(read()?);
        cache.push((root.to_path_buf(), v.clone()));
        Ok(v)
    }
}

impl<T> Default for FolderCache<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_successes_only() {
        let cache = FolderCache::<u32>::new();
        let root = Path::new("/a");
        assert_eq!(cache.get_or_try(root, || Err::<u32, _>("no")), Err("no"));
        assert_eq!(*cache.get_or_try(root, || Ok::<_, ()>(1)).unwrap(), 1);
        assert_eq!(*cache.get_or_try(root, || Ok::<_, ()>(2)).unwrap(), 1, "read once");
        assert_eq!(*cache.get_or_try(Path::new("/b"), || Ok::<_, ()>(3)).unwrap(), 3);
    }
}
