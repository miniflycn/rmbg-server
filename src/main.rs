use once_cell::sync::Lazy;
use rmbg::Rmbg;
use serde_derive::{Deserialize, Serialize};
use warp::Filter;
use image::{DynamicImage, ImageFormat};
use std::io::Cursor;
use std::time::Instant;

#[derive(Deserialize, Serialize)]
struct ImageInput {
    base64: String,
}
use base64::{engine::general_purpose, Engine as _};

pub fn get_base64_type(type_info: &str) -> image::ImageFormat {
    println!("type_info == {}", type_info);
    if type_info == "data:image/jpeg" {
        return image::ImageFormat::Jpeg
    } else if type_info == "data:image/png" {
        return image::ImageFormat::Png
    } else if type_info == "data:image/webp" {
        return image::ImageFormat::WebP
    }
    panic!("invalid file type")
}
 
fn base64_to_image(base64_string: String) -> DynamicImage {
    let mut iter = base64_string.split(";base64,");
    let type_info = iter.next().unwrap();
    let base64 = iter.next().unwrap();
    let decoded_data = general_purpose::STANDARD.decode(base64).expect("Invalid base64 string");
    let mut buf = Cursor::new(decoded_data);
    let img = image::load(&mut buf, get_base64_type(type_info)).expect("Could not create image from bytes");
    return img
}

fn image_to_base64(img: DynamicImage) -> String {
    let mut image_data: Vec<u8> = Vec::new();
    img.write_to(&mut Cursor::new(&mut image_data), ImageFormat::Png)
        .unwrap();
    let res_base64 = general_purpose::STANDARD.encode(image_data);
    format!("data:image/png;base64,{}", res_base64)
}
static RMGB: Lazy<Rmbg> = Lazy::new(|| {
    Rmbg::new("models/model_quantized.onnx").unwrap()
});
#[tokio::main]
async fn main() {
    // pretty_env_logger::init();

    let route = warp::post()
        .and(warp::path("run"))
        .and(warp::body::json())
        .map(|image_input: ImageInput| {
            let now = Instant::now();
            let img = base64_to_image(image_input.base64);
            let img_without_bg = RMGB.remove_background(&img).unwrap();
            let res_base64 = image_to_base64(img_without_bg);
            let image_out = ImageInput {
                base64: res_base64
            };
            let end = now.elapsed().as_millis();
            println!("程序运行了 {:?} ms",end);
            warp::reply::json(&image_out)
        });

    warp::serve(route)
        .run(([127, 0, 0, 1], 3030)).await
}