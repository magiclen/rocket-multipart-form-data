#[macro_use]
extern crate rocket_include_static_resources;

#[macro_use]
extern crate rocket;

use rocket::{Data, http::ContentType};
use rocket_multipart_form_data::{
    MultipartFormData, MultipartFormDataError, MultipartFormDataField, MultipartFormDataOptions,
    mime, multer,
};
use rocket_raw_response::RawResponse;

static_response_handler! {
    "/" => index => "html-image-uploader",
}

#[post("/upload", data = "<data>")]
async fn upload(content_type: &ContentType, data: Data<'_>) -> Result<RawResponse, &'static str> {
    let options = MultipartFormDataOptions {
        max_data_bytes: 33 * 1024 * 1024,
        allowed_fields: vec![
            MultipartFormDataField::raw("image")
                .size_limit(32 * 1024 * 1024)
                .content_type(Some(mime::IMAGE_STAR)),
        ],
        ..MultipartFormDataOptions::default()
    };

    let mut multipart_form_data = match MultipartFormData::parse(content_type, data, options).await
    {
        Ok(multipart_form_data) => multipart_form_data,
        Err(err) => match err {
            MultipartFormDataError::DataTooLargeError(_) => {
                return Err("The file is too large.");
            },
            MultipartFormDataError::DataTypeError(_) => {
                return Err("The file is not an image.");
            },
            MultipartFormDataError::MulterError(multer::Error::StreamSizeExceeded {
                ..
            }) => {
                return Err("The request body seems too large.");
            },
            _ => return Err("The upload could not be parsed."),
        },
    };

    let image = multipart_form_data.raw.remove("image");

    match image {
        Some(mut image) => {
            let raw = image.remove(0);

            let content_type = raw.content_type;
            let file_name = raw.file_name.unwrap_or_else(|| "Image".to_string());
            let data = raw.raw;

            let res = RawResponse::from_vec(data).file_name(file_name);

            let res = if let Some(mime) = content_type { res.content_type(mime) } else { res };

            Ok(res)
        },
        None => Err("Please input a file."),
    }
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .attach(static_resources_initializer!("html-image-uploader" => "examples/front-end/html/image-uploader.html"))
        .mount("/", routes![index])
        .mount("/", routes![upload])
}
