#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket_include_static_resources;

extern crate rocket_raw_response;

#[macro_use]
extern crate rocket;

extern crate rocket_multipart_form_data;

use rocket::http::ContentType;
use rocket::Data;

use rocket_include_static_resources::{EtagIfNoneMatch, StaticResponse};

use rocket_multipart_form_data::mime;
use rocket_multipart_form_data::{
    MultipartFormData, MultipartFormDataError, MultipartFormDataField, MultipartFormDataOptions,
    RawField,
};

use rocket_raw_response::RawResponse;

#[get("/")]
fn index(etag_if_none_match: EtagIfNoneMatch) -> StaticResponse {
    static_response!(etag_if_none_match, "html-image-uploader")
}

#[post("/upload", data = "<data>")]
fn upload(content_type: &ContentType, data: Data) -> Result<RawResponse, &'static str> {
    let mut options = MultipartFormDataOptions::new();
    options.allowed_fields.push(
        MultipartFormDataField::raw("image")
            .size_limit(32 * 1024 * 1024)
            .content_type_by_string(Some(mime::IMAGE_STAR))
            .unwrap(),
    );

    let mut multipart_form_data = match MultipartFormData::parse(content_type, data, options) {
        Ok(multipart_form_data) => multipart_form_data,
        Err(err) => {
            match err {
                MultipartFormDataError::DataTooLargeError(_) => {
                    return Err("The file is too large.")
                }
                MultipartFormDataError::DataTypeError(_) => {
                    return Err("The file is not an image.")
                }
                _ => panic!("{:?}", err),
            }
        }
    };

    let image = multipart_form_data.raw.remove("image");

    match image {
        Some(image) => {
            match image {
                RawField::Single(raw) => {
                    let content_type = raw.content_type;
                    let file_name = raw.file_name.unwrap_or("Image".to_string());
                    let data = raw.raw;

                    Ok(RawResponse::from_vec(data, Some(file_name), content_type))
                }
                RawField::Multiple(_) => unreachable!(),
            }
        }
        None => Err("Please input a file."),
    }
}

fn main() {
    rocket::ignite()
        .attach(StaticResponse::fairing(|resources| {
            static_resources_initialize!(
                resources,
                "html-image-uploader",
                "examples/front-end/html/image-uploader.html",
            );
        }))
        .mount("/", routes![index])
        .mount("/", routes![upload])
        .launch();
}
