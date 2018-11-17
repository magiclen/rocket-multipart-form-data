/*!
# Multipart Form Data for Rocket Framework

This crate provides a multipart parser for the Rocket framework.

## Example

```rust
#![feature(plugin)]
#![plugin(rocket_codegen)]

extern crate rocket;
extern crate rocket_multipart_form_data;

use rocket::Data;
use rocket::http::ContentType;

use rocket_multipart_form_data::{mime, MultipartFormDataOptions, MultipartFormData, MultipartFormDataField, FileField, TextField, RawField};

#[post("/", data = "<data>")]
fn index(content_type: &ContentType, data: Data) -> &'static str
{
    let mut options = MultipartFormDataOptions::new();
    options.allowed_fields.push(MultipartFormDataField::file("photo").content_type_by_string(Some(mime::IMAGE_STAR)).unwrap());
    options.allowed_fields.push(MultipartFormDataField::raw("fingerprint").size_limit(4096));
    options.allowed_fields.push(MultipartFormDataField::text("name"));
    options.allowed_fields.push(MultipartFormDataField::text("array_max_length_3"));
    options.allowed_fields.push(MultipartFormDataField::text("array_max_length_3"));
    options.allowed_fields.push(MultipartFormDataField::text("array_max_length_3"));

    let multipart_form_data = MultipartFormData::parse(content_type, data, options).unwrap();

    let photo = multipart_form_data.files.get(&"photo".to_string());
    let fingerprint = multipart_form_data.raw.get(&"fingerprint".to_string());
    let name = multipart_form_data.texts.get(&"name".to_string());
    let array = multipart_form_data.texts.get(&"array_max_length_3".to_string());

    if let Some(photo) = photo {
        match photo {
            FileField::Single(file) => {
                let _content_type = &file.content_type;
                let _file_name = &file.file_name;
                let _path = &file.path;
                // You can now deal with the uploaded file. The file will be delete automatically when the MultipartFormData instance is dropped. If you want to handle that file by your own, instead of killing it, just remove it out from the MultipartFormData instance.
            }
            FileField::Multiple(_files) => {
                // Because we only put one "photo" field to the allowed_fields, this arm will not be matched.
            }
        }
    }

    if let Some(fingerprint) = fingerprint {
        match fingerprint {
            RawField::Single(raw) => {
                let _content_type = &raw.content_type;
                let _file_name = &raw.file_name;
                let _raw = &raw.raw;
                // You can now deal with the raw data.
            }
            RawField::Multiple(_bytes) => {
                // Because we only put one "fingerprint" field to the allowed_fields, this arm will not be matched.
            }
        }
    }

    if let Some(name) = name {
        match name {
            TextField::Single(text) => {
                let _content_type = &text.content_type;
                let _file_name = &text.file_name;
                let _text = &text.text;
                // You can now deal with the raw data.
            }
            TextField::Multiple(_texts) => {
                // Because we only put one "text" field to the allowed_fields, this arm will not be matched.
            }
        }
    }

    if let Some(array) = array {
        match array {
            TextField::Single(text) => {
                let _content_type = &text.content_type;
                let _file_name = &text.file_name;
                let _text = &text.text;
                // You can now deal with the text data.
            }
            TextField::Multiple(texts) => {
                // Because we put "array_max_length_3" field to the allowed_fields for three times, this arm will probably be matched.

                for text in texts { // The max length of the "texts" variable is 3
                    let _content_type = &text.content_type;
                    let _file_name = &text.file_name;
                    let _text = &text.text;
                    // You can now deal with the text data.
                }
            }
        }
    }

    "ok"
}
```

Also see `examples`.
*/

pub extern crate mime;
extern crate rocket;
extern crate multipart;
extern crate chrono;

mod multipart_form_data_type;
mod multipart_form_data_field;

pub use multipart_form_data_type::MultipartFormDataType;
pub use multipart_form_data_field::*;

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::path::{PathBuf, Path};
use std::sync::Arc;
use std::env;
use std::string;
use std::fs::{self, File};
use std::str::FromStr;

use chrono::prelude::*;

use mime::Mime;

use rocket::Data;
use rocket::http::ContentType;
use rocket::http::hyper::{self, mime::{TopLevel, SubLevel}};

use multipart::server::Multipart;

#[derive(Debug)]
pub enum MultipartFormDataError {
    NotFormDataError,
    BoundaryNotFoundError,
    IOError(io::Error),
    FromUtf8Error(string::FromUtf8Error),
    DataTooLargeError(Arc<String>),
    DataTypeError(Arc<String>),
}

/// Options for parsing multipart/form-data.
#[derive(Debug)]
pub struct MultipartFormDataOptions<'a> {
    /// A path of directory where the uploaded files will be stored.
    pub temporary_dir: PathBuf,
    /// Allowed fields of data.
    pub allowed_fields: Vec<MultipartFormDataField<'a>>,
}

impl<'a> MultipartFormDataOptions<'a> {
    /// Create a default `MultipartFormDataOptions` instance.
    pub fn new() -> MultipartFormDataOptions<'a> {
        MultipartFormDataOptions {
            temporary_dir: env::temp_dir(),
            allowed_fields: Vec::new(),
        }
    }
}

/// Parsed multipart/form-data.
#[derive(Debug)]
pub struct MultipartFormData {
    pub files: HashMap<Arc<String>, FileField>,
    pub raw: HashMap<Arc<String>, RawField>,
    pub texts: HashMap<Arc<String>, TextField>,
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

        options.allowed_fields.sort();

        let mut multipart = Multipart::with_body(data.open(), boundary);

        let mut files = HashMap::new();
        let mut raw = HashMap::new();
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
                    let content_type = entry.headers.content_type;

                    'accept: loop {
                        if let Ok(vi) = options.allowed_fields.binary_search_by(|f| f.field_name.cmp(&field_name.as_str())) {
                            {
                                let field_ref = &options.allowed_fields[vi];

                                if let Some(content_type_ref) = &field_ref.content_type { // Whether to check content type
                                    let mut mat = false; // Is the content type matching?

                                    let (top, sub) = match &content_type {
                                        Some(content_type) => {
                                            let hyper::mime::Mime(top, sub, _) = content_type;
                                            (Some(top), Some(sub))
                                        }
                                        None => (None, None)
                                    };

                                    for content_type_ref in content_type_ref {
                                        let mime = hyper::mime::Mime::from_str(content_type_ref.as_ref()).unwrap();
                                        let hyper::mime::Mime(top_ref, sub_ref, _) = mime;
                                        if top_ref.ne(&TopLevel::Star) {
                                            if let Some(top) = top {
                                                if top_ref.ne(top) {
                                                    continue;
                                                }
                                            } else {
                                                continue;
                                            }
                                        }

                                        if sub_ref.ne(&SubLevel::Star) {
                                            if let Some(sub) = sub {
                                                if sub_ref.ne(sub) {
                                                    continue;
                                                }
                                            } else {
                                                continue;
                                            }
                                        }

                                        mat = true;
                                        break;
                                    }

                                    if !mat {
                                        return Err(MultipartFormDataError::DataTypeError(field_name));
                                    }

                                    // The content type has been checked
                                }
                            }

                            let field = options.allowed_fields.remove(vi);

                            let mut data = entry.data;
                            let mut buffer = [0u8; 4096];

                            match field.typ {
                                MultipartFormDataType::File => {
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

                                        if sum_c > field.size_limit {
                                            try_delete(&target_path);
                                            return Err(MultipartFormDataError::DataTooLargeError(field_name));
                                        }

                                        file.write(&buffer[..c]).map_err(|err| {
                                            try_delete(&target_path);
                                            MultipartFormDataError::IOError(err)
                                        })?;
                                    }

                                    let file_name = entry.headers.filename;

                                    let f = SingleFileField {
                                        content_type: content_type.map(|mime| Mime::from_str(&mime.to_string()).unwrap()),
                                        file_name,
                                        path: target_path,
                                    };

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
                                MultipartFormDataType::Raw => {
                                    let mut bytes = Vec::new();

                                    loop {
                                        let c = data.read(&mut buffer).map_err(|err| MultipartFormDataError::IOError(err))?;

                                        if c == 0 {
                                            break;
                                        }

                                        if bytes.len() as u64 + c as u64 > field.size_limit {
                                            return Err(MultipartFormDataError::DataTooLargeError(field_name));
                                        }

                                        bytes.extend_from_slice(&buffer[..c]);
                                    }

                                    let file_name = entry.headers.filename;

                                    let f = SingleRawField {
                                        content_type: content_type.map(|mime| Mime::from_str(&mime.to_string()).unwrap()),
                                        file_name,
                                        raw: bytes,
                                    };

                                    match raw.remove(&field_name) {
                                        Some(field) => {
                                            match field {
                                                RawField::Single(t) => {
                                                    let v = vec![t, f];
                                                    raw.insert(field_name, RawField::Multiple(v));
                                                }
                                                RawField::Multiple(mut v) => {
                                                    v.push(f);
                                                    raw.insert(field_name, RawField::Multiple(v));
                                                }
                                            }
                                        }
                                        None => {
                                            raw.insert(field_name, RawField::Single(f));
                                        }
                                    }
                                }
                                MultipartFormDataType::Text => {
                                    let mut text_buffer = Vec::new();

                                    loop {
                                        let c = data.read(&mut buffer).map_err(|err| MultipartFormDataError::IOError(err))?;

                                        if c == 0 {
                                            break;
                                        }

                                        if text_buffer.len() as u64 + c as u64 > field.size_limit {
                                            return Err(MultipartFormDataError::DataTooLargeError(field_name));
                                        }

                                        text_buffer.extend_from_slice(&buffer[..c]);
                                    }

                                    let text = String::from_utf8(text_buffer).map_err(|err| MultipartFormDataError::FromUtf8Error(err))?;

                                    let file_name = entry.headers.filename;

                                    let f = SingleTextField {
                                        content_type: content_type.map(|mime| Mime::from_str(&mime.to_string()).unwrap()),
                                        file_name,
                                        text,
                                    };

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
                                }
                            }

                            break 'accept;
                        } else {
                            break 'accept;
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
            raw,
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