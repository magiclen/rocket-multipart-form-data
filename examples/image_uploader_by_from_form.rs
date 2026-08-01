#[macro_use]
extern crate rocket_include_static_resources;

#[macro_use]
extern crate rocket;

use rocket::{
    config::Config,
    data::{Limits, ToByteUnit},
    form::{Contextual, Form},
    fs::TempFile,
    http::ContentType,
};
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
async fn upload<'r>(
    data: Form<Contextual<'r, MultipartFormData<'r>>>,
) -> Result<RawResponsePro<'r>, String> {
    match data.into_inner().value {
        Some(data) => {
            let image_file = data.image;

            RawResponsePro::from_temp_file(image_file).await.map_err(|error| error.to_string())
        },
        None => Err(String::from("Incorrect.")),
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
