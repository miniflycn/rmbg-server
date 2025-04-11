mod rmbg;

use once_cell::sync::Lazy;
use rmbg::Rmbg;
use serde_derive::{Deserialize, Serialize};
use warp::{Filter, Rejection, Reply, http::StatusCode};
use image::{DynamicImage, ImageFormat};
use std::io::Cursor;
use std::time::Instant;
use std::convert::Infallible;
use base64::{engine::general_purpose, Engine as _};

#[derive(Deserialize)]
struct ImageInput {
    base64: String,
}

#[derive(Serialize)]
struct ImageOutput {
    base64: String,
    process_time: u128,
}

#[derive(Serialize)]
struct ErrorResponse {
    code: u16,
    message: String,
}

#[derive(Debug)]
pub struct ImageError(pub String);

impl warp::reject::Reject for ImageError {}

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
    println!("正在加载模型...");
    let model = Rmbg::new("models/model.onnx").unwrap();
    println!("模型加载完成");
    model
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

// 错误处理中间件
async fn handle_rejection(err: Rejection) -> Result<impl Reply, Infallible> {
    let (code, message) = if err.is_not_found() {
        (StatusCode::NOT_FOUND, "未找到资源".to_string())
    } else if let Some(e) = err.find::<ImageError>() {
        (StatusCode::BAD_REQUEST, e.0.clone())
    } else if let Some(e) = err.find::<warp::filters::body::BodyDeserializeError>() {
        (StatusCode::BAD_REQUEST, format!("请求格式错误: {}", e))
    } else {
        eprintln!("未处理的错误: {:?}", err);
        (StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误".to_string())
    };

    let json = warp::reply::json(&ErrorResponse {
        code: code.as_u16(),
        message,
    });

    Ok(warp::reply::with_status(json, code))
}

#[tokio::main]
async fn main() {
    // 添加CORS支持
    let cors = warp::cors()
        .allow_any_origin()
        .allow_methods(vec!["POST", "GET", "OPTIONS"])
        .allow_headers(vec!["Content-Type"]);

    let health_route = warp::path("health")
        .and(warp::get())
        .map(|| "OK");

    let api_route = warp::path("run")
        .and(warp::post())
        .and(warp::body::json())
        .and_then(process_image);

    let routes = health_route
        .or(api_route)
        .with(cors)
        .recover(handle_rejection);

    println!("服务器启动在 http://127.0.0.1:3030");
    println!("健康检查: GET /health");
    println!("图片处理: POST /run");
    
    warp::serve(routes)
        .run(([127, 0, 0, 1], 3030))
        .await;
}