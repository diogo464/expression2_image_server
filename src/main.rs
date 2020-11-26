#![feature(proc_macro_hygiene, decl_macro)]
#[macro_use]
extern crate rocket;
use bytes::BytesMut;
use clap::Clap;
use image;
use percent_encoding;
use reqwest;
use rocket::request::Form;
use std::path::Path;
use thiserror::Error;
use tokio::stream::StreamExt;

#[derive(Clap)]
#[clap(version = "1.0", author = "diogo464")]
struct Params {
    /// The address the http server should bind to
    #[clap(long = "ipaddr", default_value = "0.0.0.0")]
    bind_ipaddr: String,
    /// The port the http server should listen on
    #[clap(short = 'p', long = "port", default_value = "8080")]
    bind_port: u16,
}

static IMAGES_PATH: &str = "images";

#[derive(Debug, Error)]
pub enum Error {
    #[error("Invalid URL")]
    InvalidURL,
    #[error("Request to remote server timed out")]
    RequestTimeOut,
    #[error("The requested image doesnt exist")]
    ImageDoesntExist,
    #[error("Requested image is to large")]
    RequestedImageToLarge,
    #[error("The requested image was invalid")]
    InvalidImage,
    #[error("Internal error")]
    InternalError(Box<dyn std::error::Error + Send>),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl<'r, 'o: 'r> rocket::response::Responder<'r, 'o> for Error {
    fn respond_to(self, _request: &'r rocket::request::Request) -> rocket::response::Result<'o> {
        use rocket::http::{ContentType, Status};
        use rocket::response::Response;
        use std::io::Cursor;

        let status = match &self {
            Self::InvalidURL => Status::BadRequest,
            Self::RequestTimeOut => Status::BadRequest,
            Self::ImageDoesntExist => Status::BadRequest,
            Self::RequestedImageToLarge => Status::BadRequest,
            Self::InvalidImage => Status::BadRequest,
            Self::InternalError(e) => {
                eprintln!("Internal error : {:#?}", e);
                Status::InternalServerError
            }
        };

        let msg = self.to_string();
        Response::build()
            .status(status)
            .header(ContentType::Plain)
            .sized_body(msg.len(), Cursor::new(msg))
            .ok()
    }
}

#[derive(Debug, FromForm)]
pub struct ImageQuery {
    width: Option<u32>,
    height: Option<u32>,
}

impl ImageQuery {
    fn width_height(&self) -> (u32, u32) {
        (self.width.unwrap_or(512), self.height.unwrap_or(512))
    }
}

fn image_to_expression2_format(img: &image::DynamicImage, width: u32, height: u32) -> Vec<u8> {
    let resized = img.resize_exact(width, height, image::imageops::FilterType::Nearest);
    let buffer = resized.to_rgb8();
    let mut data = Vec::<u8>::with_capacity((buffer.width() * buffer.height() * 3 + 12) as usize);
    data.extend_from_slice(format!("{}x{};", buffer.width(), buffer.height()).as_bytes());
    for (_, _, color) in buffer.enumerate_pixels() {
        data.push(color[0]);
        data.push(color[1]);
        data.push(color[2]);
    }
    data
}

#[get("/custom/<url>?<query..>")]
async fn custom_image(url: String, query: Form<ImageQuery>) -> Result<Vec<u8>> {
    const MAX_IMAGE_SIZE: u64 = 1024 * 1024 * 16; //16MB

    let (width, height) = query.width_height();

    let url = String::from(
        percent_encoding::percent_decode_str(&url)
            .decode_utf8()
            .map_err(|_| Error::InvalidURL)?,
    );
    let req = reqwest::get(&url)
        .await
        .map_err(|e| Error::InternalError(Box::new(e)))?;
    if let Some(lenght) = req.content_length() {
        if lenght > MAX_IMAGE_SIZE {
            return Err(Error::RequestedImageToLarge);
        }
    }

    let body = {
        let mut data = BytesMut::with_capacity(MAX_IMAGE_SIZE as usize);
        let mut stream = req.bytes_stream();
        while let Some(partial) = stream.next().await {
            let partial = partial.map_err(|e| Error::InternalError(Box::new(e)))?;
            let remaining = (MAX_IMAGE_SIZE as usize).saturating_sub(data.len());
            if remaining == 0 {
                break;
            }
            data.extend_from_slice(partial.slice(0..remaining.min(partial.len())).as_ref());
        }
        data.freeze()
    };
    let img = image::load_from_memory(&body).map_err(|_| Error::InvalidImage)?;
    let expression2_data = image_to_expression2_format(&img, width, height);

    Ok(expression2_data)
}

#[get("/image/<filename>?<query..>")]
fn local_image(filename: String, query: Form<ImageQuery>) -> Result<Vec<u8>> {
    let (width, height) = query.width_height();
    let img = image::open(Path::new(IMAGES_PATH).join(filename.as_str()))
        .map_err(|_| Error::ImageDoesntExist)?;
    let expression2_data = image_to_expression2_format(&img, width, height);

    Ok(expression2_data)
}

#[launch]
fn rocket() -> rocket::Rocket {
    use rocket::config::Config;

    let params = Params::parse();
    let config = Config {
        address: params.bind_ipaddr.parse().expect("Invalid bind address"),
        port: params.bind_port,
        ..Config::debug_default()
    };
    rocket::custom(config).mount("/", routes![custom_image, local_image])
}
