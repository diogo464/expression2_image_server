use axum::{
    extract::{Path, Query},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Router,
};
use bytes::BytesMut;
use clap::Parser;
use futures::StreamExt;
use image;
use percent_encoding;
use reqwest;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use thiserror::Error;

#[derive(Parser)]
#[clap(version = "0.2.0", author = "diogo464")]
struct Params {
    /// The address the http server should bind to
    #[clap(long, default_value = "0.0.0.0")]
    address: String,
    /// The port the http server should listen on
    #[clap(short, long, default_value = "8080")]
    port: u16,
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

impl IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        match self {
            Error::InvalidURL => (StatusCode::BAD_REQUEST, "Invalid URL".to_string()),
            Error::RequestTimeOut => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Request to remote server timed out".to_string(),
            ),
            Error::ImageDoesntExist => (
                StatusCode::NOT_FOUND,
                "The requested image doesnt exist".to_string(),
            ),
            Error::RequestedImageToLarge => (
                StatusCode::BAD_REQUEST,
                "Requested image is to large".to_string(),
            ),
            Error::InvalidImage => (
                StatusCode::BAD_REQUEST,
                "The requested image was invalid".to_string(),
            ),
            Error::InternalError(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Internal error: {e}"),
            ),
        }
        .into_response()
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Serialize, Deserialize)]
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

#[axum::debug_handler]
async fn local_image(
    Path(filename): Path<String>,
    Query(query): Query<ImageQuery>,
) -> Result<Vec<u8>> {
    let (width, height) = query.width_height();
    log::info!(
        "Requesting image {} with size {}x{}",
        filename,
        width,
        height
    );

    let img = image::open(std::path::Path::new(IMAGES_PATH).join(filename.as_str()))
        .map_err(|_| Error::ImageDoesntExist)?;
    let expression2_data = image_to_expression2_format(&img, width, height);

    Ok(expression2_data)
}

#[axum::debug_handler]
async fn custom_image(Path(url): Path<String>, Query(query): Query<ImageQuery>) -> Result<Vec<u8>> {
    const MAX_IMAGE_SIZE: u64 = 1024 * 1024 * 16; //16MB

    let (width, height) = query.width_height();
    log::info!(
        "Requesting image from {} with size {}x{}",
        url,
        width,
        height
    );

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let args = Params::parse();

    // build our application with a single route
    let app = Router::new()
        .route("/image/:url", get(local_image))
        .route("/custom/:url", get(custom_image));

    tokio::spawn(async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install CTRL+C signal handler");
        std::process::exit(0);
    });

    let addr = format!("{}:{}", args.address, args.port).parse::<SocketAddr>()?;
    log::info!("Listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();

    Ok(())
}
