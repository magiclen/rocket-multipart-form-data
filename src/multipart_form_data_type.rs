#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum MultipartFormDataType {
    /// Stored the parsed data as a string.
    Text,
    /// Stored the parsed data as a Vec<u8> instance.
    Raw,
    /// Stored the parsed data as a file.
    File,
}
