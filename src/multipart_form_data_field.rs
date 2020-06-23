use std::str::FromStr;

use crate::mime::Mime;

use crate::{MultipartFormDataType, Repetition};

const DEFAULT_IN_MEMORY_DATA_LIMIT: u64 = 1024 * 1024;
const DEFAULT_FILE_DATA_LIMIT: u64 = 8 * 1024 * 1024;

/// The guarder for fields.
#[derive(Debug, Clone)]
pub struct MultipartFormDataField<'a> {
    /// The type of this field.
    pub typ: MultipartFormDataType,
    /// The name of this field.
    pub field_name: &'a str,
    /// The size limit for this field.
    pub size_limit: u64,
    /// To filter the content types. It supports stars.
    pub content_type: Option<Vec<Mime>>,
    /// To define this `MultipartFormDataField` instance can be used how many times.
    pub repetition: Repetition,
}

impl<'a> MultipartFormDataField<'a> {
    /// Create a text field, the default size_limit is 1 MiB.
    #[inline]
    pub fn text<S: ?Sized + AsRef<str>>(field_name: &S) -> MultipartFormDataField {
        MultipartFormDataField {
            typ: MultipartFormDataType::Text,
            field_name: field_name.as_ref(),
            size_limit: DEFAULT_IN_MEMORY_DATA_LIMIT,
            content_type: None,
            repetition: Repetition::default(),
        }
    }

    /// Create a raw field, the default size_limit is 1 MiB.
    #[inline]
    pub fn bytes<S: ?Sized + AsRef<str>>(field_name: &S) -> MultipartFormDataField {
        Self::raw(field_name.as_ref())
    }

    /// Create a raw field, the default size_limit is 1 MiB.
    #[inline]
    pub fn raw<S: ?Sized + AsRef<str>>(field_name: &S) -> MultipartFormDataField {
        MultipartFormDataField {
            typ: MultipartFormDataType::Raw,
            field_name: field_name.as_ref(),
            size_limit: DEFAULT_IN_MEMORY_DATA_LIMIT,
            content_type: None,
            repetition: Repetition::default(),
        }
    }

    /// Create a file field, the default size_limit is 8 MiB.
    #[inline]
    pub fn file<S: ?Sized + AsRef<str>>(field_name: &S) -> MultipartFormDataField {
        MultipartFormDataField {
            typ: MultipartFormDataType::File,
            field_name: field_name.as_ref(),
            size_limit: DEFAULT_FILE_DATA_LIMIT,
            content_type: None,
            repetition: Repetition::default(),
        }
    }

    /// Set the size_limit for this field.
    #[inline]
    pub fn size_limit(mut self, size_limit: u64) -> MultipartFormDataField<'a> {
        self.size_limit = size_limit;
        self
    }

    /// Add a content type filter for this field. This method can be used multiple times to use multiple content type filters.
    #[inline]
    pub fn content_type(mut self, content_type: Option<Mime>) -> MultipartFormDataField<'a> {
        match content_type {
            Some(content_type) => {
                match self.content_type.as_mut() {
                    Some(v) => {
                        v.push(content_type);
                    }
                    None => {
                        self.content_type = Some(vec![content_type]);
                    }
                }
            }
            None => self.content_type = None,
        }
        self
    }

    /// Add a content type filter for this field. This method can be used multiple times to use multiple content type filters.
    #[inline]
    pub fn content_type_by_string<S: AsRef<str>>(
        mut self,
        content_type: Option<S>,
    ) -> Result<MultipartFormDataField<'a>, mime::FromStrError> {
        match content_type {
            Some(content_type) => {
                let content_type = Mime::from_str(content_type.as_ref())?;
                match self.content_type.as_mut() {
                    Some(v) => {
                        v.push(content_type);
                    }
                    None => {
                        self.content_type = Some(vec![content_type]);
                    }
                }
            }
            None => self.content_type = None,
        }
        Ok(self)
    }

    /// Set the repetition for this field.
    #[inline]
    pub fn repetition(mut self, repetition: Repetition) -> MultipartFormDataField<'a> {
        self.repetition = repetition;
        self
    }
}
