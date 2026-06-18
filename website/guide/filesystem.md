# File Storage

Larastvel's filesystem abstraction allows swapping local storage with cloud storage.

## LocalDisk

```rust
use larastvel_core::storage::{LocalDisk, StorageManager};

let disk = LocalDisk::new(
    PathBuf::from("storage/app"),
    "/storage".to_string(),
);

let mut manager = StorageManager::new("local".to_string());
manager.register("local", disk);
```

## Basic Operations

```rust
let disk = manager.default_disk()?;

// Write
disk.put("file.txt", b"contents").await?;
disk.put_string("greeting.txt", "Hello, World!").await?;

// Read
let bytes = disk.get("file.txt").await?;
let text = disk.get_string("greeting.txt").await?;

// Check
disk.exists("file.txt").await;
disk.missing("old.txt").await;

// Delete
disk.delete("file.txt").await?;

// Copy / Move
disk.copy("source.txt", "dest.txt").await?;
disk.move_file("old.txt", "new.txt").await?;

// Directory
disk.files("").await?;        // files in root
disk.all_files("").await?;    // recursive
disk.directories("").await?;  // dirs in root

// URL
disk.url("file.txt");  // "/storage/file.txt"

// Meta
disk.size("file.txt").await?;
disk.last_modified("file.txt").await?;
```
