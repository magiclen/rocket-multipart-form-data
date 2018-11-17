use ::MultipartFormDataType;

use std::cmp::Ordering;
use std::path::PathBuf;
use std::str::FromStr;

use mime::Mime;

const DEFAULT_IN_MEMORY_DATA_LIMIT: u64 = 1 * 1024 * 1024;
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
}

impl<'a> MultipartFormDataField<'a> {
    /// Create a text field, the default size_limit is 1 MiB.
    pub fn text(field_name: &'a str) -> MultipartFormDataField<'a> {
        MultipartFormDataField {
            typ: MultipartFormDataType::Text,
            field_name,
            size_limit: DEFAULT_IN_MEMORY_DATA_LIMIT,
            content_type: None,
        }
    }

    /// Create a raw field, the default size_limit is 1 MiB.
    pub fn bytes(field_name: &'a str) -> MultipartFormDataField<'a> {
        Self::raw(field_name)
    }

    /// Create a raw field, the default size_limit is 1 MiB.
    pub fn raw(field_name: &'a str) -> MultipartFormDataField<'a> {
        MultipartFormDataField {
            typ: MultipartFormDataType::Raw,
            field_name,
            size_limit: DEFAULT_IN_MEMORY_DATA_LIMIT,
            content_type: None,
        }
    }

    /// Create a file field, the default size_limit is 8 MiB.
    pub fn file(field_name: &'a str) -> MultipartFormDataField<'a> {
        MultipartFormDataField {
            typ: MultipartFormDataType::File,
            field_name,
            size_limit: DEFAULT_FILE_DATA_LIMIT,
            content_type: None,
        }
    }

    /// Set the size_limit for this field.
    pub fn size_limit(mut self, size_limit: u64) -> MultipartFormDataField<'a> {
        self.size_limit = size_limit;
        self
    }

    /// Add a content type filter for this field. This method can be used multiple times to use multiple content type filters.
    pub fn content_type(mut self, content_type: Option<Mime>) -> MultipartFormDataField<'a> {
        match content_type {
            Some(content_type) => {
                match self.content_type {
                    Some(mut v) => {
                        v.push(content_type);
                        self.content_type = Some(v);
                    }
                    None => {
                        self.content_type = Some(vec![content_type]);
                    }
                }
            }
            None => self.content_type = None
        }
        self
    }

    /// Add a content type filter for this field. This method can be used multiple times to use multiple content type filters.
    pub fn content_type_by_string<S: AsRef<str>>(mut self, content_type: Option<S>) -> Result<MultipartFormDataField<'a>, mime::FromStrError> {
        match content_type {
            Some(content_type) => {
                let content_type = Mime::from_str(content_type.as_ref())?;
                match self.content_type {
                    Some(mut v) => {
                        v.push(content_type);
                        self.content_type = Some(v);
                    }
                    None => {
                        self.content_type = Some(vec![content_type]);
                    }
                }
            }
            None => self.content_type = None
        }
        Ok(self)
    }
}

impl<'a> PartialEq for MultipartFormDataField<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.field_name.eq(other.field_name)
    }
}

impl<'a> Eq for MultipartFormDataField<'a> {}

impl<'a> PartialOrd for MultipartFormDataField<'a> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.field_name.partial_cmp(other.field_name)
    }
}

impl<'a> Ord for MultipartFormDataField<'a> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.field_name.cmp(other.field_name)
    }
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
pub struct SingleRawField {
    pub content_type: Option<Mime>,
    pub file_name: Option<String>,
    pub raw: Vec<u8>,
}

#[derive(Debug)]
pub enum RawField {
    Single(SingleRawField),
    Multiple(Vec<SingleRawField>),
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