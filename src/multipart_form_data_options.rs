use std::env;
use std::path::PathBuf;

use crate::MultipartFormDataField;

/// Options for parsing multipart/form-data.
#[derive(Debug)]
pub struct MultipartFormDataOptions<'a> {
    /// A path of directory where the uploaded files will be stored. It should be created before parsing.
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

    /// Create a `MultipartFormDataOptions` instance with existing multipart_form_data_fields.
    #[inline]
    pub fn with_multipart_form_data_fields(
        allowed_fields: Vec<MultipartFormDataField<'a>>,
    ) -> MultipartFormDataOptions<'a> {
        MultipartFormDataOptions {
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
