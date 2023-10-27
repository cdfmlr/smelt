use gray_matter::engine::YAML;
use gray_matter::Matter;
use log::warn;
use once_cell::sync::Lazy;
use regex::Regex;

use std::fs;
use std::path::Path;
use walkdir::{DirEntry, WalkDir};

/// is_markdown checks if the given DirEntry is a markdown file.
fn is_markdown(entry: &DirEntry) -> bool {
    if !entry.file_type().is_file() {
        return false;
    }

    entry
        .file_name()
        .to_str()
        .map(|s| s.ends_with(".md"))
        .unwrap_or(false)
}

/// MATTER is a singleton that can be used to parse
/// markdown files, and extract the YAML front matters.
static MATTER: Lazy<Matter<YAML>> = Lazy::new(|| Matter::<YAML>::new());
// 真逆天啊，编译器提示用第三方库。。

#[derive(Debug)]
pub enum CheckMarkdownFrontMatterError {
    WalkDirIterError(walkdir::Error),
    ReadFileError(std::io::Error),
    AsHashmapError(gray_matter::Error),
    RefDataNone,
    AsStringError(gray_matter::Error),
}

use CheckMarkdownFrontMatterError::*;

/// contains_tag checks if the given markdown file contains
/// the given key/value pair in its YAML front matter.
fn contains_tag(
    markdown_file: &Path,
    key: &str,
    value: &str,
) -> Result<bool, CheckMarkdownFrontMatterError> {
    let content = fs::read_to_string(markdown_file).or_else(|err| Err(ReadFileError(err)))?;
    let result = MATTER.parse(content.trim());
    if result.data.is_none() {
        return Ok(false);
    }

    let data = result.data.as_ref().ok_or(RefDataNone)?;

    // let got_value = data[key].as_string()?;
    // data[key] panic if no entry found for key
    let data = data.as_hashmap().or_else(|err| Err(AsHashmapError(err)))?;
    let got_value = data.get(key);

    if let None = got_value {
        return Ok(false);
    }

    let got_value = got_value
        .unwrap()
        .as_string()
        .or_else(|err| Err(AsStringError(err)))?;
    Ok(got_value == value)
}

/// find_markdown_files walks the given directory and returns
/// an iter of all markdown files.
fn find_markdown_files(dir: &Path) -> impl Iterator<Item = DirEntry> {
    walkdir_iter(dir).into_iter().filter(is_markdown)
}

/// find_markdown_files_with_tag walks the given directory and returns
/// an iter of all markdown files that contain the given key/value pair
/// in their YAML front matter.
pub fn find_markdown_files_with_tag<'a, P: AsRef<Path>>(
    dir: P,
    key: &'a str,
    value: &'a str,
) -> impl Iterator<Item = DirEntry> + 'a {
    // but why 'a is needed here?
    let dir = dir.as_ref();
    find_markdown_files(dir).filter(|e| {
        contains_tag(e.path(), key, value).unwrap_or_else(|err| {
            warn!(
                "failed to check YAML front matter from {:?}: err = {:?}",
                e.path(),
                err
            );
            false
        })
    })
}

/// print_files prints the path of each file in the given iter.
pub fn print_files(files: impl Iterator<Item = DirEntry>) {
    for file in files {
        println!("{}", file.path().to_str().unwrap());
    }
}

// #[derive(Error, Debug)]
// pub enum RsyncFilesError {
//     #[error("failed to create temp dir: {0}")]
//     CreateTempDirError(io::Error),
//     #[error("failed to create sub dir in temp dir: {reason}")]
//     SubDirInTempDirError{reason: String},
//     #[error("failed to hard link {src} to {dst}: {err}")]
//     HardLinkError{src: String, dst: String, err: io::Error},
// }
//
// use RsyncFilesError::*;

/// rsync_files hard links all files in the given iter to a temporary directory
/// and then exec rsync(1) to sync the temporary directory to the given dst.
///
/// Errors if any of the src_files is not in the src_base_dir.
///
/// The temporary directory is necessary here to
/// - make a "view" of the filtered src files (src_files);
/// - keep them in the same directory structure as the src_base_dir;
/// - make sure only src_files are synced to the dst.
pub fn rsync_files(
    src_base_dir: impl AsRef<Path>,
    src_files: impl Iterator<Item = DirEntry>,
    dst: impl AsRef<Path>,
) -> anyhow::Result<()> {
    let src_base_dir = src_base_dir.as_ref();
    let dst = dst.as_ref();

    let tmp_dir = tempfile::tempdir()?;
    let tmp_dir = tmp_dir.path().to_owned();
    // to_owned() to drop tmp_dir after this function
    // drop(TempDir) do rm -rf tmp_dir by std::fs::remove_dir_all

    for file in src_files {
        let s = file.path();
        let d = &tmp_dir.join(s.strip_prefix(src_base_dir)?);
        let d_parent_dir = d.parent().unwrap();

        if !d_parent_dir.exists() {
            fs::create_dir_all(d_parent_dir)?;
        }

        if d.exists() {
            warn!("file {:?} already exists, skip", d);
            continue;
        }

        fs::hard_link(s, d)?;
    }

    // add a trailing slash to src:
    //   rsync /path/to/src/ /path/to/dst
    // to make sure /path/to/src/{file} is synced to /path/to/dst/{file}
    // instead of /path/to/dst/src/{file}
    let rsync_src_dir = &dir_path_with_tail_slash(&tmp_dir);

    let status = std::process::Command::new("rsync")
        .arg("-av")
        .arg("--delete")
        .arg(rsync_src_dir)
        .arg(dst)
        .status()?;

    assert!(status.success());

    Ok(())
}

/// dir_path_with_tail_slash converts a Path to a String,
/// and append a tailing slash to the String if it doesn't have one.
///
/// ```
/// // assert!(dir_path_with_tail_slash(Path::new("path/to/hello")) == "path/to/hello/");
/// ```
fn dir_path_with_tail_slash(dir: &Path) -> String {
    let mut dir = dir.to_str().unwrap().to_owned();

    let slash = if cfg!(windows) { "\\" } else { "/" };

    if !dir.ends_with(slash) {
        dir.push_str(slash);
    }

    dir
}

/// walkdir_iter is a wrapper of walkdir::WalkDir::new(dir).into_iter().
/// It filters out any error, log and ignore it.
fn walkdir_iter(dir: &Path) -> impl Iterator<Item = DirEntry> {
    WalkDir::new(dir).into_iter().filter_map(|e| match e {
        Ok(e) => Some(e),
        Err(e) => {
            let e = WalkDirIterError(e);
            warn!("failed to walk dir: {:?}", e);
            None
        }
    })
}

pub fn find_attachments<'a>(
    dir: impl AsRef<Path> + 'a,
    attachment_dir_re: &'a Regex,
) -> impl Iterator<Item = DirEntry> + 'a {
    walkdir_iter(dir.as_ref())
        .filter(|e| e.file_type().is_file() && is_attachment(attachment_dir_re, e))
}

/// is_attachment check all ancestors of the file
/// if any ancestor matches the regex, trait it as an attachment
fn is_attachment(attachment_dir_re: &Regex, file: &DirEntry) -> bool {
    for ancestor in file.path().ancestors() {
        let ancestor = ancestor.to_str().unwrap();
        if attachment_dir_re.is_match(ancestor) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use log::debug;
    use std::env;
    use std::sync::Once;

    static INIT: Once = Once::new();

    fn setup() -> () {
        INIT.call_once(|| {
            if let Err(_) = env::var("RUST_LOG") {
                env::set_var("RUST_LOG", "debug");
            }

            let _ = env_logger::builder().is_test(true).try_init();
            debug!(
                "tests: use env_logger with RUST_LOG={}",
                env::var("RUST_LOG").unwrap_or("".to_string())
            );
        });
    }

    #[test]
    fn test_is_markdown() {
        setup();

        let dir = Path::new("test_resc");

        let markdowns = WalkDir::new(dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(is_markdown);

        let markdowns: Vec<_> = markdowns.collect();

        assert_eq!(markdowns.len(), 5);

        assert!(markdowns
            .iter()
            .all(|e| e.file_name().to_str().unwrap().ends_with(".md")));

        let file_names = markdowns
            .iter()
            .map(|e| e.file_name().to_str().unwrap())
            .collect::<Vec<_>>();

        assert!(file_names.contains(&"has_yaml.md"));
        assert!(file_names.contains(&"missing_yaml.md"));
        assert!(file_names.contains(&"missing_key.md"));
        assert!(file_names.contains(&"bad_value_type.md"));
        assert!(file_names.contains(&"atta.md"));
    }

    #[test]
    fn test_contains_tag() {
        setup();

        let dir = Path::new("test_resc");
        let file_with_yaml = dir.join("has_yaml.md");
        let file_wo_yaml = dir.join("missing_yaml.md");
        let file_not_exist = dir.join("not_exist.md");

        assert!(contains_tag(&file_with_yaml, "publish_to", "hello-world").unwrap());
        assert!(!contains_tag(&file_wo_yaml, "publish_to", "hello-world").unwrap());

        assert!(contains_tag(&file_not_exist, "tags", "rust").is_err());
    }

    #[test]
    fn test_find_markdown_files() {
        setup();

        let dir = Path::new("test_resc");
        let files: Vec<_> = find_markdown_files(&dir).collect();

        assert_eq!(files.len(), 5);
        assert!(files.iter().any(|e| e.path().ends_with("has_yaml.md")));
        assert!(files.iter().any(|e| e.path().ends_with("missing_yaml.md")));
        assert!(files
            .iter()
            .any(|e| e.path().ends_with("sub_dir/missing_key.md")));
        assert!(files
            .iter()
            .any(|e| e.path().ends_with("sub_dir/bad_value_type.md")));
        assert!(files.iter().any(|e| e.path().ends_with("atta.md")));
    }

    #[test]
    fn test_find_markdown_files_with_tag() {
        setup();

        let dir = Path::new("test_resc");
        let files: Vec<_> =
            find_markdown_files_with_tag(&dir, "publish_to", "hello-world").collect();

        assert_eq!(files.len(), 1);
        assert!(files.iter().any(|e| e.path().ends_with("has_yaml.md")));
    }

    #[test]
    fn test_rsync_files() {
        setup();

        let src_dir = Path::new("test_resc");
        let dst_dir = tempfile::tempdir().unwrap();
        let dst_dir = dst_dir.path().to_owned();

        debug!("dst_dir: {:?}", dst_dir);

        let files = find_markdown_files_with_tag(&src_dir, "rsync_test", "expect copy");

        let files: Vec<_> = files.collect();
        debug!("src files: {:?}", files);

        let files = files.into_iter();
        let result = rsync_files(&src_dir, files, &dst_dir);
        assert!(result.is_ok());

        let mut dst_files: Vec<_> = find_markdown_files(&dst_dir).collect();

        debug!("dst_files: {:?}", dst_files);

        assert_eq!(dst_files.len(), 2);

        // check the full path of each file

        dst_files.sort_by(|a, b| a.path().cmp(&b.path()));
        let dst_files_path = dst_files
            .iter()
            .map(|e| e.path().to_owned())
            .collect::<Vec<_>>();

        let mut expected_path = vec![
            dst_dir.join("has_yaml.md"),
            dst_dir.join("sub_dir/bad_value_type.md"),
        ];

        expected_path.sort_by(|a, b| a.as_path().cmp(&b.as_path()));

        assert_eq!(dst_files_path, expected_path);
    }

    #[test]
    fn test_find_attachments() {
        setup();

        let dir = Path::new("test_resc");
        let attachment_dir_re = Regex::new(r"attachment").unwrap();
        let files: Vec<_> = find_attachments(&dir, &attachment_dir_re).collect();

        assert_eq!(files.len(), 3);
        assert!(files.iter().any(|e| e
            .path()
            .ends_with("test_resc/attachment/atta_sub_dir/atta.md")));
        assert!(files
            .iter()
            .any(|e| e.path().ends_with("test_resc/attachment/atta_root.txt")));
        assert!(files.iter().any(|e| e
            .path()
            .ends_with("test_resc/sub_dir/attachment/atta_sub.txt")));
    }
}
