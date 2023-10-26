To accomplish the task described, we'll create a Rust program with a modular structure. We'll use the `walkdir`, `gray_matter`, and `rsync` crates to handle file operations and front matter extraction. Below is the code, divided into `lib.rs` and `main.rs`, and assumes you have a Cargo project set up with dependencies specified in your `Cargo.toml`:

Cargo.toml (add dependencies):
```toml
[dependencies]
walkdir = "2.3"
gray_matter = "0.11"
rsync = "0.4"
```

lib.rs:
```rust
use std::fs;
use std::path::Path;
use walkdir::WalkDir;
use gray_matter::parse_and_find;
use rsync::Rsync;

pub fn find_markdown_files_with_tags(root_dir: &str, wanted_tags: Vec<&str>) -> Vec<String> {
    let mut result = Vec::new();

    for entry in WalkDir::new(root_dir).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            if entry.path().extension() == Some("md".as_ref()) {
                if let Ok(content) = fs::read_to_string(entry.path()) {
                    if let Ok(parsed) = parse_and_find(&content, wanted_tags) {
                        result.push(entry.path().to_str().unwrap().to_string());
                    }
                }
            }
        }
    }

    result
}

pub fn rsync_files(source_files: Vec<String>, dest_dir: &str) {
    let mut rsync = Rsync::from_paths(source_files)
        .archive()
        .update()
        .progress();

    rsync.set_from_flags(vec!["-q"]); // Optional: Additional rsync flags

    rsync.push(dest_dir).execute().unwrap();
}
```

main.rs:
```rust
use std::env;
use std::process;
use your_project_name::find_markdown_files_with_tags;
use your_project_name::rsync_files;

fn main() {
    let args: Vec<String> = env::args().collect();
    
    if args.len() != 4 {
        eprintln!("Usage: {} <source_dir> <destination_dir> <wanted_tag>", args[0]);
        process::exit(1);
    }

    let source_dir = &args[1];
    let dest_dir = &args[2];
    let wanted_tag = &args[3];

    let markdown_files = find_markdown_files_with_tags(source_dir, vec![wanted_tag]);

    if markdown_files.is_empty() {
        println!("No markdown files found with the specified tag.");
        return;
    }

    for file in &markdown_files {
        println!("Found: {}", file);
    }

    rsync_files(markdown_files, dest_dir);
    println!("Rsync completed successfully!");
}
```

Replace `your_project_name` with the actual name of your Rust project.

To run the program, you would use the command line as follows:

```bash
cargo run -- <source_directory> <destination_directory> <wanted_tag>
```

This will search for Markdown files in the source directory, filter those with the specified YAML front matter tag, and rsync them to the destination directory.