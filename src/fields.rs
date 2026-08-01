use std::path::PathBuf;

use crate::mime::Mime;

/// A field stored as a file.
#[derive(Debug)]
pub struct FileField {
    /// The content type sent by the client, if any.
    pub content_type: Option<Mime>,
    /// The file name sent by the client, if any.
    ///
    /// It is not sanitized at all. Never use it as a path or a part of a path, or the client may be able to escape the intended directory.
    pub file_name:    Option<String>,
    /// The path of the temporary file which stores the data of this field.
    pub path:         PathBuf,
}

/// A field stored as a `Vec<u8>` instance.
#[derive(Debug)]
pub struct RawField {
    /// The content type sent by the client, if any.
    pub content_type: Option<Mime>,
    /// The file name sent by the client, if any.
    ///
    /// It is not sanitized at all. Never use it as a path or a part of a path, or the client may be able to escape the intended directory.
    pub file_name:    Option<String>,
    /// The data of this field.
    pub raw:          Vec<u8>,
}

/// A field stored as a string.
#[derive(Debug)]
pub struct TextField {
    /// The content type sent by the client, if any.
    pub content_type: Option<Mime>,
    /// The file name sent by the client, if any.
    ///
    /// It is not sanitized at all. Never use it as a path or a part of a path, or the client may be able to escape the intended directory.
    pub file_name:    Option<String>,
    /// The data of this field.
    pub text:         String,
}
