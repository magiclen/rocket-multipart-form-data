#[macro_use]
extern crate rocket_include_static_resources;

extern crate rocket_raw_response;

#[macro_use]
extern crate rocket;

use rocket::config::Config;
use rocket::data::{Limits, ToByteUnit};
use rocket::form::{Contextual, Form};
use rocket::fs::TempFile;
use rocket::http::ContentType;

use rocket_raw_response::RawResponsePro;

#[derive(Debug, FromForm)]
struct MultipartFormData<'v> {
    #[field(validate = ext(ContentType::JPEG))] // only JPEG, cannot be other image types (yet?)
    #[field(validate = len(..32.mebibytes()))]
    image: TempFile<'v>, // need to be `TempFile<'_>`, cannot be `Vec<u8>` (yet?)
}

static_response_handler! {
    "/" => index => "html-image-uploader",
}

#[post("/upload", data = "<data>")]
fn upload<'r>(
    data: Form<Contextual<'r, MultipartFormData<'r>>>,
) -> Result<RawResponsePro<'r>, &'static str> {
    match data.into_inner().value {
        Some(data) => {
            let image_file = data.image;
            let file_name = image_file.name().unwrap_or("Image").to_string();

            Ok(RawResponsePro::from_temp_file(image_file, Some(file_name), None))
        }
        None => return Err("Incorrect."),
    }
}

#[launch]
fn rocket() -> _ {
    let config = Config {
        limits: Limits::default().limit("file", 33.mebibytes()).limit("data-form", 33.mebibytes()),
        ..Config::default()
    };

    rocket::custom(config)
        .attach(static_resources_initializer!("html-image-uploader" => "examples/front-end/html/image-uploader.html"))
        .mount("/", routes![index])
        .mount("/", routes![upload])
}
