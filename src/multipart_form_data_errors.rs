use std::{
    error::Error,
    fmt::{self, Display, Formatter},
    io,
    string::FromUtf8Error,
    sync::Arc,
};

use crate::multer;

/// Errors which may occur while parsing multipart/form-data.
#[derive(Debug)]
pub enum MultipartFormDataError {
    /// The content type of the request is not `multipart/form-data`.
    NotFormDataError,
    /// The content type of the request has no `boundary` parameter.
    BoundaryNotFoundError,
    /// A temporary file cannot be created or written.
    IOError(io::Error),
    /// The multipart stream is malformed, or one of the limits of `multer` is exceeded.
    MulterError(multer::Error),
    /// The data of a text field is not valid UTF-8.
    FromUtf8Error(FromUtf8Error),
    /// The data of the field exceeds its `size_limit`.
    DataTooLargeError(Arc<str>),
    /// The content type of the field does not match any of its content type filters.
    DataTypeError(Arc<str>),
}

impl From<io::Error> for MultipartFormDataError {
    #[inline]
    fn from(err: io::Error) -> MultipartFormDataError {
        MultipartFormDataError::IOError(err)
    }
}

impl From<multer::Error> for MultipartFormDataError {
    #[inline]
    fn from(err: multer::Error) -> MultipartFormDataError {
        MultipartFormDataError::MulterError(err)
    }
}

impl From<FromUtf8Error> for MultipartFormDataError {
    #[inline]
    fn from(err: FromUtf8Error) -> MultipartFormDataError {
        MultipartFormDataError::FromUtf8Error(err)
    }
}

impl Display for MultipartFormDataError {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        match self {
            MultipartFormDataError::NotFormDataError => {
                f.write_str("The content type is not `multipart/form-data`.")
            },
            MultipartFormDataError::BoundaryNotFoundError => f.write_str(
                "The boundary cannot be found. Maybe the multipart form data is incorrect.",
            ),
            MultipartFormDataError::IOError(err) => Display::fmt(err, f),
            MultipartFormDataError::MulterError(err) => Display::fmt(err, f),
            MultipartFormDataError::FromUtf8Error(err) => Display::fmt(err, f),
            MultipartFormDataError::DataTooLargeError(field) => {
                write!(f, "The data of field `{field}` is too large.")
            },
            MultipartFormDataError::DataTypeError(field) => {
                write!(f, "The data type of field `{field}` is incorrect.")
            },
        }
    }
}

impl Error for MultipartFormDataError {
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            MultipartFormDataError::IOError(err) => Some(err),
            MultipartFormDataError::MulterError(err) => Some(err),
            MultipartFormDataError::FromUtf8Error(err) => Some(err),
            _ => None,
        }
    }
}
