use crate::MultipartFormDataType;

use std::cmp::Ordering;
use std::path::PathBuf;
use std::str::FromStr;

use mime::Mime;

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
    pub fn text(field_name: &'a str) -> MultipartFormDataField<'a> {
        MultipartFormDataField {
            typ: MultipartFormDataType::Text,
            field_name,
            size_limit: DEFAULT_IN_MEMORY_DATA_LIMIT,
            content_type: None,
            repetition: Repetition::default(),
        }
    }

    /// Create a raw field, the default size_limit is 1 MiB.
    #[inline]
    pub fn bytes(field_name: &'a str) -> MultipartFormDataField<'a> {
        Self::raw(field_name)
    }

    /// Create a raw field, the default size_limit is 1 MiB.
    #[inline]
    pub fn raw(field_name: &'a str) -> MultipartFormDataField<'a> {
        MultipartFormDataField {
            typ: MultipartFormDataType::Raw,
            field_name,
            size_limit: DEFAULT_IN_MEMORY_DATA_LIMIT,
            content_type: None,
            repetition: Repetition::default(),
        }
    }

    /// Create a file field, the default size_limit is 8 MiB.
    #[inline]
    pub fn file(field_name: &'a str) -> MultipartFormDataField<'a> {
        MultipartFormDataField {
            typ: MultipartFormDataType::File,
            field_name,
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

impl<'a> PartialEq for MultipartFormDataField<'a> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.field_name.eq(other.field_name)
    }
}

impl<'a> Eq for MultipartFormDataField<'a> {}

impl<'a> PartialOrd for MultipartFormDataField<'a> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.field_name.partial_cmp(other.field_name)
    }
}

impl<'a> Ord for MultipartFormDataField<'a> {
    #[inline]
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

#[derive(Debug, Clone, Copy)]
enum RepetitionCounter {
    Fixed(u32),
    Infinite,
}

impl RepetitionCounter {
    #[inline]
    pub(crate) fn decrease_check_is_over(&mut self) -> bool {
        match self {
            RepetitionCounter::Fixed(n) => {
                debug_assert!(*n > 0);

                *n -= 1;

                *n == 0
            }
            RepetitionCounter::Infinite => false,
        }
    }
}

impl Default for RepetitionCounter {
    #[inline]
    fn default() -> Self {
        RepetitionCounter::Fixed(1)
    }
}

#[derive(Debug, Clone, Copy)]
/// It can be used to define a `MultipartFormDataField` instance can be used how many times.
pub struct Repetition {
    counter: RepetitionCounter,
}

impl Repetition {
    #[inline]
    /// Create a `Repetition` instance for only one time.
    pub fn new() -> Repetition {
        Repetition::fixed(1)
    }

    #[inline]
    /// Create a `Repetition` instance for any fixed times.
    pub fn fixed(count: u32) -> Repetition {
        if count == 0 {
            eprintln!("The count of fixed repetition for a `MultipartFormDataField` instance should be bigger than 0. Use 1 instead.");

            Repetition {
                counter: RepetitionCounter::Fixed(1),
            }
        } else {
            Repetition {
                counter: RepetitionCounter::Fixed(count),
            }
        }
    }

    #[inline]
    /// Create a `Repetition` instance for infinite times.
    pub fn infinite() -> Repetition {
        Repetition {
            counter: RepetitionCounter::Infinite,
        }
    }

    #[inline]
    pub(crate) fn decrease_check_is_over(&mut self) -> bool {
        self.counter.decrease_check_is_over()
    }
}

impl Default for Repetition {
    #[inline]
    /// Create a `Repetition` instance for only one time.
    fn default() -> Self {
        Repetition::new()
    }
}
