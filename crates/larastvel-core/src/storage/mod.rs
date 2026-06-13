use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use tokio::fs;
use tokio::io::AsyncWriteExt;

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Path not found: {0}")]
    NotFound(String),
    #[error("Path is not a file: {0}")]
    NotAFile(String),
    #[error("Path is not a directory: {0}")]
    NotADirectory(String),
    #[error("Storage error: {0}")]
    General(String),
}

#[async_trait]
pub trait Filesystem: Send + Sync + std::fmt::Debug {
    async fn put(&self, path: &str, contents: &[u8]) -> Result<(), StorageError>;
    async fn put_string(&self, path: &str, contents: &str) -> Result<(), StorageError> {
        self.put(path, contents.as_bytes()).await
    }
    async fn get(&self, path: &str) -> Result<Vec<u8>, StorageError>;
    async fn get_string(&self, path: &str) -> Result<String, StorageError> {
        let bytes = self.get(path).await?;
        String::from_utf8(bytes).map_err(|e| StorageError::General(e.to_string()))
    }
    async fn exists(&self, path: &str) -> bool;
    async fn missing(&self, path: &str) -> bool {
        !self.exists(path).await
    }
    async fn delete(&self, path: &str) -> Result<(), StorageError>;
    async fn copy(&self, from: &str, to: &str) -> Result<(), StorageError>;
    async fn move_file(&self, from: &str, to: &str) -> Result<(), StorageError>;
    async fn size(&self, path: &str) -> Result<u64, StorageError>;
    async fn last_modified(&self, path: &str) -> Result<u64, StorageError>;
    async fn files(&self, directory: &str) -> Result<Vec<String>, StorageError>;
    async fn all_files(&self, directory: &str) -> Result<Vec<String>, StorageError>;
    async fn directories(&self, directory: &str) -> Result<Vec<String>, StorageError>;
    async fn all_directories(&self, directory: &str) -> Result<Vec<String>, StorageError>;
    async fn make_directory(&self, path: &str) -> Result<(), StorageError>;
    async fn delete_directory(&self, path: &str) -> Result<(), StorageError>;
    fn url(&self, path: &str) -> String;
    fn path(&self, relative: &str) -> PathBuf;
}

#[derive(Debug, Clone)]
pub struct LocalDisk {
    root: PathBuf,
    url_prefix: String,
}

impl LocalDisk {
    pub fn new(root: PathBuf, url_prefix: String) -> Self {
        Self { root, url_prefix }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn full_path(&self, path: &str) -> PathBuf {
        let sanitized = path.trim_start_matches('/');
        self.root.join(sanitized)
    }
}

#[async_trait]
impl Filesystem for LocalDisk {
    async fn put(&self, path: &str, contents: &[u8]) -> Result<(), StorageError> {
        let full = self.full_path(path);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).await?;
        }
        let mut file = fs::File::create(&full).await?;
        file.write_all(contents).await?;
        file.flush().await?;
        Ok(())
    }

    async fn get(&self, path: &str) -> Result<Vec<u8>, StorageError> {
        let full = self.full_path(path);
        if !full.exists() {
            return Err(StorageError::NotFound(path.to_string()));
        }
        if !full.is_file() {
            return Err(StorageError::NotAFile(path.to_string()));
        }
        Ok(fs::read(&full).await?)
    }

    async fn exists(&self, path: &str) -> bool {
        self.full_path(path).exists()
    }

    async fn delete(&self, path: &str) -> Result<(), StorageError> {
        let full = self.full_path(path);
        if !full.exists() {
            return Err(StorageError::NotFound(path.to_string()));
        }
        if full.is_dir() {
            fs::remove_dir_all(&full).await?;
        } else {
            fs::remove_file(&full).await?;
        }
        Ok(())
    }

    async fn copy(&self, from: &str, to: &str) -> Result<(), StorageError> {
        let from_full = self.full_path(from);
        let to_full = self.full_path(to);
        if !from_full.exists() {
            return Err(StorageError::NotFound(from.to_string()));
        }
        if let Some(parent) = to_full.parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::copy(&from_full, &to_full).await?;
        Ok(())
    }

    async fn move_file(&self, from: &str, to: &str) -> Result<(), StorageError> {
        let from_full = self.full_path(from);
        let to_full = self.full_path(to);
        if !from_full.exists() {
            return Err(StorageError::NotFound(from.to_string()));
        }
        if let Some(parent) = to_full.parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::rename(&from_full, &to_full).await?;
        Ok(())
    }

    async fn size(&self, path: &str) -> Result<u64, StorageError> {
        let full = self.full_path(path);
        if !full.exists() {
            return Err(StorageError::NotFound(path.to_string()));
        }
        let meta = fs::metadata(&full).await?;
        Ok(meta.len())
    }

    async fn last_modified(&self, path: &str) -> Result<u64, StorageError> {
        let full = self.full_path(path);
        if !full.exists() {
            return Err(StorageError::NotFound(path.to_string()));
        }
        let meta = fs::metadata(&full).await?;
        meta.modified()
            .map(|t| {
                t.duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
            })
            .map_err(|e| StorageError::General(e.to_string()))
    }

    async fn files(&self, directory: &str) -> Result<Vec<String>, StorageError> {
        let full = self.full_path(directory);
        if !full.exists() {
            return Err(StorageError::NotFound(directory.to_string()));
        }
        if !full.is_dir() {
            return Err(StorageError::NotADirectory(directory.to_string()));
        }
        let mut result = Vec::new();
        let mut entries = fs::read_dir(&full).await?;
        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_file() {
                if let Some(name) = entry.file_name().to_str() {
                    result.push(name.to_string());
                }
            }
        }
        result.sort();
        Ok(result)
    }

    async fn all_files(&self, directory: &str) -> Result<Vec<String>, StorageError> {
        let full = self.full_path(directory);
        if !full.exists() {
            return Err(StorageError::NotFound(directory.to_string()));
        }
        let mut result = Vec::new();
        self.collect_files_recursive(&full, &full, &mut result).await?;
        result.sort();
        Ok(result)
    }

    async fn directories(&self, directory: &str) -> Result<Vec<String>, StorageError> {
        let full = self.full_path(directory);
        if !full.exists() {
            return Err(StorageError::NotFound(directory.to_string()));
        }
        if !full.is_dir() {
            return Err(StorageError::NotADirectory(directory.to_string()));
        }
        let mut result = Vec::new();
        let mut entries = fs::read_dir(&full).await?;
        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    result.push(name.to_string());
                }
            }
        }
        result.sort();
        Ok(result)
    }

    async fn all_directories(&self, directory: &str) -> Result<Vec<String>, StorageError> {
        let full = self.full_path(directory);
        if !full.exists() {
            return Err(StorageError::NotFound(directory.to_string()));
        }
        let mut result = Vec::new();
        self.collect_dirs_recursive(&full, &full, &mut result).await?;
        result.sort();
        Ok(result)
    }

    async fn make_directory(&self, path: &str) -> Result<(), StorageError> {
        let full = self.full_path(path);
        fs::create_dir_all(&full).await?;
        Ok(())
    }

    async fn delete_directory(&self, path: &str) -> Result<(), StorageError> {
        let full = self.full_path(path);
        if !full.exists() {
            return Err(StorageError::NotFound(path.to_string()));
        }
        fs::remove_dir_all(&full).await?;
        Ok(())
    }

    fn url(&self, path: &str) -> String {
        let sanitized = path.trim_start_matches('/');
        format!("{}/{}", self.url_prefix.trim_end_matches('/'), sanitized)
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.full_path(relative)
    }
}

impl LocalDisk {
    async fn collect_files_recursive(
        &self,
        base: &Path,
        dir: &Path,
        result: &mut Vec<String>,
    ) -> Result<(), StorageError> {
        let mut entries = fs::read_dir(dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_dir() {
                Box::pin(self.collect_files_recursive(base, &path, result)).await?;
            } else if path.is_file() {
                if let Ok(relative) = path.strip_prefix(base) {
                    result.push(relative.to_string_lossy().to_string());
                }
            }
        }
        Ok(())
    }

    async fn collect_dirs_recursive(
        &self,
        base: &Path,
        dir: &Path,
        result: &mut Vec<String>,
    ) -> Result<(), StorageError> {
        let mut entries = fs::read_dir(dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_dir() {
                if let Ok(relative) = path.strip_prefix(base) {
                    result.push(relative.to_string_lossy().to_string());
                }
                Box::pin(self.collect_dirs_recursive(base, &path, result)).await?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct StorageManager {
    disks: HashMap<String, Arc<dyn Filesystem>>,
    default: String,
}

impl StorageManager {
    pub fn new(default_disk: String) -> Self {
        Self {
            disks: HashMap::new(),
            default: default_disk,
        }
    }

    pub fn register<D: Filesystem + 'static>(&mut self, name: &str, disk: D) {
        self.disks.insert(name.to_string(), Arc::new(disk));
    }

    pub fn disk(&self, name: &str) -> Result<Arc<dyn Filesystem>, StorageError> {
        self.disks
            .get(name)
            .cloned()
            .ok_or_else(|| StorageError::General(format!("Disk [{}] not configured", name)))
    }

    pub fn default_disk(&self) -> Result<Arc<dyn Filesystem>, StorageError> {
        self.disk(&self.default)
    }

    pub fn set_default(&mut self, name: &str) {
        self.default = name.to_string();
    }

    pub fn default_name(&self) -> &str {
        &self.default
    }

    pub fn disk_names(&self) -> Vec<String> {
        self.disks.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;

    fn temp_dir() -> PathBuf {
        let mut rng = rand::rngs::OsRng;
        let suffix: u64 = rng.gen();
        std::env::temp_dir().join(format!("larastvel_storage_test_{}", suffix))
    }

    fn create_disk() -> (LocalDisk, PathBuf) {
        let root = temp_dir();
        let disk = LocalDisk::new(root.clone(), "/storage".to_string());
        (disk, root)
    }

    async fn cleanup(root: &Path) {
        if root.exists() {
            fs::remove_dir_all(root).await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_put_and_get() {
        let (disk, root) = create_disk();
        disk.put("hello.txt", b"world").await.unwrap();
        let contents = disk.get("hello.txt").await.unwrap();
        assert_eq!(contents, b"world");
        cleanup(&root).await;
    }

    #[tokio::test]
    async fn test_put_string_and_get_string() {
        let (disk, root) = create_disk();
        disk.put_string("greeting.txt", "Hello, World!")
            .await
            .unwrap();
        let contents = disk.get_string("greeting.txt").await.unwrap();
        assert_eq!(contents, "Hello, World!");
        cleanup(&root).await;
    }

    #[tokio::test]
    async fn test_exists() {
        let (disk, root) = create_disk();
        assert!(!disk.exists("test.txt").await);
        disk.put("test.txt", b"data").await.unwrap();
        assert!(disk.exists("test.txt").await);
        cleanup(&root).await;
    }

    #[tokio::test]
    async fn test_missing() {
        let (disk, root) = create_disk();
        assert!(disk.missing("nonexistent.txt").await);
        disk.put("exists.txt", b"x").await.unwrap();
        assert!(!disk.missing("exists.txt").await);
        cleanup(&root).await;
    }

    #[tokio::test]
    async fn test_delete_file() {
        let (disk, root) = create_disk();
        disk.put("delete_me.txt", b"bye").await.unwrap();
        assert!(disk.exists("delete_me.txt").await);
        disk.delete("delete_me.txt").await.unwrap();
        assert!(!disk.exists("delete_me.txt").await);
        cleanup(&root).await;
    }

    #[tokio::test]
    async fn test_delete_nonexistent() {
        let (disk, root) = create_disk();
        let result = disk.delete("nowhere.txt").await;
        assert!(result.is_err());
        cleanup(&root).await;
    }

    #[tokio::test]
    async fn test_copy() {
        let (disk, root) = create_disk();
        disk.put("source.txt", b"source content")
            .await
            .unwrap();
        disk.copy("source.txt", "dest.txt").await.unwrap();
        assert_eq!(disk.get("dest.txt").await.unwrap(), b"source content");
        cleanup(&root).await;
    }

    #[tokio::test]
    async fn test_move_file() {
        let (disk, root) = create_disk();
        disk.put("original.txt", b"move me").await.unwrap();
        disk.move_file("original.txt", "moved.txt").await.unwrap();
        assert!(!disk.exists("original.txt").await);
        assert_eq!(disk.get("moved.txt").await.unwrap(), b"move me");
        cleanup(&root).await;
    }

    #[tokio::test]
    async fn test_size() {
        let (disk, root) = create_disk();
        disk.put("sized.txt", b"12345").await.unwrap();
        assert_eq!(disk.size("sized.txt").await.unwrap(), 5);
        cleanup(&root).await;
    }

    #[tokio::test]
    async fn test_files() {
        let (disk, root) = create_disk();
        disk.put("a.txt", b"").await.unwrap();
        disk.put("b.txt", b"").await.unwrap();
        disk.put("sub/c.txt", b"").await.unwrap();
        let files = disk.files("").await.unwrap();
        assert_eq!(files, vec!["a.txt", "b.txt"]);
        cleanup(&root).await;
    }

    #[tokio::test]
    async fn test_all_files_recursive() {
        let (disk, root) = create_disk();
        disk.put("root.txt", b"").await.unwrap();
        disk.put("sub/a.txt", b"").await.unwrap();
        disk.put("sub/deep/b.txt", b"").await.unwrap();
        let all = disk.all_files("").await.unwrap();
        assert_eq!(all, vec!["root.txt", "sub/a.txt", "sub/deep/b.txt"]);
        cleanup(&root).await;
    }

    #[tokio::test]
    async fn test_directories() {
        let (disk, root) = create_disk();
        disk.put("sub1/file.txt", b"").await.unwrap();
        disk.put("sub2/file.txt", b"").await.unwrap();
        disk.put("root.txt", b"").await.unwrap();
        let dirs = disk.directories("").await.unwrap();
        assert_eq!(dirs, vec!["sub1", "sub2"]);
        cleanup(&root).await;
    }

    #[tokio::test]
    async fn test_all_directories_recursive() {
        let (disk, root) = create_disk();
        disk.put("a/x.txt", b"").await.unwrap();
        disk.put("a/b/y.txt", b"").await.unwrap();
        disk.put("a/b/c/z.txt", b"").await.unwrap();
        let dirs = disk.all_directories("").await.unwrap();
        assert_eq!(dirs, vec!["a", "a/b", "a/b/c"]);
        cleanup(&root).await;
    }

    #[tokio::test]
    async fn test_make_and_delete_directory() {
        let (disk, root) = create_disk();
        disk.make_directory("new_dir").await.unwrap();
        assert!(root.join("new_dir").exists());
        disk.delete_directory("new_dir").await.unwrap();
        assert!(!root.join("new_dir").exists());
        cleanup(&root).await;
    }

    #[tokio::test]
    async fn test_url() {
        let disk = LocalDisk::new(PathBuf::from("/tmp"), "/storage".to_string());
        assert_eq!(disk.url("file.txt"), "/storage/file.txt");
        assert_eq!(disk.url("/file.txt"), "/storage/file.txt");
        assert_eq!(disk.url("sub/file.txt"), "/storage/sub/file.txt");
    }

    #[tokio::test]
    async fn test_path() {
        let disk = LocalDisk::new(PathBuf::from("/app/storage"), "".to_string());
        assert_eq!(disk.path("file.txt"), PathBuf::from("/app/storage/file.txt"));
        assert_eq!(
            disk.path("sub/dir/file.txt"),
            PathBuf::from("/app/storage/sub/dir/file.txt")
        );
    }

    #[tokio::test]
    async fn test_get_nonexistent() {
        let (disk, root) = create_disk();
        let result = disk.get("nowhere.txt").await;
        assert!(result.is_err());
        cleanup(&root).await;
    }

    #[tokio::test]
    async fn test_overwrite_file() {
        let (disk, root) = create_disk();
        disk.put("file.txt", b"original").await.unwrap();
        disk.put("file.txt", b"overwritten").await.unwrap();
        assert_eq!(disk.get("file.txt").await.unwrap(), b"overwritten");
        cleanup(&root).await;
    }

    #[tokio::test]
    async fn test_put_in_subdirectory() {
        let (disk, root) = create_disk();
        disk.put("nested/deep/file.txt", b"deep").await.unwrap();
        assert!(disk.exists("nested/deep/file.txt").await);
        assert_eq!(disk.get("nested/deep/file.txt").await.unwrap(), b"deep");
        cleanup(&root).await;
    }

    #[tokio::test]
    async fn test_storage_manager() {
        let (disk, _root) = create_disk();
        let mut manager = StorageManager::new("local".to_string());
        manager.register("local", disk);

        let retrieved = manager.disk("local").unwrap();
        assert!(retrieved.exists("nonexistent").await == false);

        let default = manager.default_disk().unwrap();
        assert!(default.exists("nonexistent").await == false);
    }

    #[tokio::test]
    async fn test_storage_manager_missing_disk() {
        let manager = StorageManager::new("local".to_string());
        let result = manager.disk("nonexistent");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_storage_manager_default_name() {
        let manager = StorageManager::new("local".to_string());
        assert_eq!(manager.default_name(), "local");
    }

    #[tokio::test]
    async fn test_storage_manager_disk_names() {
        let mut manager = StorageManager::new("local".to_string());
        let (disk, _root) = create_disk();
        manager.register("local", disk);
        let (disk2, _root2) = create_disk();
        manager.register("s3", disk2);
        let mut names = manager.disk_names();
        names.sort();
        assert_eq!(names, vec!["local", "s3"]);
    }

    #[tokio::test]
    async fn test_delete_directory_with_contents() {
        let (disk, root) = create_disk();
        disk.put("dir/a.txt", b"").await.unwrap();
        disk.put("dir/sub/b.txt", b"").await.unwrap();
        assert!(root.join("dir").exists());
        disk.delete_directory("dir").await.unwrap();
        assert!(!root.join("dir").exists());
        cleanup(&root).await;
    }

    #[tokio::test]
    async fn test_last_modified() {
        let (disk, root) = create_disk();
        disk.put("fresh.txt", b"data").await.unwrap();
        let modified = disk.last_modified("fresh.txt").await.unwrap();
        assert!(modified > 0);
        cleanup(&root).await;
    }
}
