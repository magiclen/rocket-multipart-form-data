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
    tokio::{
        fs::File,
        io::{AsyncWriteExt, BufWriter},
        task,
    },
};

use crate::{
    FileField, MultipartFormDataError, MultipartFormDataOptions, MultipartFormDataType, RawField,
    TextField, mime,
    multer::{Constraints, Multipart, SizeLimit},
};

/// The buffer size for reading the HTTP body.
const READ_BUFFER_SIZE: usize = 64 * 1024;
/// The buffer size for writing a temporary file, in order to reduce the number of blocking tasks.
const WRITE_BUFFER_SIZE: usize = 64 * 1024;

/// Parsed multipart/form-data.
///
/// The temporary files in `files` are removed when this instance is dropped.
/// To keep a file, copy or rename it to another path before that.
/// If a `FileField` is taken out of `files`, its file is no longer removed automatically.
#[derive(Debug)]
pub struct MultipartFormData {
    /// File fields, keyed by the field name.
    pub files: HashMap<Arc<str>, Vec<FileField>>,
    /// Raw fields, keyed by the field name.
    pub raw:   HashMap<Arc<str>, Vec<RawField>>,
    /// Text fields, keyed by the field name.
    pub texts: HashMap<Arc<str>, Vec<TextField>>,
}

impl MultipartFormData {
    /// Parse multipart/form-data from the HTTP body.
    ///
    /// Temporary files are removed on a best-effort basis if parsing fails or this future is dropped.
    pub async fn parse(
        content_type: &ContentType,
        data: Data<'_>,
        options: MultipartFormDataOptions<'_>,
    ) -> Result<MultipartFormData, MultipartFormDataError> {
        if !content_type.is_form_data() {
            return Err(MultipartFormDataError::NotFormDataError);
        }

        let (_, boundary) = match content_type.params().find(|&(k, _)| k == "boundary") {
            Some(s) => s,
            None => return Err(MultipartFormDataError::BoundaryNotFoundError),
        };

        let stream = data.open(options.max_data_bytes.saturating_add(1).into());
        let constraints =
            Constraints::new().size_limit(SizeLimit::new().whole_stream(options.max_data_bytes));

        let multipart = Multipart::with_constraints(
            tokio_util::io::ReaderStream::with_capacity(stream, READ_BUFFER_SIZE),
            boundary,
            constraints,
        );

        Self::parse_multipart(multipart, options).await
    }

    async fn parse_multipart(
        mut multipart: Multipart<'_>,
        mut options: MultipartFormDataOptions<'_>,
    ) -> Result<Self, MultipartFormDataError> {
        options.allowed_fields.sort_by_key(|e| e.field_name);

        // Create the shared path once so that every file field only needs to bump a reference count.
        let temporary_dir: Arc<Path> = Arc::from(options.temporary_dir.as_path());

        let mut form = MultipartFormData {
            files: HashMap::new(),
            raw:   HashMap::new(),
            texts: HashMap::new(),
        };

        let mut output_err: Option<MultipartFormDataError> = None;

        'outer: loop {
            let mut entry = match multipart.next_field().await {
                Ok(Some(entry)) => entry,
                Ok(None) => break,
                Err(err) => {
                    // Keep the error so the rest of the stream can be drained below.
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

            let drop_field = {
                let field = &mut options.allowed_fields[vi];

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
                if let Some(content_type_ref) = &field.content_type {
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

                match field.typ {
                    MultipartFormDataType::File => {
                        let mut temporary_file =
                            match TemporaryFile::create(temporary_dir.clone()).await {
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

                        form.files.entry(field_name).or_default().push(FileField {
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

                            form.texts.entry(field_name).or_default().push(TextField {
                                content_type,
                                file_name,
                                text,
                            });
                        } else {
                            form.raw.entry(field_name).or_default().push(RawField {
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
            drop(form);

            // Drain the rest of the stream so that the connection can be reused. Any error here is ignored to keep the original one.
            while let Ok(Some(_)) = multipart.next_field().await {}

            Err(err)
        } else {
            Ok(form)
        }
    }
}

struct TemporaryFile {
    file: Option<BufWriter<File>>,
    path: Option<PathBuf>,
}

impl TemporaryFile {
    async fn create(temporary_dir: Arc<Path>) -> io::Result<Self> {
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
                            file: Some(BufWriter::with_capacity(
                                WRITE_BUFFER_SIZE,
                                File::from_std(file),
                            )),
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

#[cfg(test)]
mod tests {
    use std::{
        fs,
        future::poll_fn,
        io,
        sync::atomic::{AtomicBool, Ordering},
        task::Poll,
        time::Duration,
    };

    use rocket::{
        futures::stream,
        tokio::time::{Instant, timeout, timeout_at},
    };
    use tempfile::tempdir;

    use super::MultipartFormData;
    use crate::{MultipartFormDataField, MultipartFormDataOptions, multer::Multipart};

    #[rocket::async_test]
    async fn timing_out_parsing_removes_completed_and_partial_files() {
        let temporary_dir = tempdir().unwrap();
        let options = MultipartFormDataOptions {
            temporary_dir: temporary_dir.path().to_path_buf(),
            allowed_fields: vec![
                MultipartFormDataField::file("first"),
                MultipartFormDataField::file("second"),
            ],
            ..MultipartFormDataOptions::default()
        };

        let mut body = Some(
            concat!(
                "--TEST-BOUNDARY\r\n",
                "Content-Disposition: form-data; name=\"first\"; filename=\"first.txt\"\r\n\r\n",
                "First file\r\n",
                "--TEST-BOUNDARY\r\n",
                "Content-Disposition: form-data; name=\"second\"; filename=\"second.txt\"\r\n\r\n",
                "Partial second file",
            )
            .as_bytes(),
        );
        let pending_source = AtomicBool::new(false);
        let stream = stream::poll_fn(|_| match body.take() {
            Some(body) => Poll::Ready(Some(Ok::<_, io::Error>(body))),
            None => {
                pending_source.store(true, Ordering::Relaxed);
                Poll::Pending
            },
        });
        let multipart = Multipart::new(stream, "TEST-BOUNDARY");
        let mut parsing = Box::pin(MultipartFormData::parse_multipart(multipart, options));

        timeout(
            Duration::from_secs(10),
            poll_fn(|cx| {
                assert!(parsing.as_mut().poll(cx).is_pending());

                // The second file is created only after the first file has been kept.
                if fs::read_dir(temporary_dir.path()).unwrap().count() != 2 {
                    return Poll::Pending;
                }

                // A fresh read confirms that the second creation task has returned.
                pending_source.store(false, Ordering::Relaxed);
                assert!(parsing.as_mut().poll(cx).is_pending());

                if pending_source.load(Ordering::Relaxed) { Poll::Ready(()) } else { Poll::Pending }
            }),
        )
        .await
        .unwrap();

        assert!(timeout_at(Instant::now(), parsing).await.is_err());
        assert_eq!(0, fs::read_dir(temporary_dir.path()).unwrap().count());
    }
}
