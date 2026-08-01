use std::{env, fs};

use rocket::{Data, Route, http::ContentType, local::asynchronous::Client, post, routes};
use rocket_multipart_form_data::{
    MultipartFormData, MultipartFormDataError, MultipartFormDataField, MultipartFormDataOptions,
    Repetition, mime, multer,
};

const BOUNDARY: &str = "TEST-BOUNDARY";

/// One part of a multipart/form-data body.
struct Part<'a> {
    name:         &'a str,
    file_name:    Option<&'a str>,
    content_type: Option<&'a str>,
    body:         &'a [u8],
}

fn build_body(parts: &[Part]) -> Vec<u8> {
    let mut buffer = Vec::new();

    for part in parts {
        buffer.extend_from_slice(format!("--{BOUNDARY}\r\n").as_bytes());
        buffer.extend_from_slice(
            format!("Content-Disposition: form-data; name=\"{}\"", part.name).as_bytes(),
        );

        if let Some(file_name) = part.file_name {
            buffer.extend_from_slice(format!("; filename=\"{file_name}\"").as_bytes());
        }

        buffer.extend_from_slice(b"\r\n");

        if let Some(content_type) = part.content_type {
            buffer.extend_from_slice(format!("Content-Type: {content_type}\r\n").as_bytes());
        }

        buffer.extend_from_slice(b"\r\n");
        buffer.extend_from_slice(part.body);
        buffer.extend_from_slice(b"\r\n");
    }

    buffer.extend_from_slice(format!("--{BOUNDARY}--\r\n").as_bytes());

    buffer
}

async fn create_client(routes: Vec<Route>) -> Client {
    Client::tracked(rocket::build().mount("/", routes)).await.unwrap()
}

/// Send a multipart/form-data request and read the response body as a string.
async fn post(client: &Client, uri: &'static str, parts: &[Part<'_>]) -> String {
    client
        .post(uri)
        .header(ContentType::new("multipart", "form-data").with_params(("boundary", BOUNDARY)))
        .body(build_body(parts))
        .dispatch()
        .await
        .into_string()
        .await
        .unwrap()
}

#[post("/text", data = "<data>")]
async fn text_route(content_type: &ContentType, data: Data<'_>) -> String {
    let options = MultipartFormDataOptions::with_multipart_form_data_fields(vec![
        MultipartFormDataField::text("name"),
    ]);

    let mut form = match MultipartFormData::parse(content_type, data, options).await {
        Ok(form) => form,
        Err(err) => return format!("ERROR: {err}"),
    };

    let fields = form.texts.remove("name").unwrap();

    format!("{}/{}", fields.len(), fields[0].text)
}

#[rocket::async_test]
async fn parse_text_field() {
    let client = create_client(routes![text_route]).await;

    let output = post(&client, "/text", &[Part {
        name:         "name",
        file_name:    None,
        content_type: None,
        body:         "Magic Len".as_bytes(),
    }])
    .await;

    assert_eq!("1/Magic Len", output);
}

#[post("/raw", data = "<data>")]
async fn raw_route(content_type: &ContentType, data: Data<'_>) -> String {
    let options = MultipartFormDataOptions::with_multipart_form_data_fields(vec![
        MultipartFormDataField::raw("fingerprint"),
    ]);

    let mut form = match MultipartFormData::parse(content_type, data, options).await {
        Ok(form) => form,
        Err(err) => return format!("ERROR: {err}"),
    };

    let mut fields = form.raw.remove("fingerprint").unwrap();

    fields.remove(0).raw.iter().map(|b| format!("{b:02X}")).collect()
}

#[rocket::async_test]
async fn parse_raw_field() {
    let client = create_client(routes![raw_route]).await;

    // The body is not valid UTF-8 on purpose. A raw field should keep the bytes as they are.
    let output = post(&client, "/raw", &[Part {
        name:         "fingerprint",
        file_name:    None,
        content_type: Some("application/octet-stream"),
        body:         &[0x00, 0xFF, 0x80, 0x1A],
    }])
    .await;

    assert_eq!("00FF801A", output);
}

#[post("/file", data = "<data>")]
async fn file_route(content_type: &ContentType, data: Data<'_>) -> String {
    let options = MultipartFormDataOptions::with_multipart_form_data_fields(vec![
        MultipartFormDataField::file("photo"),
    ]);

    let (content, file_name, path, permissions) = {
        let form = match MultipartFormData::parse(content_type, data, options).await {
            Ok(form) => form,
            Err(err) => return format!("ERROR: {err}"),
        };

        let field = &form.files.get("photo").unwrap()[0];

        #[cfg(unix)]
        let permissions = {
            use std::os::unix::fs::PermissionsExt;

            format!("{:o}", fs::metadata(&field.path).unwrap().permissions().mode() & 0o777)
        };
        #[cfg(not(unix))]
        let permissions = String::from("unsupported");

        (
            fs::read(&field.path).unwrap(),
            field.file_name.clone().unwrap(),
            field.path.clone(),
            permissions,
        )
    };

    // `form` has been dropped, so its temporary file should have been removed.
    format!(
        "{}/{}/{}/{}",
        String::from_utf8(content).unwrap(),
        file_name,
        permissions,
        path.exists()
    )
}

#[rocket::async_test]
async fn parse_file_field() {
    let client = create_client(routes![file_route]).await;

    let output = post(&client, "/file", &[Part {
        name:         "photo",
        file_name:    Some("photo.png"),
        content_type: Some("image/png"),
        body:         "Hello world!".as_bytes(),
    }])
    .await;

    #[cfg(unix)]
    assert_eq!("Hello world!/photo.png/600/false", output);
    #[cfg(not(unix))]
    assert_eq!("Hello world!/photo.png/unsupported/false", output);
}

#[post("/empty-file-input", data = "<data>")]
async fn empty_file_input_route(content_type: &ContentType, data: Data<'_>) -> String {
    let temporary_dir = env::temp_dir().join("rocket-multipart-form-data-empty-file-input");

    fs::create_dir_all(&temporary_dir).unwrap();

    let options = MultipartFormDataOptions {
        temporary_dir: temporary_dir.clone(),
        allowed_fields: vec![
            MultipartFormDataField::file("photo").content_type(Some(mime::IMAGE_STAR)),
        ],
        ..MultipartFormDataOptions::default()
    };

    let result = MultipartFormData::parse(content_type, data, options).await;
    let left_behind = fs::read_dir(&temporary_dir).unwrap().count();

    fs::remove_dir_all(&temporary_dir).unwrap();

    match result {
        Ok(form) => format!("{}/{left_behind}", form.files.contains_key("photo")),
        Err(err) => format!("ERROR: {err}/{left_behind}"),
    }
}

#[rocket::async_test]
async fn an_empty_browser_file_input_is_ignored_without_leaving_a_temporary_file() {
    let client = create_client(routes![empty_file_input_route]).await;

    let output = post(&client, "/empty-file-input", &[Part {
        name:         "photo",
        file_name:    Some(""),
        content_type: Some("application/octet-stream"),
        body:         b"",
    }])
    .await;

    assert_eq!("false/0", output);
}

#[post("/content-type", data = "<data>")]
async fn content_type_route(content_type: &ContentType, data: Data<'_>) -> String {
    let options = MultipartFormDataOptions::with_multipart_form_data_fields(vec![
        MultipartFormDataField::raw("photo").content_type(Some(mime::IMAGE_STAR)),
    ]);

    let mut form = match MultipartFormData::parse(content_type, data, options).await {
        Ok(form) => form,
        Err(err) => return format!("ERROR: {err}"),
    };

    let mut fields = form.raw.remove("photo").unwrap();

    String::from_utf8(fields.remove(0).raw).unwrap()
}

#[rocket::async_test]
async fn content_type_filter_accepts_a_matching_star() {
    let client = create_client(routes![content_type_route]).await;

    let output = post(&client, "/content-type", &[Part {
        name:         "photo",
        file_name:    Some("photo.png"),
        content_type: Some("image/png"),
        body:         "PNG".as_bytes(),
    }])
    .await;

    assert_eq!("PNG", output);
}

#[rocket::async_test]
async fn content_type_filter_rejects_a_mismatch() {
    let client = create_client(routes![content_type_route]).await;

    let output = post(&client, "/content-type", &[Part {
        name:         "photo",
        file_name:    Some("photo.txt"),
        content_type: Some("text/plain"),
        body:         "TXT".as_bytes(),
    }])
    .await;

    assert_eq!("ERROR: The data type of field `photo` is incorrect.", output);
}

#[rocket::async_test]
async fn content_type_filter_rejects_a_field_without_a_content_type_header() {
    let client = create_client(routes![content_type_route]).await;

    let output = post(&client, "/content-type", &[Part {
        name:         "photo",
        file_name:    Some("photo.png"),
        content_type: None,
        body:         "PNG".as_bytes(),
    }])
    .await;

    assert_eq!("ERROR: The data type of field `photo` is incorrect.", output);
}

#[post("/repetition", data = "<data>")]
async fn repetition_route(content_type: &ContentType, data: Data<'_>) -> String {
    let options = MultipartFormDataOptions::with_multipart_form_data_fields(vec![
        MultipartFormDataField::text("email").repetition(Repetition::fixed(3)),
    ]);

    let mut form = match MultipartFormData::parse(content_type, data, options).await {
        Ok(form) => form,
        Err(err) => return format!("ERROR: {err}"),
    };

    let fields = form.texts.remove("email").unwrap();

    fields.into_iter().map(|field| field.text).collect::<Vec<String>>().join(",")
}

#[rocket::async_test]
async fn repetition_limits_the_collected_occurrences() {
    let client = create_client(routes![repetition_route]).await;

    let email = |body: &'static str| Part {
        name:         "email",
        file_name:    None,
        content_type: None,
        body:         body.as_bytes(),
    };

    // The fourth occurrence should be ignored because the field is allowed three times only.
    let output = post(&client, "/repetition", &[
        email("a@magiclen.org"),
        email("b@magiclen.org"),
        email("c@magiclen.org"),
        email("d@magiclen.org"),
    ])
    .await;

    assert_eq!("a@magiclen.org,b@magiclen.org,c@magiclen.org", output);
}

#[post("/duplicate-repetition", data = "<data>")]
async fn duplicate_repetition_route(content_type: &ContentType, data: Data<'_>) -> String {
    let options = MultipartFormDataOptions::with_multipart_form_data_fields(vec![
        MultipartFormDataField::text("email").repetition(Repetition::fixed(2)),
        MultipartFormDataField::text("email"),
    ]);

    let mut form = match MultipartFormData::parse(content_type, data, options).await {
        Ok(form) => form,
        Err(err) => return format!("ERROR: {err}"),
    };

    let fields = form.texts.remove("email").unwrap();

    fields.into_iter().map(|field| field.text).collect::<Vec<String>>().join(",")
}

#[rocket::async_test]
async fn repetitions_for_duplicate_field_definitions_add_up() {
    let client = create_client(routes![duplicate_repetition_route]).await;

    let email = |body: &'static str| Part {
        name:         "email",
        file_name:    None,
        content_type: None,
        body:         body.as_bytes(),
    };

    let output = post(&client, "/duplicate-repetition", &[
        email("a@magiclen.org"),
        email("b@magiclen.org"),
        email("c@magiclen.org"),
        email("d@magiclen.org"),
    ])
    .await;

    assert_eq!("a@magiclen.org,b@magiclen.org,c@magiclen.org", output);
}

#[post("/infinite-repetition", data = "<data>")]
async fn infinite_repetition_route(content_type: &ContentType, data: Data<'_>) -> String {
    let options = MultipartFormDataOptions::with_multipart_form_data_fields(vec![
        MultipartFormDataField::text("email").repetition(Repetition::infinite()),
    ]);

    let mut form = match MultipartFormData::parse(content_type, data, options).await {
        Ok(form) => form,
        Err(err) => return format!("ERROR: {err}"),
    };

    let fields = form.texts.remove("email").unwrap();

    fields.into_iter().map(|field| field.text).collect::<Vec<String>>().join(",")
}

#[rocket::async_test]
async fn infinite_repetition_collects_every_occurrence() {
    let client = create_client(routes![infinite_repetition_route]).await;

    let email = |body: &'static str| Part {
        name:         "email",
        file_name:    None,
        content_type: None,
        body:         body.as_bytes(),
    };

    let output = post(&client, "/infinite-repetition", &[
        email("a@magiclen.org"),
        email("b@magiclen.org"),
        email("c@magiclen.org"),
        email("d@magiclen.org"),
        email("e@magiclen.org"),
    ])
    .await;

    assert_eq!(
        "a@magiclen.org,b@magiclen.org,c@magiclen.org,d@magiclen.org,e@magiclen.org",
        output
    );
}

#[post("/mixed", data = "<data>")]
async fn mixed_route(content_type: &ContentType, data: Data<'_>) -> String {
    let options = MultipartFormDataOptions::with_multipart_form_data_fields(vec![
        MultipartFormDataField::text("name"),
        MultipartFormDataField::raw("fingerprint"),
        MultipartFormDataField::file("photo"),
    ]);

    let mut form = match MultipartFormData::parse(content_type, data, options).await {
        Ok(form) => form,
        Err(err) => return format!("ERROR: {err}"),
    };

    let name = form.texts.remove("name").unwrap().remove(0).text;
    let fingerprint = form.raw.remove("fingerprint").unwrap().remove(0).raw;
    let photo = fs::read(&form.files.get("photo").unwrap()[0].path).unwrap();

    format!(
        "{name}/{}/{}",
        fingerprint.iter().map(|b| format!("{b:02X}")).collect::<String>(),
        String::from_utf8(photo).unwrap()
    )
}

#[rocket::async_test]
async fn mixed_field_types_in_one_request() {
    let client = create_client(routes![mixed_route]).await;

    let output = post(&client, "/mixed", &[
        Part {
            name:         "name",
            file_name:    None,
            content_type: None,
            body:         "Magic Len".as_bytes(),
        },
        Part {
            name:         "fingerprint",
            file_name:    None,
            content_type: Some("application/octet-stream"),
            body:         &[0x00, 0xFF],
        },
        Part {
            name:         "photo",
            file_name:    Some("photo.png"),
            content_type: Some("image/png"),
            body:         "Hello world!".as_bytes(),
        },
    ])
    .await;

    assert_eq!("Magic Len/00FF/Hello world!", output);
}

#[post("/size-limit", data = "<data>")]
async fn size_limit_route(content_type: &ContentType, data: Data<'_>) -> String {
    let options = MultipartFormDataOptions::with_multipart_form_data_fields(vec![
        MultipartFormDataField::text("name").size_limit(4),
    ]);

    match MultipartFormData::parse(content_type, data, options).await {
        Ok(_) => String::from("OK"),
        Err(err) => format!("ERROR: {err}"),
    }
}

#[rocket::async_test]
async fn size_limit_is_respected() {
    let client = create_client(routes![size_limit_route]).await;

    let output = post(&client, "/size-limit", &[Part {
        name:         "name",
        file_name:    None,
        content_type: None,
        body:         "Magic Len".as_bytes(),
    }])
    .await;

    assert_eq!("ERROR: The data of field `name` is too large.", output);
}

#[post("/ignored", data = "<data>")]
async fn ignored_route(content_type: &ContentType, data: Data<'_>) -> String {
    let options = MultipartFormDataOptions::with_multipart_form_data_fields(vec![
        MultipartFormDataField::text("name"),
    ]);

    let mut form = match MultipartFormData::parse(content_type, data, options).await {
        Ok(form) => form,
        Err(err) => return format!("ERROR: {err}"),
    };

    let name = form.texts.remove("name").unwrap().remove(0).text;

    format!("{}/{}", name, form.texts.len())
}

#[rocket::async_test]
async fn a_field_out_of_the_allowed_fields_is_ignored() {
    let client = create_client(routes![ignored_route]).await;

    let output = post(&client, "/ignored", &[
        Part {
            name:         "name",
            file_name:    None,
            content_type: None,
            body:         "Magic Len".as_bytes(),
        },
        Part {
            name:         "unknown",
            file_name:    None,
            content_type: None,
            body:         "whatever".as_bytes(),
        },
    ])
    .await;

    assert_eq!("Magic Len/0", output);
}

#[post("/max-data-bytes", data = "<data>")]
async fn max_data_bytes_route(content_type: &ContentType, data: Data<'_>) -> String {
    let temporary_dir = env::temp_dir().join("rocket-multipart-form-data-max-data-bytes");

    fs::create_dir_all(&temporary_dir).unwrap();

    let options = MultipartFormDataOptions {
        max_data_bytes: 1024,
        temporary_dir:  temporary_dir.clone(),
        allowed_fields: vec![MultipartFormDataField::file("photo")],
    };

    let result = MultipartFormData::parse(content_type, data, options).await;

    let left_behind = fs::read_dir(&temporary_dir).unwrap().count();

    fs::remove_dir_all(&temporary_dir).unwrap();

    match result {
        Ok(_) => format!("ok/{left_behind}"),
        Err(MultipartFormDataError::MulterError(multer::Error::StreamSizeExceeded {
            limit,
        })) => format!("stream-size-exceeded-{limit}/{left_behind}"),
        Err(err) => format!("other-error: {err}/{left_behind}"),
    }
}

#[rocket::async_test]
async fn max_data_bytes_reports_stream_size_exceeded_and_leaves_no_temporary_file() {
    let client = create_client(routes![max_data_bytes_route]).await;

    // The second part has a huge header line, so the complete multipart stream exceeds `max_data_bytes`.
    let long_name = "x".repeat(8192);

    let output = post(&client, "/max-data-bytes", &[
        Part {
            name:         "photo",
            file_name:    Some("photo.png"),
            content_type: Some("image/png"),
            body:         "Hello world!".as_bytes(),
        },
        Part {
            name:         long_name.as_str(),
            file_name:    None,
            content_type: None,
            body:         "whatever".as_bytes(),
        },
    ])
    .await;

    assert_eq!("stream-size-exceeded-1024/0", output);
}
