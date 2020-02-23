/*!
# Multipart Form Data for Rocket Framework

This crate provides a multipart parser for the Rocket framework.

## Example

```rust
#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use] extern crate rocket;
extern crate rocket_multipart_form_data;

use rocket::Data;
use rocket::http::ContentType;

use rocket_multipart_form_data::{mime, MultipartFormDataOptions, MultipartFormData, MultipartFormDataField, Repetition, FileField, TextField, RawField};

#[post("/", data = "<data>")]
fn index(content_type: &ContentType, data: Data) -> &'static str
{
    let mut options = MultipartFormDataOptions::new();
    options.allowed_fields.push(MultipartFormDataField::file("photo").content_type_by_string(Some(mime::IMAGE_STAR)).unwrap());
    options.allowed_fields.push(MultipartFormDataField::raw("fingerprint").size_limit(4096));
    options.allowed_fields.push(MultipartFormDataField::text("name"));
    options.allowed_fields.push(MultipartFormDataField::text("array_max_length_3").repetition(Repetition::fixed(3)));

    let multipart_form_data = MultipartFormData::parse(content_type, data, options).unwrap();

    let photo = multipart_form_data.files.get("photo");
    let fingerprint = multipart_form_data.raw.get("fingerprint");
    let name = multipart_form_data.texts.get("name");
    let array = multipart_form_data.texts.get("array_max_length_3");

    if let Some(photo) = photo {
        match photo {
            FileField::Single(file) => {
                let _content_type = &file.content_type;
                let _file_name = &file.file_name;
                let _path = &file.path;
                // You can now deal with the uploaded file. The file will be deleted automatically when the MultipartFormData instance is dropped. If you want to handle that file by your own, instead of killing it, just remove it out from the MultipartFormData instance.
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

extern crate chrono;
pub extern crate mime;
extern crate multipart;
extern crate rocket;

mod multipart_form_data_field;
mod multipart_form_data_type;

pub use multipart_form_data_field::*;
pub use multipart_form_data_type::MultipartFormDataType;

use std::collections::HashMap;
use std::env;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::string;
use std::sync::Arc;

use chrono::prelude::*;

use mime::Mime;

use rocket::http::hyper::{
    self,
    mime::{SubLevel, TopLevel},
};
use rocket::http::ContentType;
use rocket::Data;

use multipart::server::Multipart;

#[derive(Debug)]
pub enum MultipartFormDataError {
    NotFormDataError,
    BoundaryNotFoundError,
    IOError(io::Error),
    FromUtf8Error(string::FromUtf8Error),
    DataTooLargeError(Arc<str>),
    DataTypeError(Arc<str>),
}

impl From<io::Error> for MultipartFormDataError {
    #[inline]
    fn from(err: io::Error) -> MultipartFormDataError {
        MultipartFormDataError::IOError(err)
    }
}

impl From<string::FromUtf8Error> for MultipartFormDataError {
    #[inline]
    fn from(err: string::FromUtf8Error) -> MultipartFormDataError {
        MultipartFormDataError::FromUtf8Error(err)
    }
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
    #[inline]
    pub fn new() -> MultipartFormDataOptions<'a> {
        MultipartFormDataOptions {
            temporary_dir: env::temp_dir(),
            allowed_fields: Vec::new(),
        }
    }
}

impl<'a> Default for MultipartFormDataOptions<'a> {
    #[inline]
    fn default() -> Self {
        MultipartFormDataOptions::new()
    }
}

/// Parsed multipart/form-data.
#[derive(Debug)]
pub struct MultipartFormData {
    pub files: HashMap<Arc<str>, FileField>,
    pub raw: HashMap<Arc<str>, RawField>,
    pub texts: HashMap<Arc<str>, TextField>,
}

impl MultipartFormData {
    /// Parse multipart/form-data from the HTTP body.
    #[allow(clippy::cognitive_complexity)]
    pub fn parse(
        content_type: &ContentType,
        data: Data,
        mut options: MultipartFormDataOptions,
    ) -> Result<MultipartFormData, MultipartFormDataError> {
        if !content_type.is_form_data() {
            return Err(MultipartFormDataError::NotFormDataError);
        }

        let (_, boundary) = match content_type.params().find(|&(k, _)| k == "boundary") {
            Some(s) => s,
            None => return Err(MultipartFormDataError::BoundaryNotFoundError),
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
                    return Err(MultipartFormDataError::IOError(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        "the temporary path exists and it is not a directory",
                    )));
                }
            } else {
                fs::create_dir_all(path)?;
            }
        }

        let mut output_err: Option<MultipartFormDataError> = None;

        'outer: while let Some(entry) = multipart.read_entry()? {
            let field_name = entry.headers.name;
            let content_type = entry.headers.content_type;

            if let Ok(vi) =
                options.allowed_fields.binary_search_by(|f| f.field_name.cmp(&field_name))
            {
                {
                    let field_ref = &options.allowed_fields[vi];

                    // Whether to check content type
                    if let Some(content_type_ref) = &field_ref.content_type {
                        let mut mat = false; // Is the content type matching?

                        let (top, sub) = match &content_type {
                            Some(content_type) => {
                                let hyper::mime::Mime(top, sub, _) = content_type;
                                (Some(top), Some(sub))
                            }
                            None => (None, None),
                        };

                        for content_type_ref in content_type_ref {
                            let mime =
                                hyper::mime::Mime::from_str(content_type_ref.as_ref()).unwrap();
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
                            output_err = Some(MultipartFormDataError::DataTypeError(field_name));

                            break 'outer;
                        }

                        // The content type has been checked
                    }
                }

                let drop_field = {
                    let mut buffer = [0u8; 4096];

                    let field = unsafe { options.allowed_fields.get_unchecked_mut(vi) };

                    let mut data = entry.data;

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
                                        Path::join(
                                            &options.temporary_dir,
                                            format!("{}-{}", &target_file_name, i),
                                        )
                                    };

                                    if !p.exists() {
                                        break;
                                    }

                                    i += 1;
                                }

                                p
                            };

                            let mut file = match File::create(&target_path) {
                                Ok(f) => f,
                                Err(err) => {
                                    output_err = Some(err.into());

                                    break 'outer;
                                }
                            };

                            let mut sum_c = 0u64;

                            loop {
                                let c = match data.read(&mut buffer) {
                                    Ok(c) => c,
                                    Err(err) => {
                                        try_delete(&target_path);

                                        output_err = Some(err.into());

                                        break 'outer;
                                    }
                                };

                                if c == 0 {
                                    break;
                                }

                                sum_c += c as u64;

                                if sum_c > field.size_limit {
                                    try_delete(&target_path);

                                    output_err =
                                        Some(MultipartFormDataError::DataTooLargeError(field_name));

                                    break 'outer;
                                }

                                match file.write(&buffer[..c]) {
                                    Ok(_) => (),
                                    Err(err) => {
                                        try_delete(&target_path);

                                        output_err = Some(err.into());

                                        break 'outer;
                                    }
                                }
                            }

                            let file_name = entry.headers.filename;

                            let f = SingleFileField {
                                content_type: content_type
                                    .map(|mime| Mime::from_str(&mime.to_string()).unwrap()),
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
                                let c = match data.read(&mut buffer) {
                                    Ok(c) => c,
                                    Err(err) => {
                                        output_err = Some(err.into());

                                        break 'outer;
                                    }
                                };

                                if c == 0 {
                                    break;
                                }

                                if bytes.len() as u64 + c as u64 > field.size_limit {
                                    output_err =
                                        Some(MultipartFormDataError::DataTooLargeError(field_name));

                                    break 'outer;
                                }

                                bytes.extend_from_slice(&buffer[..c]);
                            }

                            let file_name = entry.headers.filename;

                            let f = SingleRawField {
                                content_type: content_type
                                    .map(|mime| Mime::from_str(&mime.to_string()).unwrap()),
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
                                let c = match data.read(&mut buffer) {
                                    Ok(c) => c,
                                    Err(err) => {
                                        output_err = Some(err.into());

                                        break 'outer;
                                    }
                                };

                                if c == 0 {
                                    break;
                                }

                                if text_buffer.len() as u64 + c as u64 > field.size_limit {
                                    output_err =
                                        Some(MultipartFormDataError::DataTooLargeError(field_name));

                                    break 'outer;
                                }

                                text_buffer.extend_from_slice(&buffer[..c]);
                            }

                            let text = match String::from_utf8(text_buffer) {
                                Ok(s) => s,
                                Err(err) => {
                                    output_err = Some(err.into());

                                    break 'outer;
                                }
                            };

                            let file_name = entry.headers.filename;

                            let f = SingleTextField {
                                content_type: content_type
                                    .map(|mime| Mime::from_str(&mime.to_string()).unwrap()),
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

                    field.repetition.decrease_check_is_over()
                };

                if drop_field {
                    options.allowed_fields.remove(vi);
                }
            }
        }

        if let Some(err) = output_err {
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

            loop {
                if multipart.read_entry()?.is_none() {
                    break;
                }
            }

            Err(err)
        } else {
            Ok(MultipartFormData {
                files,
                raw,
                texts,
            })
        }
    }
}

impl Drop for MultipartFormData {
    #[inline]
    fn drop(&mut self) {
        let files = &self.files;

        for field in files.values() {
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

#[inline]
fn try_delete<P: AsRef<Path>>(path: P) {
    if fs::remove_file(path.as_ref()).is_err() {}
}
