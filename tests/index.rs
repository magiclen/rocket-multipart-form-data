#![feature(plugin)]
#![plugin(rocket_codegen)]

extern crate rocket;
extern crate rocket_multipart_form_data;

use std::env;

use rocket::Data;
use rocket::http::ContentType;

use rocket_multipart_form_data::MultipartFormData;

#[post("/", data = "<data>")]
fn index(content_type: &ContentType, data: Data) -> &'static str
{
    let multipart_form_data = MultipartFormData::parse(content_type, data, 1024 * 1024, &env::temp_dir()).unwrap();


    "ok"
}