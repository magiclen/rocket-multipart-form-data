extern crate multipart;
extern crate rocket;

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use std::time::SystemTime;

use crate::{
    FileField, MultipartFormDataError, MultipartFormDataOptions, MultipartFormDataType, RawField,
    TextField,
};

use crate::mime::{self, Mime};

use rocket::http::ContentType;
use rocket::Data;

use multipart::server::Multipart;

/// Parsed multipart/form-data.
#[derive(Debug)]
pub struct MultipartFormData {
    pub files: HashMap<Arc<str>, Vec<FileField>>,
    pub raw: HashMap<Arc<str>, Vec<RawField>>,
    pub texts: HashMap<Arc<str>, Vec<TextField>>,
}

impl MultipartFormData {
    /// Parse multipart/form-data from the HTTP body.
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

        options.allowed_fields.sort_by_key(|e| e.field_name);

        let mut multipart = Multipart::with_body(WouldBlockFreeRead(data.open()), boundary);

        let mut files: HashMap<Arc<str>, Vec<FileField>> = HashMap::new();
        let mut raw: HashMap<Arc<str>, Vec<RawField>> = HashMap::new();
        let mut texts: HashMap<Arc<str>, Vec<TextField>> = HashMap::new();

        let mut output_err: Option<MultipartFormDataError> = None;

        'outer: while let Some(entry) = multipart.read_entry()? {
            let field_name = entry.headers.name;
            let content_type: Option<Mime> = entry.headers.content_type;

            if let Ok(vi) =
                options.allowed_fields.binary_search_by(|f| f.field_name.cmp(&field_name))
            {
                // To deal with the weird behavior of web browsers
                // If the client wants to upload an empty file, it should not set the filename to empty string.
                let mut might_be_empty_file_input_in_html = false;

                {
                    let field_ref = &options.allowed_fields[vi];

                    // The HTTP request body of an empty file input in a HTML form sent by web browsers:
                    // Content-Disposition: form-data; name="???"; filename=""
                    // Content-Type: application/octet-stream
                    if let Some(filename) = entry.headers.filename.as_ref() {
                        if filename.is_empty() {
                            // No need to check the MIME type. It's not practical.
                            might_be_empty_file_input_in_html = true;
                        }
                    }

                    // Whether to check content type
                    if let Some(content_type_ref) = &field_ref.content_type {
                        let mut mat = false; // Is the content type matching?

                        if let Some(content_type) = content_type.as_ref() {
                            let top = content_type.type_();
                            let sub = content_type.subtype();

                            for content_type_ref in content_type_ref {
                                let top_ref = content_type_ref.type_();

                                if top_ref != mime::STAR && top_ref != top {
                                    continue;
                                }

                                let sub_ref = content_type_ref.subtype();

                                if sub_ref != mime::STAR && sub_ref != sub {
                                    continue;
                                }

                                mat = true;
                                break;
                            }
                        }

                        if !mat {
                            if might_be_empty_file_input_in_html {
                                // Reserve the disciplinary action
                                output_err =
                                    Some(MultipartFormDataError::DataTypeError(field_name.clone()));
                            } else {
                                output_err =
                                    Some(MultipartFormDataError::DataTypeError(field_name));
                                break 'outer;
                            }
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
                            let target_file_name = format!(
                                "rs-{}",
                                SystemTime::now()
                                    .duration_since(SystemTime::UNIX_EPOCH)
                                    .unwrap()
                                    .as_nanos()
                            );

                            let target_path = {
                                let mut p = Path::join(&options.temporary_dir, &target_file_name);

                                let mut i = 1usize;

                                while p.exists() {
                                    p = Path::join(
                                        &options.temporary_dir,
                                        format!("{}-{}", &target_file_name, i),
                                    );

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

                            if might_be_empty_file_input_in_html {
                                if sum_c == 0 {
                                    // This file might be from an empty file input in the HTML form, so ignore it.
                                    try_delete(&target_path);

                                    output_err = None;
                                    continue;
                                } else if output_err.is_some() {
                                    try_delete(&target_path);

                                    break 'outer;
                                }
                            }

                            let file_name = entry.headers.filename;

                            let f = FileField {
                                content_type: content_type
                                    .map(|mime| Mime::from_str(&mime.to_string()).unwrap()),
                                file_name,
                                path: target_path,
                            };

                            if let Some(fields) = files.get_mut(&field_name) {
                                fields.push(f);
                            } else {
                                files.insert(field_name, vec![f]);
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

                            if might_be_empty_file_input_in_html {
                                if bytes.is_empty() {
                                    // This file might be from an empty file input in the HTML form, so ignore it.
                                    output_err = None;
                                    continue;
                                } else if output_err.is_some() {
                                    break 'outer;
                                }
                            }

                            let file_name = entry.headers.filename;

                            let f = RawField {
                                content_type: content_type
                                    .map(|mime| Mime::from_str(&mime.to_string()).unwrap()),
                                file_name,
                                raw: bytes,
                            };

                            if let Some(fields) = raw.get_mut(&field_name) {
                                fields.push(f);
                            } else {
                                raw.insert(field_name, vec![f]);
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

                            if might_be_empty_file_input_in_html {
                                if text_buffer.is_empty() {
                                    // This file might be from an empty file input in the HTML form, so ignore it.
                                    output_err = None;
                                    continue;
                                } else if output_err.is_some() {
                                    break 'outer;
                                }
                            }

                            let text = match String::from_utf8(text_buffer) {
                                Ok(s) => s,
                                Err(err) => {
                                    output_err = Some(err.into());

                                    break 'outer;
                                }
                            };

                            let file_name = entry.headers.filename;

                            let f = TextField {
                                content_type: content_type
                                    .map(|mime| Mime::from_str(&mime.to_string()).unwrap()),
                                file_name,
                                text,
                            };

                            if let Some(fields) = texts.get_mut(&field_name) {
                                fields.push(f);
                            } else {
                                texts.insert(field_name, vec![f]);
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
            for (_, fields) in files {
                for f in fields {
                    try_delete(&f.path);
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

        for fields in files.values() {
            for f in fields {
                try_delete(&f.path);
            }
        }
    }
}

#[inline]
fn try_delete<P: AsRef<Path>>(path: P) {
    if fs::remove_file(path.as_ref()).is_err() {}
}

// Work around an issue with non blocking I/O which is hard to fix all the through
// rocket + hyper-0.10. Rocket 0.5 seems not to suffer from that defect, probably due to some
// special treatment related to async I/O.
struct WouldBlockFreeRead<R: Read>(R);

const DELAY_BEFORE_WOULD_BLOCK_RETRY: std::time::Duration = std::time::Duration::from_millis(200);

impl<R: Read> Read for WouldBlockFreeRead<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        use backoff::retry;
        let res = retry(
            backoff::backoff::Constant::new(DELAY_BEFORE_WOULD_BLOCK_RETRY),
            || -> Result<usize, backoff::Error<std::io::Error>> {
                match self.0.read(buf) {
                    Ok(read) => Ok(read),
                    Err(
                        err
                        @ std::io::Error {
                            ..
                        },
                    ) if err.kind() == std::io::ErrorKind::WouldBlock => {
                        Err(backoff::Error::Transient(err))
                    }
                    Err(err) => Err(backoff::Error::Permanent(err)),
                }
            },
        );
        match res {
            Ok(res) => Ok(res),
            Err(backoff::Error::Permanent(err)) | Err(backoff::Error::Transient(err)) => Err(err),
        }
    }
}
