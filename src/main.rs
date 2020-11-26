use actix_web::{web, App, HttpServer};
use image;
use percent_encoding;
use reqwest;
use serde::Deserialize;
use std::path::Path;

static IMAGES_PATH: &str = "images";

use clap::Clap;

/// URL Shortener
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

pub enum Error {
    ImageNotFound,
    InternalError,
}

#[derive(Deserialize, Debug)]
pub struct ImageQuery {
    width: Option<u32>,
    height: Option<u32>,
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    let params = Params::parse();

    HttpServer::new(move || {
        App::new()
            .route("/image/{name}", web::get().to(handlers::image))
            .route("/custom/{url}", web::get().to(handlers::remote_image))
    })
    .bind(format!("{}:{}", params.bind_ipaddr, params.bind_port))?
    .run()
    .await
}

mod handlers {
    use super::*;
    use actix_web::{http::header::ContentType, HttpResponse, Responder};

    fn image_to_expression2_format(img: &image::DynamicImage, width: u32, height: u32) -> Vec<u8> {
        let resized = img.resize_exact(width, height, image::imageops::FilterType::Nearest);
        let buffer = resized.to_rgb8();
        let mut data =
            Vec::<u8>::with_capacity((buffer.width() * buffer.height() * 3 + 12) as usize);
        data.extend_from_slice(format!("{}x{};", buffer.width(), buffer.height()).as_bytes());
        for (_, _, color) in buffer.enumerate_pixels() {
            data.push(color[0]);
            data.push(color[1]);
            data.push(color[2]);
        }
        data
    }

    pub async fn image(name: web::Path<String>, query: web::Query<ImageQuery>) -> impl Responder {
        let width = query.width.unwrap_or(512);
        let height = query.height.unwrap_or(512);
        let img = image::open(Path::new(IMAGES_PATH).join(name.as_str())).unwrap();
        let expression2_data = image_to_expression2_format(&img, width, height);

        println!("Image requested : {:?}", query);
        HttpResponse::Ok()
            .set(ContentType::plaintext())
            .body(expression2_data)
    }

    pub async fn remote_image(
        url: web::Path<String>,
        query: web::Query<ImageQuery>,
    ) -> impl Responder {
        let width = query.width.unwrap_or(512);
        let height = query.height.unwrap_or(512);
        let url = String::from(
            percent_encoding::percent_decode_str(&url)
                .decode_utf8()
                .unwrap(),
        );
        let req = reqwest::get(&url).await.unwrap();
        let body = req.bytes().await.unwrap();
        let img = image::load_from_memory(&body).unwrap();
        let expression2_data = image_to_expression2_format(&img, width, height);

        HttpResponse::Ok()
            .set(ContentType::plaintext())
            .body(expression2_data)
    }
}
