use std::io::{Error, ErrorKind};

pub fn build_invalid_data_error_str(text: &str) -> Error {
    Error::new(ErrorKind::InvalidData, text)
}

pub fn build_invalid_data_error_string(text: String) -> Error {
    Error::new(ErrorKind::InvalidData, text)
}
