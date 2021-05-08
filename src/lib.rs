/*!
# Multipart Form Data for Rocket Framework

This crate provides a multipart parser for the Rocket framework.

## Example

```rust
#[macro_use] extern crate rocket;
extern crate rocket_multipart_form_data;

use rocket::Data;
use rocket::http::ContentType;

use rocket_multipart_form_data::{mime, MultipartFormDataOptions, MultipartFormData, MultipartFormDataField, Repetition};

#[post("/", data = "<data>")]
async fn index(content_type: &ContentType, data: Data) -> &'static str {
    let mut options = MultipartFormDataOptions::with_multipart_form_data_fields(
        vec! [
            MultipartFormDataField::file("photo").content_type_by_string(Some(mime::IMAGE_STAR)).unwrap(),
            MultipartFormDataField::raw("fingerprint").size_limit(4096),
            MultipartFormDataField::text("name"),
            MultipartFormDataField::text("email").repetition(Repetition::fixed(3)),
            MultipartFormDataField::text("email"),
        ]
    );

    let mut multipart_form_data = MultipartFormData::parse(content_type, data, options).await.unwrap();

    let photo = multipart_form_data.files.get("photo"); // Use the get method to preserve file fields from moving out of the MultipartFormData instance in order to delete them automatically when the MultipartFormData instance is being dropped
    let fingerprint = multipart_form_data.raw.remove("fingerprint"); // Use the remove method to move raw fields out of the MultipartFormData instance (recommended)
    let name = multipart_form_data.texts.remove("name"); // Use the remove method to move text fields out of the MultipartFormData instance (recommended)
    let email = multipart_form_data.texts.remove("email");

    if let Some(file_fields) = photo {
        let file_field = &file_fields[0]; // Because we only put one "photo" field to the allowed_fields, the max length of this file_fields is 1.

        let _content_type = &file_field.content_type;
        let _file_name = &file_field.file_name;
        let _path = &file_field.path;

        // You can now deal with the uploaded file.
    }

    if let Some(mut raw_fields) = fingerprint {
        let raw_field = raw_fields.remove(0); // Because we only put one "fingerprint" field to the allowed_fields, the max length of this raw_fields is 1.

        let _content_type = raw_field.content_type;
        let _file_name = raw_field.file_name;
        let _raw = raw_field.raw;

        // You can now deal with the raw data.
    }

    if let Some(mut text_fields) = name {
        let text_field = text_fields.remove(0); // Because we only put one "text" field to the allowed_fields, the max length of this text_fields is 1.

        let _content_type = text_field.content_type;
        let _file_name = text_field.file_name;
        let _text = text_field.text;

        // You can now deal with the text data.
    }

    if let Some(text_fields) = email {
        for text_field in text_fields { // We put "email" field to the allowed_fields for two times and let the first time repeat for 3 times, so the max length of this text_fields is 4.
            let _content_type = text_field.content_type;
            let _file_name = text_field.file_name;
            let _text = text_field.text;

            // You can now deal with the text data.
        }
    }

    "ok"
}
```

Also see `examples`.
*/

pub extern crate mime;
pub extern crate multer;

mod fields;
mod multipart_form_data;
mod multipart_form_data_errors;
mod multipart_form_data_field;
mod multipart_form_data_options;
mod multipart_form_data_type;
mod repetition;

pub use fields::*;
pub use multipart_form_data::*;
pub use multipart_form_data_errors::*;
pub use multipart_form_data_field::*;
pub use multipart_form_data_options::*;
pub use multipart_form_data_type::*;
pub use repetition::*;
