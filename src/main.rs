use regex::Regex;
use smelt::{find_attachments, find_markdown_files_with_tag, print_files, rsync_files};

fn main() {
    // TODO: use clap to parse command line arguments

    let src_dir = "test_resc";
    let key = "publish_to";
    let value = "hello-world";
    let include_attachment_dir = "attachment";
    let dst_dir = "publish_to_hello-world";

    let files = find_markdown_files_with_tag(src_dir, key, value);

    let attachment_dir_re = Regex::new(include_attachment_dir).unwrap();
    let attachment_files = find_attachments(src_dir, &attachment_dir_re);

    let files = files.chain(attachment_files);

    // print_files(files);

    let result = rsync_files(src_dir, files, dst_dir);
    match result {
        Ok(_) => println!("rsync success"),
        Err(e) => println!("rsync failed: {}", e),
    }
}
