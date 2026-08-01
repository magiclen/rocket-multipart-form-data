use std::{env, path::PathBuf};

use crate::MultipartFormDataField;

/// Options for parsing multipart/form-data.
#[derive(Debug, Clone)]
pub struct MultipartFormDataOptions<'a> {
    /// The maximum number of bytes to read from the complete multipart stream.
    ///
    /// This limit includes multipart headers and fields that are not allowed.
    /// The default is `u64::MAX`, so applications should set a finite limit for untrusted input.
    ///
    /// Once parsing fails, the rest of the stream is still read out so that the connection can be reused, and this limit is what bounds that read.
    pub max_data_bytes: u64,
    /// A path of directory where the uploaded files will be stored. It should be created before parsing.
    pub temporary_dir:  PathBuf,
    /// Allowed fields of data.
    ///
    /// Several entries may share the same `field_name`. Their repetitions add up, but which entry is applied to a specific occurrence is unspecified.
    pub allowed_fields: Vec<MultipartFormDataField<'a>>,
}

impl<'a> MultipartFormDataOptions<'a> {
    /// Create a default `MultipartFormDataOptions` instance.
    #[inline]
    #[must_use]
    pub fn new() -> MultipartFormDataOptions<'a> {
        MultipartFormDataOptions {
            max_data_bytes: u64::MAX,
            temporary_dir:  env::temp_dir(),
            allowed_fields: Vec::new(),
        }
    }

    /// Create a `MultipartFormDataOptions` instance with existing multipart_form_data_fields.
    #[inline]
    #[must_use]
    pub fn with_multipart_form_data_fields(
        allowed_fields: Vec<MultipartFormDataField<'a>>,
    ) -> MultipartFormDataOptions<'a> {
        MultipartFormDataOptions {
            max_data_bytes: u64::MAX,
            temporary_dir: env::temp_dir(),
            allowed_fields,
        }
    }
}

impl<'a> Default for MultipartFormDataOptions<'a> {
    #[inline]
    fn default() -> Self {
        MultipartFormDataOptions::new()
    }
}
