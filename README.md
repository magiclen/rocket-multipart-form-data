Multipart Form Data for Rocket Framework
====================

This crate provides a multipart parser for the Rocket framework.

## Example

```rust
#![feature(plugin)]
#![plugin(rocket_codegen)]

extern crate rocket;
extern crate rocket_multipart_form_data;

use rocket::Data;
use rocket::http::ContentType;

use rocket_multipart_form_data::{MultipartFormDataOptions, MultipartFormData};

#[post("/", data = "<data>")]
fn index(content_type: &ContentType, data: Data) -> &'static str
{
    let mut options = MultipartFormDataOptions::new();
    options.allowed_file_fields.push("photo");
    options.allowed_text_fields.push("name");
    options.allowed_text_fields.push("array_max_length_3");
    options.allowed_text_fields.push("array_max_length_3");
    options.allowed_text_fields.push("array_max_length_3");

    let multipart_form_data = MultipartFormData::parse(content_type, data, options).unwrap();

    let photo = multipart_form_data.files.get(&"photo".to_string());
    let name = multipart_form_data.texts.get(&"name".to_string());
    let array = multipart_form_data.texts.get(&"array_max_length_3".to_string());

    println!("name = {:?}", name);
    println!("photo = {:?}", photo);
    println!("array = {:?}", array);

    "ok"
}
```

## License

[MIT](LICENSE)