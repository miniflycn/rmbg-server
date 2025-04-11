// 在文件顶部添加这一行
mod rmbg;


use once_cell::sync::Lazy;
use rmbg::Rmbg;
// use rmbg::Rmbg;
use serde_derive::{Deserialize, Serialize};
use warp::{Filter, Rejection, Reply};
use image::{DynamicImage, ImageFormat};
use std::io::Cursor;
use std::time::Instant;

#[derive(Deserialize)]
struct ImageInput {
    base64: String,
}

#[derive(Serialize)]
struct ImageOutput {
    base64: String,
    process_time: u128,
}

#[derive(Debug)]
pub struct ImageError(pub String);

impl warp::reject::Reject for ImageError {}

use base64::{engine::general_purpose, Engine as _};

fn get_base64_type(type_info: &str) -> Result<ImageFormat, ImageError> {
    match type_info {
        "data:image/jpeg" => Ok(ImageFormat::Jpeg),
        "data:image/png" => Ok(ImageFormat::Png),
        "data:image/webp" => Ok(ImageFormat::WebP),
        _ => Err(ImageError("不支持的图片格式".to_string()))
    }
}
 
fn base64_to_image(base64_string: &str) -> Result<DynamicImage, ImageError> {
    let mut parts = base64_string.split(";base64,");
    let type_info = parts.next().ok_or_else(|| ImageError("无效的 base64 格式".to_string()))?;
    let base64 = parts.next().ok_or_else(|| ImageError("无效的 base64 格式".to_string()))?;
    
    let decoded_data = general_purpose::STANDARD
        .decode(base64)
        .map_err(|e| ImageError(format!("base64 解码失败: {}", e)))?;
    
    let mut buf = Cursor::new(decoded_data);
    let img = image::load(&mut buf, get_base64_type(type_info)?)
        .map_err(|e| ImageError(format!("图片加载失败: {}", e)))?;
    
    Ok(img)
}

fn image_to_base64(img: &DynamicImage) -> Result<String, ImageError> {
    let mut image_data = Vec::new();
    img.write_to(&mut Cursor::new(&mut image_data), ImageFormat::Png)
        .map_err(|e| ImageError(format!("图片编码失败: {}", e)))?;
        
    let res_base64 = general_purpose::STANDARD.encode(image_data);
    Ok(format!("data:image/png;base64,{}", res_base64))
}

static RMGB: Lazy<Rmbg> = Lazy::new(|| {
    Rmbg::new("models/model.onnx").unwrap()
});

async fn process_image(image_input: ImageInput) -> Result<impl Reply, Rejection> {
    let now = Instant::now();
    
    let img = base64_to_image(&image_input.base64)
        .map_err(warp::reject::custom)?;
        
    let img_without_bg = RMGB.remove_background(&img)
        .map_err(|e| warp::reject::custom(ImageError(e.to_string())))?;
        
    let res_base64 = image_to_base64(&img_without_bg)
        .map_err(warp::reject::custom)?;
        
    let process_time = now.elapsed().as_millis();
    println!("处理耗时: {} ms", process_time);
    
    Ok(warp::reply::json(&ImageOutput {
        base64: res_base64,
        process_time,
    }))
}

#[tokio::main]
async fn main() {
    let route = warp::post()
        .and(warp::path("run"))
        .and(warp::body::json())
        .and_then(process_image);

    println!("服务器启动在 http://127.0.0.1:3030");
    warp::serve(route)
        .run(([127, 0, 0, 1], 3030))
        .await;
}