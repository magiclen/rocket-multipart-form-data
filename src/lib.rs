//! # Multipart Form Data for Rocket Framework
//! This crate provides a multipart parser for the Rocket framework.
//!
//! ## Examples
//!
//! ```
//! #![feature(plugin)]
//! #![plugin(rocket_codegen)]
//!
//! extern crate rocket;
//! extern crate rocket_multipart_form_data;
//!
//! use rocket::Data;
//! use rocket::http::ContentType;
//!
//! use rocket_multipart_form_data::{MultipartFormDataOptions, MultipartFormData};
//!
//! #[post("/", data = "<data>")]
//! fn index(content_type: &ContentType, data: Data) -> &'static str
//! {
//!     let mut options = MultipartFormDataOptions::new();
//!     options.allowed_file_fields.push("photo");
//!     options.allowed_text_fields.push("name");
//!     options.allowed_text_fields.push("array_max_length_3");
//!     options.allowed_text_fields.push("array_max_length_3");
//!     options.allowed_text_fields.push("array_max_length_3");
//!
//!     let multipart_form_data = MultipartFormData::parse(content_type, data, options).unwrap();
//!
//!     let photo = multipart_form_data.files.get(&"photo".to_string());
//!     let name = multipart_form_data.texts.get(&"name".to_string());
//!     let array = multipart_form_data.texts.get(&"array_max_length_3".to_string());
//!
//!     println!("name = {:?}", name);
//!     println!("photo = {:?}", photo);
//!     println!("array = {:?}", array);
//!
//!     "ok"
//! }
//! ```

extern crate rocket;
extern crate multipart;
extern crate chrono;

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::path::{PathBuf, Path};
use std::sync::Arc;
use std::env;
use std::string;
use std::fs::{self, File};

use chrono::prelude::*;

use rocket::Data;
use rocket::http::ContentType;
pub use rocket::http::hyper::mime::{Mime, TopLevel};

use multipart::server::Multipart;

const BUFFER_SIZE: usize = 4096;

/// Options for parsing multipart/form-data.
#[derive(Debug)]
pub struct MultipartFormDataOptions<'a> {
    /// The size limit for each of in-memory text strings. The default value is 1 MiB.
    pub text_size_limit: u64,
    /// The size limit for each of files. The default value is 8 MiB.
    pub file_size_limit: u64,
    /// The directory where saving the parsed temporary files. The temporary files will be delete when its owner instance of the `MultipartFormData` struct is dropped. The default value is the path of a temporary directory from the runtime environment.
    pub temporary_dir: PathBuf,
    /// Allowed fields of text data.
    pub allowed_text_fields: Vec<&'a str>,
    /// Allowed fields of file data.
    pub allowed_file_fields: Vec<&'a str>,
}

/// Parsed multipart/form-data.
#[derive(Debug)]
pub struct MultipartFormData {
    pub files: HashMap<Arc<String>, FileField>,
    pub texts: HashMap<Arc<String>, TextField>,
}

#[derive(Debug)]
pub struct SingleFileField {
    pub content_type: Option<Mime>,
    pub file_name: Option<String>,
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
    pub file_name: Option<String>,
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
    FromUtf8Error(string::FromUtf8Error),
    DataTooLargeError(Arc<String>),
}

impl<'a> MultipartFormDataOptions<'a> {
    /// Create a default `MultipartFormDataOptions` instance.
    pub fn new() -> MultipartFormDataOptions<'a> {
        MultipartFormDataOptions {
            text_size_limit: 1 * 1024 * 1024,
            file_size_limit: 8 * 1024 * 1024,
            temporary_dir: env::temp_dir(),
            allowed_text_fields: Vec::new(),
            allowed_file_fields: Vec::new(),
        }
    }
}

impl MultipartFormData {
    /// Parse multipart/form-data from the HTTP body.
    pub fn parse(content_type: &ContentType, data: Data, mut options: MultipartFormDataOptions) -> Result<MultipartFormData, MultipartFormDataError> {
        if !content_type.is_form_data() {
            return Err(MultipartFormDataError::NotFormDataError);
        }

        let (_, boundary) = match content_type.params().find(|&(k, _)| k == "boundary") {
            Some(s) => s,
            None => return Err(MultipartFormDataError::BoundaryNotFoundError)
        };

        options.allowed_file_fields.sort();
        options.allowed_text_fields.sort();

        let mut multipart = Multipart::with_body(data.open(), boundary);

        let mut files = HashMap::new();
        let mut texts = HashMap::new();

        if !files.is_empty() {
            let path = options.temporary_dir.as_path();

            if path.exists() {
                if !path.is_dir() {
                    return Err(MultipartFormDataError::IOError(io::Error::new(io::ErrorKind::AlreadyExists, "the temporary path exists and it is not a directory")));
                }
            } else {
                fs::create_dir_all(path).map_err(|err| MultipartFormDataError::IOError(err))?;
            }
        }

        loop {
            match multipart.read_entry().map_err(|err| MultipartFormDataError::IOError(err))? {
                Some(entry) => {
                    let field_name = entry.headers.name;

                    if let Ok(vi) = options.allowed_text_fields.binary_search(&field_name.as_str()) {
                        let mut data = entry.data;
                        let mut buffer = [0u8; BUFFER_SIZE];

                        let mut text_buffer = Vec::new();

                        loop {
                            let c = data.read(&mut buffer).map_err(|err| MultipartFormDataError::IOError(err))?;

                            if c == 0 {
                                break;
                            }

                            if text_buffer.len() as u64 + c as u64 > options.text_size_limit {
                                return Err(MultipartFormDataError::DataTooLargeError(field_name));
                            }

                            text_buffer.extend_from_slice(&buffer[..c]);
                        }

                        let text = String::from_utf8(text_buffer).map_err(|err| MultipartFormDataError::FromUtf8Error(err))?;

                        let content_type = entry.headers.content_type;
                        let file_name = entry.headers.filename;

                        let f = SingleTextField {
                            content_type,
                            file_name,
                            text,
                        };

                        options.allowed_text_fields.remove(vi);

                        match texts.remove(&field_name) {
                            Some(field) => {
                                match field {
                                    TextField::Single(t) => {
                                        let v = vec![t, f];
                                        texts.insert(field_name, TextField::Multiple(v));
                                    }
                                    TextField::Multiple(mut v) => {
                                        v.push(f);
                                        texts.insert(field_name, TextField::Multiple(v));
                                    }
                                }
                            }
                            None => {
                                texts.insert(field_name, TextField::Single(f));
                            }
                        }
                    } else if let Ok(vi) = options.allowed_file_fields.binary_search(&field_name.as_str()) {
                        let mut data = entry.data;
                        let mut buffer = [0u8; BUFFER_SIZE];

                        let now = Utc::now();

                        let target_file_name = format!("rs-{}", now.timestamp_nanos());

                        let target_path = {
                            let mut i = 0usize;

                            let mut p;

                            loop {
                                p = if i == 0 {
                                    Path::join(&options.temporary_dir, &target_file_name)
                                } else {
                                    Path::join(&options.temporary_dir, format!("{}-{}", &target_file_name, i))
                                };

                                if !p.exists() {
                                    break;
                                }

                                i += 1;
                            }

                            p
                        };

                        let mut file = File::create(&target_path).map_err(|err| MultipartFormDataError::IOError(err))?;

                        let mut sum_c = 0u64;

                        loop {
                            let c = data.read(&mut buffer).map_err(|err| {
                                try_delete(&target_path);
                                MultipartFormDataError::IOError(err)
                            })?;

                            if c == 0 {
                                break;
                            }

                            sum_c += c as u64;

                            if sum_c > options.file_size_limit {
                                try_delete(&target_path);
                                return Err(MultipartFormDataError::DataTooLargeError(field_name));
                            }

                            file.write(&buffer[..c]).map_err(|err| {
                                try_delete(&target_path);
                                MultipartFormDataError::IOError(err)
                            })?;
                        }

                        let content_type = entry.headers.content_type;
                        let file_name = entry.headers.filename;

                        let f = SingleFileField {
                            content_type,
                            file_name,
                            path: target_path,
                        };

                        options.allowed_text_fields.remove(vi);

                        match files.remove(&field_name) {
                            Some(field) => {
                                match field {
                                    FileField::Single(t) => {
                                        let v = vec![t, f];
                                        files.insert(field_name, FileField::Multiple(v));
                                    }
                                    FileField::Multiple(mut v) => {
                                        v.push(f);
                                        files.insert(field_name, FileField::Multiple(v));
                                    }
                                }
                            }
                            None => {
                                files.insert(field_name, FileField::Single(f));
                            }
                        }
                    }
                }
                None => {
                    break;
                }
            }
        }

        Ok(MultipartFormData {
            files,
            texts,
        })
    }
}

impl Drop for MultipartFormData {
    fn drop(&mut self) {
        let files = &self.files;

        for (_, field) in files {
            match field {
                FileField::Single(f) => {
                    try_delete(&f.path);
                }
                FileField::Multiple(vf) => {
                    for f in vf {
                        try_delete(&f.path);
                    }
                }
            }
        }
    }
}

fn try_delete<P: AsRef<Path>>(path: P) {
    if let Err(_) = fs::remove_file(path.as_ref()) {}
}