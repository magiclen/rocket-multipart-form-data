use std::{
    error::Error,
    fmt::{self, Display, Formatter},
    io,
    string::FromUtf8Error,
    sync::Arc,
};

use crate::multer;

#[derive(Debug)]
pub enum MultipartFormDataError {
    NotFormDataError,
    BoundaryNotFoundError,
    IOError(io::Error),
    MulterError(multer::Error),
    FromUtf8Error(FromUtf8Error),
    DataTooLargeError(Arc<str>),
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
                f.write_fmt(format_args!("The data of field `{}` is too large.", field))
            },
            MultipartFormDataError::DataTypeError(field) => {
                f.write_fmt(format_args!("The data type of field `{}` is incorrect.", field))
            },
        }
    }
}

impl Error for MultipartFormDataError {}
