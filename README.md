Multipart Form Data for Rocket Framework
====================

[![Build Status](https://travis-ci.org/magiclen/rocket-multipart-form-data.svg?branch=master)](https://travis-ci.org/magiclen/rocket-multipart-form-data)

This crate provides a multipart parser for the Rocket framework.

## Example

```rust
#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use] extern crate rocket;
extern crate rocket_multipart_form_data;

use rocket::Data;
use rocket::http::ContentType;

use rocket_multipart_form_data::{mime, MultipartFormDataOptions, MultipartFormData, MultipartFormDataField, FileField, TextField, RawField};

#[post("/", data = "<data>")]
fn index(content_type: &ContentType, data: Data) -> &'static str
{
    let mut options = MultipartFormDataOptions::new();
    options.allowed_fields.push(MultipartFormDataField::file("photo").content_type_by_string(Some(mime::IMAGE_STAR)).unwrap());
    options.allowed_fields.push(MultipartFormDataField::raw("fingerprint").size_limit(4096));
    options.allowed_fields.push(MultipartFormDataField::text("name"));
    options.allowed_fields.push(MultipartFormDataField::text("array_max_length_3"));
    options.allowed_fields.push(MultipartFormDataField::text("array_max_length_3"));
    options.allowed_fields.push(MultipartFormDataField::text("array_max_length_3"));

    let multipart_form_data = MultipartFormData::parse(content_type, data, options).unwrap();

    let photo = multipart_form_data.files.get("photo");
    let fingerprint = multipart_form_data.raw.get("fingerprint");
    let name = multipart_form_data.texts.get("name");
    let array = multipart_form_data.texts.get("array_max_length_3");

    if let Some(photo) = photo {
        match photo {
            FileField::Single(file) => {
                let _content_type = &file.content_type;
                let _file_name = &file.file_name;
                let _path = &file.path;
                // You can now deal with the uploaded file. The file will be deleted automatically when the MultipartFormData instance is dropped. If you want to handle that file by your own, instead of killing it, just remove it out from the MultipartFormData instance.
            }
            FileField::Multiple(_files) => {
                // Because we only put one "photo" field to the allowed_fields, this arm will not be matched.
            }
        }
    }

    if let Some(fingerprint) = fingerprint {
        match fingerprint {
            RawField::Single(raw) => {
                let _content_type = &raw.content_type;
                let _file_name = &raw.file_name;
                let _raw = &raw.raw;
                // You can now deal with the raw data.
            }
            RawField::Multiple(_bytes) => {
                // Because we only put one "fingerprint" field to the allowed_fields, this arm will not be matched.
            }
        }
    }

    if let Some(name) = name {
        match name {
            TextField::Single(text) => {
                let _content_type = &text.content_type;
                let _file_name = &text.file_name;
                let _text = &text.text;
                // You can now deal with the raw data.
            }
            TextField::Multiple(_texts) => {
                // Because we only put one "text" field to the allowed_fields, this arm will not be matched.
            }
        }
    }

    if let Some(array) = array {
        match array {
            TextField::Single(text) => {
                let _content_type = &text.content_type;
                let _file_name = &text.file_name;
                let _text = &text.text;
                // You can now deal with the text data.
            }
            TextField::Multiple(texts) => {
                // Because we put "array_max_length_3" field to the allowed_fields for three times, this arm will probably be matched.

                for text in texts { // The max length of the "texts" variable is 3
                    let _content_type = &text.content_type;
                    let _file_name = &text.file_name;
                    let _text = &text.text;
                    // You can now deal with the text data.
                }
            }
        }
    }

    "ok"
}
```

Also see `examples`.

## Crates.io

https://crates.io/crates/rocket-multipart-form-data

## Documentation

https://docs.rs/rocket-multipart-form-data

## License

[MIT](LICENSE)