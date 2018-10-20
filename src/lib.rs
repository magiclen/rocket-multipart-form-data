//! # Multipart Form Data for Rocket Framework
//! This crate provides a multipart parser.

extern crate rocket;
extern crate multipart;
extern crate tempdir;

use std::collections::HashMap;
use std::io;
use std::path::{PathBuf, Path};
use std::sync::Arc;

use rocket::Data;
use rocket::http::ContentType;
pub use rocket::http::hyper::mime::Mime;

use multipart::server::Multipart;
use multipart::server::save::{SaveBuilder, SavedData, SaveResult, PartialReason, SavedField};

use tempdir::TempDir;

/// Parsed multipart/form-data.
#[derive(Debug)]
pub struct MultipartFormData {
    pub partial: Option<PartialReason>,
    pub files: HashMap<Arc<String>, FileField>,
    pub body: HashMap<Arc<String>, TextField>,
}

#[derive(Debug)]
pub struct SingleFileField {
    pub content_type: Option<Mime>,
    pub path: PathBuf,
}

#[derive(Debug)]
pub enum FileField {
    Single(SingleFileField),
    Multiple(Vec<SingleFileField>),
}

#[derive(Debug)]
pub struct SingleTextField {
    pub content_type: Option<Mime>,
    pub text: String,
}

#[derive(Debug)]
pub enum TextField {
    Single(SingleTextField),
    Multiple(Vec<SingleTextField>),
}

#[derive(Debug)]
pub enum MultipartFormDataError {
    NotFormDataError,
    BoundaryNotFoundError,
    BodySizeTooLargeError,
    IOError(io::Error),
    FieldTypeAmbiguousError,
}

impl MultipartFormData {
    /// Parse multipart/form-data from the HTTP body.
    pub fn parse<P: AsRef<Path>>(content_type: &ContentType, data: Data, size_limit: u64, temporary_dir: P) -> Result<MultipartFormData, MultipartFormDataError> {
        if !content_type.is_form_data() {
            return Err(MultipartFormDataError::NotFormDataError);
        }

        let (_, boundary) = match content_type.params().find(|&(k, _)| k == "boundary") {
            Some(s) => s,
            None => return Err(MultipartFormDataError::BoundaryNotFoundError)
        };

        let multipart = Multipart::with_body(data.open(), boundary);

        let save_builder = SaveBuilder::new(multipart).size_limit(size_limit).count_limit(None).memory_threshold(0).force_text();

        let temp = save_builder.with_temp_dir(TempDir::new_in(temporary_dir, "rocket-multipart").map_err(|err| MultipartFormDataError::IOError(err))?);

        match temp {
            SaveResult::Full(entries) => {
                let (files, body) = parse_fields(entries.fields)?;

                Ok(MultipartFormData {
                    partial: None,
                    files,
                    body,
                })
            }
            SaveResult::Partial(partial, reason) => {
                let (files, body) = parse_fields(partial.entries.fields)?;

                Ok(MultipartFormData {
                    partial: Some(reason),
                    files,
                    body,
                })
            }
            SaveResult::Error(err) => Err(MultipartFormDataError::IOError(err))
        }
    }
}

fn parse_fields(fields: HashMap<Arc<String>, Vec<SavedField>>) -> Result<(HashMap<Arc<String>, FileField>, HashMap<Arc<String>, TextField>), MultipartFormDataError> {
    let mut files = HashMap::new();
    let mut body = HashMap::new();

    for (key, mut value) in fields {
        let len = value.len();

        if len > 0 {
            if len > 1 {
                let mut file_array = Vec::new();
                let mut text_array = Vec::new();

                for value in value {
                    let content_type = value.headers.content_type;

                    let data = value.data;

                    match data {
                        SavedData::File(path, _) => {
                            if !text_array.is_empty() {
                                return Err(MultipartFormDataError::FieldTypeAmbiguousError);
                            }
                            file_array.push(SingleFileField {
                                content_type,
                                path,
                            });
                        }
                        SavedData::Text(text) => {
                            if !file_array.is_empty() {
                                return Err(MultipartFormDataError::FieldTypeAmbiguousError);
                            }
                            text_array.push(SingleTextField {
                                content_type,
                                text,
                            });
                        }
                        _ => ()
                    }
                }

                if !file_array.is_empty() {
                    files.insert(key, FileField::Multiple(file_array));
                } else if !text_array.is_empty() {
                    body.insert(key, TextField::Multiple(text_array));
                }
            } else {
                let value = value.remove(0);

                let content_type = value.headers.content_type;

                let data = value.data;

                match data {
                    SavedData::File(path, _) => {
                        files.insert(key, FileField::Single(SingleFileField {
                            content_type,
                            path,
                        }));
                    }
                    SavedData::Text(text) => {
                        body.insert(key, TextField::Single(SingleTextField {
                            content_type,
                            text,
                        }));
                    }
                    _ => ()
                }
            }
        }
    }

    Ok((files, body))
}