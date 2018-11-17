#![feature(plugin)]
#![feature(const_vec_new)]
#![plugin(rocket_codegen)]

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate lazy_static_include;

#[macro_use]
extern crate rocket_include_static_resources;

extern crate rocket_raw_response;

extern crate rocket;

extern crate rocket_multipart_form_data;

use rocket::Data;
use rocket::http::ContentType;

use rocket_multipart_form_data::mime;
use rocket_multipart_form_data::{MultipartFormDataOptions, MultipartFormData, MultipartFormDataField, MultipartFormDataError, RawField};

use rocket_include_static_resources::EtagIfNoneMatch;

use rocket_raw_response::RawResponse;

use rocket::response::Response;

static_resources_initialize!(
   "html-image-upload", "examples/front-end/html/image-upload.html",
);

#[get("/")]
fn index(etag_if_none_match: EtagIfNoneMatch) -> Response<'static> {
    static_response!(etag_if_none_match, "html-image-upload")
}

#[post("/upload", data = "<data>")]
fn upload(content_type: &ContentType, data: Data) -> RawResponse {
    let mut options = MultipartFormDataOptions::new();
    options.allowed_fields.push(MultipartFormDataField::raw("image").size_limit(32 * 1024 * 1024).content_type_by_string(Some(mime::IMAGE_STAR)).unwrap());

    let mut multipart_form_data = match MultipartFormData::parse(content_type, data, options) {
        Ok(multipart_form_data) => multipart_form_data,
        Err(err) => match err {
            MultipartFormDataError::DataTooLargeError(_) => return RawResponse::from_vec("The file is too large.".bytes().collect(), "", Some(mime::TEXT_PLAIN_UTF_8)),
            MultipartFormDataError::DataTypeError(_) => return RawResponse::from_vec("The file is not an image.".bytes().collect(), "", Some(mime::TEXT_PLAIN_UTF_8)),
            _ => panic!("{:?}", err)
        }
    };

    let image = multipart_form_data.raw.remove(&"image".to_string());

    match image {
        Some(image) => match image {
            RawField::Single(raw) => {
                let content_type = raw.content_type;
                let file_name = raw.file_name.unwrap_or("Image".to_string());
                let data = raw.raw;

                RawResponse::from_vec(data, file_name, content_type)
            }
            RawField::Multiple(_) => unreachable!()
        },
        None => RawResponse::from_vec("Please input a file.".bytes().collect(), "", Some(mime::TEXT_PLAIN_UTF_8))
    }
}

fn main() {
    rocket::ignite()
        .mount("/", routes![index])
        .mount("/", routes![upload])
        .launch();
}