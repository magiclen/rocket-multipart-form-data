use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
    sync::Arc,
    time::SystemTime,
};

use rocket::{
    Data,
    http::ContentType,
    tokio::{fs::File, io::AsyncWriteExt, task},
};

use crate::{
    FileField, MultipartFormDataError, MultipartFormDataOptions, MultipartFormDataType, RawField,
    TextField, mime,
    multer::{Constraints, Multipart, SizeLimit},
};

/// Parsed multipart/form-data.
#[derive(Debug)]
pub struct MultipartFormData {
    pub files: HashMap<Arc<str>, Vec<FileField>>,
    pub raw:   HashMap<Arc<str>, Vec<RawField>>,
    pub texts: HashMap<Arc<str>, Vec<TextField>>,
}

impl MultipartFormData {
    /// Parse multipart/form-data from the HTTP body.
    pub async fn parse(
        content_type: &ContentType,
        data: Data<'_>,
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

        let stream = data.open(options.max_data_bytes.saturating_add(1).into());
        let constraints =
            Constraints::new().size_limit(SizeLimit::new().whole_stream(options.max_data_bytes));

        let mut multipart = Multipart::with_constraints(
            tokio_util::io::ReaderStream::new(stream),
            boundary,
            constraints,
        );

        let mut files: HashMap<Arc<str>, Vec<FileField>> = HashMap::new();
        let mut raw: HashMap<Arc<str>, Vec<RawField>> = HashMap::new();
        let mut texts: HashMap<Arc<str>, Vec<TextField>> = HashMap::new();

        let mut output_err: Option<MultipartFormDataError> = None;

        'outer: loop {
            let mut entry = match multipart.next_field().await {
                Ok(Some(entry)) => entry,
                Ok(None) => break,
                Err(err) => {
                    // Do not return here, or the temporary files created so far would be left behind.
                    output_err = Some(err.into());

                    break;
                },
            };

            // Search by `&str` first so that a disallowed field does not allocate an `Arc`.
            let (vi, field_name) = match entry.name() {
                Some(name) => {
                    match options.allowed_fields.binary_search_by(|f| f.field_name.cmp(name)) {
                        Ok(vi) => (vi, Arc::<str>::from(name)),
                        Err(_) => continue,
                    }
                },
                None => continue,
            };

            // To deal with the weird behavior of web browsers
            // If the client wants to upload an empty file, it should not set the filename to empty string.
            let mut might_be_empty_file_input_in_html = false;

            {
                let field_ref = &options.allowed_fields[vi];

                // The HTTP request body of an empty file input in a HTML form sent by web browsers:
                // Content-Disposition: form-data; name="???"; filename=""
                // Content-Type: application/octet-stream
                if let Some(filename) = entry.file_name()
                    && filename.is_empty()
                {
                    // No need to check the MIME type. It's not practical.
                    might_be_empty_file_input_in_html = true;
                }

                // Whether to check content type
                if let Some(content_type_ref) = &field_ref.content_type {
                    let mut mat = false; // Is the content type matching?

                    if let Some(content_type) = entry.content_type() {
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
                            output_err = Some(MultipartFormDataError::DataTypeError(field_name));
                            break 'outer;
                        }
                    }

                    // The content type has been checked
                }
            }

            let drop_field = {
                // SAFETY: `vi` comes from a successful `binary_search_by` on `allowed_fields`, which has not been modified since then.
                let field = unsafe { options.allowed_fields.get_unchecked_mut(vi) };

                match field.typ {
                    MultipartFormDataType::File => {
                        let mut temporary_file =
                            match TemporaryFile::create(options.temporary_dir.clone()).await {
                                Ok(temporary_file) => temporary_file,
                                Err(err) => {
                                    output_err = Some(err.into());

                                    break 'outer;
                                },
                            };

                        let mut sum_c = 0u64;

                        loop {
                            match entry.chunk().await {
                                Ok(Some(bytes)) => {
                                    sum_c += bytes.len() as u64;

                                    if sum_c > field.size_limit {
                                        output_err = Some(
                                            MultipartFormDataError::DataTooLargeError(field_name),
                                        );

                                        break 'outer;
                                    }

                                    if let Err(err) = temporary_file.write_all(bytes.as_ref()).await
                                    {
                                        output_err = Some(err.into());

                                        break 'outer;
                                    }
                                },
                                Ok(None) => break,
                                Err(err) => {
                                    output_err = Some(err.into());

                                    break 'outer;
                                },
                            }
                        }

                        if might_be_empty_file_input_in_html {
                            if sum_c == 0 {
                                // This file might be from an empty file input in the HTML form, so ignore it.
                                output_err = None;
                                continue;
                            } else if output_err.is_some() {
                                break 'outer;
                            }
                        }

                        let target_path = match temporary_file.keep().await {
                            Ok(target_path) => target_path,
                            Err(err) => {
                                output_err = Some(err.into());

                                break 'outer;
                            },
                        };

                        files.entry(field_name).or_default().push(FileField {
                            content_type: entry.content_type().cloned(),
                            file_name:    entry.file_name().map(String::from),
                            path:         target_path,
                        });
                    },
                    typ @ (MultipartFormDataType::Raw | MultipartFormDataType::Text) => {
                        let mut buffer = Vec::new();

                        loop {
                            match entry.chunk().await {
                                Ok(Some(bytes)) => {
                                    if buffer.len() as u64 + bytes.len() as u64 > field.size_limit {
                                        output_err = Some(
                                            MultipartFormDataError::DataTooLargeError(field_name),
                                        );

                                        break 'outer;
                                    }

                                    buffer.extend_from_slice(bytes.as_ref());
                                },
                                Ok(None) => break,
                                Err(err) => {
                                    output_err = Some(err.into());

                                    break 'outer;
                                },
                            }
                        }

                        if might_be_empty_file_input_in_html {
                            if buffer.is_empty() {
                                // This file might be from an empty file input in the HTML form, so ignore it.
                                output_err = None;
                                continue;
                            } else if output_err.is_some() {
                                break 'outer;
                            }
                        }

                        let content_type = entry.content_type().cloned();
                        let file_name = entry.file_name().map(String::from);

                        if typ == MultipartFormDataType::Text {
                            let text = match String::from_utf8(buffer) {
                                Ok(s) => s,
                                Err(err) => {
                                    output_err = Some(err.into());

                                    break 'outer;
                                },
                            };

                            texts.entry(field_name).or_default().push(TextField {
                                content_type,
                                file_name,
                                text,
                            });
                        } else {
                            raw.entry(field_name).or_default().push(RawField {
                                content_type,
                                file_name,
                                raw: buffer,
                            });
                        }
                    },
                }

                field.repetition.decrease_check_is_over()
            };

            if drop_field {
                options.allowed_fields.remove(vi);
            }
        }

        if let Some(err) = output_err {
            for fields in files.into_values() {
                for f in fields {
                    try_delete(f.path);
                }
            }

            // Drain the rest of the stream so that the connection can be reused. Any error here is ignored to keep the original one.
            while let Ok(Some(_)) = multipart.next_field().await {}

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

struct TemporaryFile {
    file: Option<File>,
    path: Option<PathBuf>,
}

impl TemporaryFile {
    async fn create(temporary_dir: PathBuf) -> io::Result<Self> {
        task::spawn_blocking(move || {
            let target_file_name = format!(
                "rs-{}",
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            );
            let mut path = Path::join(&temporary_dir, &target_file_name);
            let mut index = 1usize;

            loop {
                let mut options = fs::OpenOptions::new();

                options.write(true).create_new(true);

                #[cfg(unix)]
                {
                    use std::os::unix::fs::OpenOptionsExt;

                    options.mode(0o600);
                }

                match options.open(&path) {
                    Ok(file) => {
                        return Ok(TemporaryFile {
                            file: Some(File::from_std(file)),
                            path: Some(path),
                        });
                    },
                    Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                        path = Path::join(&temporary_dir, format!("{target_file_name}-{index}"));
                        index += 1;
                    },
                    Err(err) => return Err(err),
                }
            }
        })
        .await
        .map_err(io::Error::other)?
    }

    async fn keep(mut self) -> io::Result<PathBuf> {
        self.file.as_mut().unwrap().flush().await?;
        self.file.take();
        Ok(self.path.take().unwrap())
    }

    async fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.file.as_mut().unwrap().write_all(bytes).await
    }
}

impl Drop for TemporaryFile {
    fn drop(&mut self) {
        self.file.take();

        if let Some(path) = self.path.take() {
            try_delete(path);
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
    let _ = fs::remove_file(path.as_ref());
}
