extern crate rocket;
extern crate tokio_util;

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;

use crate::{
    FileField, MultipartFormDataError, MultipartFormDataOptions, MultipartFormDataType, RawField,
    TextField,
};

use crate::mime;

use rocket::http::ContentType;
use rocket::tokio::fs::File;
use rocket::tokio::io::AsyncWriteExt;
use rocket::Data;

use crate::multer::Multipart;

/// Parsed multipart/form-data.
#[derive(Debug)]
pub struct MultipartFormData {
    pub files: HashMap<Arc<str>, Vec<FileField>>,
    pub raw: HashMap<Arc<str>, Vec<RawField>>,
    pub texts: HashMap<Arc<str>, Vec<TextField>>,
}

impl MultipartFormData {
    /// Parse multipart/form-data from the HTTP body.
    pub async fn parse(
        content_type: &ContentType,
        data: Data,
        mut options: MultipartFormDataOptions<'_>,
    ) -> Result<MultipartFormData, MultipartFormDataError> {
        if !content_type.is_form_data() {
            return Err(MultipartFormDataError::NotFormDataError);
        }

        let (_, boundary) = match content_type.params().find(|&(k, _)| k == "boundary") {
            Some(s) => s,
            None => return Err(MultipartFormDataError::BoundaryNotFoundError),
        };

        options.allowed_fields.sort_by_key(|e| e.field_name);

        let stream = data.open(options.max_data_bytes.into());

        let mut multipart = Multipart::new(tokio_util::io::ReaderStream::new(stream), boundary);

        let mut files: HashMap<Arc<str>, Vec<FileField>> = HashMap::new();
        let mut raw: HashMap<Arc<str>, Vec<RawField>> = HashMap::new();
        let mut texts: HashMap<Arc<str>, Vec<TextField>> = HashMap::new();

        let mut output_err: Option<MultipartFormDataError> = None;

        'outer: while let Some(mut entry) = multipart.next_field().await? {
            let field_name = match entry.name() {
                Some(name) => Arc::from(name),
                None => continue,
            };

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
                    if let Some(filename) = entry.file_name() {
                        if filename.is_empty() {
                            // No need to check the MIME type. It's not practical.
                            might_be_empty_file_input_in_html = true;
                        }
                    }

                    // Whether to check content type
                    if let Some(content_type_ref) = &field_ref.content_type {
                        let mut mat = false; // Is the content type matching?

                        if let Some(content_type) = entry.content_type().as_ref() {
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
                    let field = unsafe { options.allowed_fields.get_unchecked_mut(vi) };

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

                            let mut file = match File::create(&target_path).await {
                                Ok(f) => f,
                                Err(err) => {
                                    output_err = Some(err.into());

                                    break 'outer;
                                }
                            };

                            let mut sum_c = 0u64;

                            loop {
                                match entry.chunk().await {
                                    Ok(bytes) => {
                                        match bytes {
                                            Some(bytes) => {
                                                sum_c += bytes.len() as u64;

                                                if sum_c > field.size_limit {
                                                    try_delete(&target_path);

                                                    output_err = Some(
                                                        MultipartFormDataError::DataTooLargeError(
                                                            field_name,
                                                        ),
                                                    );

                                                    break 'outer;
                                                }

                                                match file.write_all(bytes.as_ref()).await {
                                                    Ok(_) => (),
                                                    Err(err) => {
                                                        try_delete(&target_path);

                                                        output_err = Some(err.into());

                                                        break 'outer;
                                                    }
                                                }
                                            }
                                            None => break,
                                        }
                                    }
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

                            let file_name = entry.file_name().map(String::from);

                            let f = FileField {
                                content_type: entry.content_type().cloned(),
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
                            let mut raw_buffer = Vec::new();

                            loop {
                                match entry.chunk().await {
                                    Ok(bytes) => {
                                        match bytes {
                                            Some(bytes) => {
                                                if raw_buffer.len() as u64 + bytes.len() as u64
                                                    > field.size_limit
                                                {
                                                    output_err = Some(
                                                        MultipartFormDataError::DataTooLargeError(
                                                            field_name,
                                                        ),
                                                    );

                                                    break 'outer;
                                                }

                                                raw_buffer.extend_from_slice(bytes.as_ref());
                                            }
                                            None => break,
                                        }
                                    }
                                    Err(err) => {
                                        output_err = Some(err.into());

                                        break 'outer;
                                    }
                                }
                            }

                            if might_be_empty_file_input_in_html {
                                if raw_buffer.is_empty() {
                                    // This file might be from an empty file input in the HTML form, so ignore it.
                                    output_err = None;
                                    continue;
                                } else if output_err.is_some() {
                                    break 'outer;
                                }
                            }

                            let file_name = entry.file_name().map(String::from);

                            let f = RawField {
                                content_type: entry.content_type().cloned(),
                                file_name,
                                raw: raw_buffer,
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
                                match entry.chunk().await {
                                    Ok(bytes) => {
                                        match bytes {
                                            Some(bytes) => {
                                                if text_buffer.len() as u64 + bytes.len() as u64
                                                    > field.size_limit
                                                {
                                                    output_err = Some(
                                                        MultipartFormDataError::DataTooLargeError(
                                                            field_name,
                                                        ),
                                                    );

                                                    break 'outer;
                                                }

                                                text_buffer.extend_from_slice(bytes.as_ref());
                                            }
                                            None => break,
                                        }
                                    }
                                    Err(err) => {
                                        output_err = Some(err.into());

                                        break 'outer;
                                    }
                                }
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

                            let file_name = entry.file_name().map(String::from);

                            let f = TextField {
                                content_type: entry.content_type().cloned(),
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
                if multipart.next_field().await?.is_none() {
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
