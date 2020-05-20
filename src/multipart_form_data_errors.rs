use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::io;
use std::string::FromUtf8Error;
use std::sync::Arc;

#[derive(Debug)]
pub enum MultipartFormDataError {
    NotFormDataError,
    BoundaryNotFoundError,
    IOError(io::Error),
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
            }
            MultipartFormDataError::BoundaryNotFoundError => {
                f.write_str(
                    "The boundary cannot be found. Maybe the multipart form data is incorrect.",
                )
            }
            MultipartFormDataError::IOError(err) => Display::fmt(err, f),
            MultipartFormDataError::FromUtf8Error(err) => Display::fmt(err, f),
            MultipartFormDataError::DataTooLargeError(field) => {
                f.write_fmt(format_args!("The data of field `{}` is too large.", field))
            }
            MultipartFormDataError::DataTypeError(field) => {
                f.write_fmt(format_args!("The data type of field `{}` is incorrect.", field))
            }
        }
    }
}

impl Error for MultipartFormDataError {}
