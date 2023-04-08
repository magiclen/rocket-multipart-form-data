use std::path::PathBuf;

use crate::mime::Mime;

#[derive(Debug)]
pub struct FileField {
    pub content_type: Option<Mime>,
    pub file_name:    Option<String>,
    pub path:         PathBuf,
}

#[derive(Debug)]
pub struct RawField {
    pub content_type: Option<Mime>,
    pub file_name:    Option<String>,
    pub raw:          Vec<u8>,
}

#[derive(Debug)]
pub struct TextField {
    pub content_type: Option<Mime>,
    pub file_name:    Option<String>,
    pub text:         String,
}
