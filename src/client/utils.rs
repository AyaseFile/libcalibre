use chrono::DateTime;
use chrono::Utc;
use deunicode::deunicode;
use sanitise_file_name::sanitise;

use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use crate::entities::book::UpdateBookData;
use crate::util::ValidDbPath;
use crate::Author;

pub fn combined_author_sort(author_list: &Vec<Author>) -> String {
    author_list
        .iter()
        .map(|author| author.sortable_name())
        .collect::<Vec<String>>()
        .join(" & ")
}

pub fn default_pubdate() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("0101-01-01T00:00:00+00:00")
        .unwrap()
        .with_timezone(&Utc)
}

pub fn update_book_data_for_path(path: &PathBuf) -> UpdateBookData {
    let path_as_string = path.to_str().unwrap().to_string();
    UpdateBookData {
        author_sort: None,
        title: None,
        timestamp: None,
        pubdate: None,
        series_index: None,
        path: Some(path_as_string),
        flags: None,
        has_cover: None,
        last_modified: None,
    }
}

pub fn gen_book_file_name(book_title: &String, author_name: &String) -> String {
    sanitise(&deunicode(
        &"{title} - {author}"
            .replace("{title}", book_title)
            .replace("{author}", author_name),
    ))
}

pub fn gen_book_folder_name(book_id: i32) -> String {
    book_id.to_string()
}

/// Create a new directory at a library-relative path.
/// Convenience function to avoid having absolute paths for files everywhere.
pub fn library_relative_mkdir(valid_db_path: &ValidDbPath, rel_path: PathBuf) -> io::Result<()> {
    let complete_path = Path::new(&valid_db_path.library_path).join(rel_path);

    match complete_path.exists() {
        true => Ok(()),
        _ => fs::create_dir_all(complete_path),
    }
}

/// Copy a file from an absolute path to a library-relative path, using a ValidDbPath.
/// Convenience function to avoid having to create the abs. path yourself.
pub fn library_relative_copy_file(
    valid_db_path: &ValidDbPath,
    source_abs: &Path,
    dest_rel: &Path,
) -> io::Result<()> {
    match source_abs.exists() {
        true => {
            let complete_dest_path = Path::new(&valid_db_path.library_path).join(dest_rel);
            fs::copy(source_abs, complete_dest_path).map(|_| ())
        }
        false => Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Source file does not exist: {:?}", source_abs),
        )),
    }
}

/// Write `contents` to a library-relative file path.
/// Convenience function to avoid having to create the absolute path.
pub fn library_relative_write_file(
    valid_db_path: &ValidDbPath,
    rel_path: &Path,
    contents: &[u8],
) -> io::Result<()> {
    let complete_path = Path::new(&valid_db_path.library_path).join(rel_path);
    fs::write(complete_path, contents)
}
