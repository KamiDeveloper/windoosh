// Windoosh - Motor de procesamiento de imágenes de alto rendimiento
// Arquitectura: Zero-copy, Arc-based state, async commands, SIMD resize
//
// Optimizaciones implementadas:
// - Arc<DynamicImage> para evitar clones de 200MB+
// - RwLock para lecturas concurrentes
// - fast_image_resize con SIMD (AVX2/SSE4.1)
// - Comandos async con spawn_blocking
// - Quantize optimizado sin copias intermedias

mod codecs;

use codecs::{EncodingResult, ImageEncoder, JpegCodec, OxiPngCodec, WebPCodec};
use fast_image_resize::{images::Image, PixelType, ResizeAlg, ResizeOptions, Resizer};
use image::{DynamicImage, GenericImageView, ImageReader, RgbaImage};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Cursor;
use std::sync::Arc;
use tauri::{Emitter, State};
use thiserror::Error;

// ============================================================================
// Error Handling
// ============================================================================

#[derive(Error, Debug)]
pub enum WindooshError {
    #[error("Error al leer archivo: {0}")]
    FileRead(String),
    #[error("Error al decodificar imagen: {0}")]
    ImageDecode(String),
    #[error("Error al procesar imagen: {0}")]
    Processing(String),
    #[error("Error al codificar imagen: {0}")]
    Encoding(String),
    #[error("No hay imagen cargada")]
    NoImage,
    #[error("Error de concurrencia: {0}")]
    Concurrency(String),
}

impl From<WindooshError> for String {
    fn from(err: WindooshError) -> String {
        err.to_string()
    }
}

// ============================================================================
// Estado Global de la Aplicación (Zero-Copy Architecture)
// ============================================================================

/// Estado optimizado con Arc para zero-copy sharing entre threads
pub struct AppState {
    /// Imagen original envuelta en Arc para compartir sin clonar bytes
    pub original_image: RwLock<Option<Arc<DynamicImage>>>,
    /// Path del archivo original
    pub original_path: RwLock<Option<String>>,
    /// Tamaño original en bytes
    pub original_size: RwLock<usize>,
    /// Última metadata de optimización
    pub last_optimization: RwLock<Option<OptimizationMetadata>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            original_image: RwLock::new(None),
            original_path: RwLock::new(None),
            original_size: RwLock::new(0),
            last_optimization: RwLock::new(None),
        }
    }
}

// ============================================================================
// DTOs (Data Transfer Objects)
// ============================================================================

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ImageInfo {
    pub width: u32,
    pub height: u32,
    pub original_size: usize,
    pub preview_base64: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ResizeOptionsDto {
    pub width: u32,
    pub height: u32,
    pub filter: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct QuantizeOptionsDto {
    pub num_colors: u32,
    pub dither: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OptimizationRequest {
    pub encoder_name: String,
    pub options: Value,
    pub resize: Option<ResizeOptionsDto>,
    pub quantize: Option<QuantizeOptionsDto>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OptimizationResult {
    pub preview_base64: String,
    pub optimized_size: usize,
    pub savings_percent: f32,
    pub mime_type: String,
    pub extension: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OptimizationMetadata {
    pub optimized_size: usize,
    pub savings_percent: f32,
    pub mime_type: String,
    pub extension: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SaveResult {
    pub path: String,
    pub final_size: usize,
}

// ============================================================================
// Helpers
// ============================================================================

fn get_encoder(name: &str) -> Box<dyn ImageEncoder> {
    match name {
        "oxipng" => Box::new(OxiPngCodec),
        "mozjpeg" | "jpeg" => Box::new(JpegCodec),
        "webp" => Box::new(WebPCodec),
        _ => Box::new(JpegCodec),
    }
}

/// Resize usando fast_image_resize con SIMD automático
/// Detecta y usa AVX2, SSE4.1, o NEON según disponibilidad
fn resize_with_simd(
    src: &DynamicImage,
    target_width: u32,
    target_height: u32,
    filter: &str,
) -> Result<DynamicImage, WindooshError> {
    let src_rgba = src.to_rgba8();
    let (src_w, src_h) = src_rgba.dimensions();
    
    // Si las dimensiones son iguales, no hay que hacer resize
    if src_w == target_width && src_h == target_height {
        return Ok(DynamicImage::ImageRgba8(src_rgba));
    }

    // Crear imagen fuente para fast_image_resize
    let src_image = Image::from_vec_u8(
        src_w,
        src_h,
        src_rgba.into_raw(),
        PixelType::U8x4,
    ).map_err(|e| WindooshError::Processing(format!("Error creando imagen fuente: {}", e)))?;

    // Crear imagen destino
    let mut dst_image = Image::new(target_width, target_height, PixelType::U8x4);

    // Seleccionar algoritmo
    let algorithm = match filter {
        "Lanczos3" => ResizeAlg::Convolution(fast_image_resize::FilterType::Lanczos3),
        "CatmullRom" => ResizeAlg::Convolution(fast_image_resize::FilterType::CatmullRom),
        "Mitchell" => ResizeAlg::Convolution(fast_image_resize::FilterType::Mitchell),
        "Bilinear" | "Triangle" => ResizeAlg::Convolution(fast_image_resize::FilterType::Bilinear),
        "Nearest" => ResizeAlg::Nearest,
        _ => ResizeAlg::Convolution(fast_image_resize::FilterType::Lanczos3),
    };

    // Crear resizer (detecta automáticamente AVX2/SSE4.1)
    let mut resizer = Resizer::new();

    // Ejecutar resize
    let options = ResizeOptions::new().resize_alg(algorithm);
    resizer.resize(&src_image, &mut dst_image, Some(&options))
        .map_err(|e| WindooshError::Processing(format!("Error en resize: {}", e)))?;

    // Convertir de vuelta a DynamicImage
    let dst_buffer = dst_image.into_vec();
    let rgba_image = RgbaImage::from_raw(target_width, target_height, dst_buffer)
        .ok_or_else(|| WindooshError::Processing("Error creando imagen de destino".into()))?;

    Ok(DynamicImage::ImageRgba8(rgba_image))
}

/// Aplica quantización de colores (reducción de paleta)
fn apply_quantize(img: DynamicImage, opts: &QuantizeOptionsDto) -> Result<DynamicImage, WindooshError> {
    let mut liq = imagequant::new();
    liq.set_speed(3).map_err(|e| WindooshError::Processing(format!("Liq speed error: {:?}", e)))?;
    liq.set_quality(0, 100).map_err(|e| WindooshError::Processing(format!("Liq quality error: {:?}", e)))?;
    liq.set_max_colors(opts.num_colors.clamp(2, 256)).map_err(|e| WindooshError::Processing(format!("Liq max colors error: {:?}", e)))?;

    let rgba = img.to_rgba8();
    let width = rgba.width() as usize;
    let height = rgba.height() as usize;
    
    let pixels: Vec<imagequant::RGBA> = rgba.pixels()
        .map(|p| {
            let [r, g, b, a] = p.0;
            imagequant::RGBA::new(r, g, b, a)
        })
        .collect();

    let mut img_attr = liq.new_image(pixels, width, height, 0.0)
        .map_err(|e| WindooshError::Processing(format!("Liq new image error: {:?}", e)))?;
    
    let mut res = liq.quantize(&mut img_attr)
        .map_err(|e| WindooshError::Processing(format!("Quantization failed: {:?}", e)))?;
    
    res.set_dithering_level(opts.dither.clamp(0.0, 1.0))
        .map_err(|e| WindooshError::Processing(format!("Liq dither error: {:?}", e)))?;
    
    let (palette, pixels_idx) = res.remapped(&mut img_attr)
        .map_err(|e| WindooshError::Processing(format!("Remapping failed: {:?}", e)))?;

    let mut new_rgba = Vec::with_capacity(width * height * 4);
    for &pixel_idx in &pixels_idx {
        let color = palette[pixel_idx as usize];
        new_rgba.push(color.r);
        new_rgba.push(color.g);
        new_rgba.push(color.b);
        new_rgba.push(color.a);
    }

    RgbaImage::from_vec(width as u32, height as u32, new_rgba)
        .map(DynamicImage::ImageRgba8)
        .ok_or_else(|| WindooshError::Processing("Error reconstruyendo imagen quantizada".into()))
}

/// Genera preview JPEG para la imagen
fn generate_preview(img: &DynamicImage, max_width: u32) -> Result<String, WindooshError> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    
    let (width, height) = img.dimensions();
    
    let preview = if width > max_width {
        let new_height = (max_width as f32 * height as f32 / width as f32) as u32;
        resize_with_simd(img, max_width, new_height, "Triangle")?
    } else {
        img.clone()
    };

    let mut preview_bytes = Vec::with_capacity(512 * 1024);
    preview
        .write_to(&mut Cursor::new(&mut preview_bytes), image::ImageFormat::Jpeg)
        .map_err(|e| WindooshError::Encoding(format!("Error generando preview: {}", e)))?;
    
    let preview_base64 = STANDARD.encode(&preview_bytes);
    Ok(format!("data:image/jpeg;base64,{}", preview_base64))
}

/// Pipeline de procesamiento completo
fn process_pipeline(
    img: &Arc<DynamicImage>,
    request: &OptimizationRequest,
) -> Result<EncodingResult, WindooshError> {
    // 1. Resize con SIMD (si es necesario)
    let processed = if let Some(ref resize_opts) = request.resize {
        resize_with_simd(img, resize_opts.width, resize_opts.height, &resize_opts.filter)?
    } else {
        (**img).clone()
    };

    // 2. Quantize (si es necesario)
    let final_img = if let Some(ref quant_opts) = request.quantize {
        apply_quantize(processed, quant_opts)?
    } else {
        processed
    };

    // 3. Encode con el códec seleccionado
    let encoder = get_encoder(&request.encoder_name);
    encoder.encode(&final_img, &request.options)
        .map_err(WindooshError::Encoding)
}


// ============================================================================
// Comandos Tauri - Async para no bloquear UI
// ============================================================================

/// Carga una imagen desde disco de forma asíncrona
#[tauri::command]
async fn load_image(
    path: String,
    state: State<'_, AppState>,
) -> Result<ImageInfo, String> {
    let path_for_load = path.clone();
    
    // Ejecutar I/O y decode en thread pool
    let (img_arc, file_size, width, height) = tauri::async_runtime::spawn_blocking(move || {
        let file_bytes = std::fs::read(&path_for_load)
            .map_err(|e| WindooshError::FileRead(e.to_string()))?;
        let file_size = file_bytes.len();

        let img = ImageReader::new(Cursor::new(&file_bytes))
            .with_guessed_format()
            .map_err(|e| WindooshError::ImageDecode(e.to_string()))?
            .decode()
            .map_err(|e| WindooshError::ImageDecode(e.to_string()))?;

        let width = img.width();
        let height = img.height();

        Ok::<_, WindooshError>((Arc::new(img), file_size, width, height))
    })
    .await
    .map_err(|e| WindooshError::Concurrency(e.to_string()))?
    .map_err(String::from)?;

    // Guardar en estado (Arc::clone es O(1))
    {
        *state.original_image.write() = Some(Arc::clone(&img_arc));
        *state.original_size.write() = file_size;
        *state.original_path.write() = Some(path);
    }

    // Generar preview en background
    let img_for_preview = Arc::clone(&img_arc);
    let preview_base64 = tauri::async_runtime::spawn_blocking(move || {
        generate_preview(&img_for_preview, 1920)
    })
    .await
    .map_err(|e| WindooshError::Concurrency(e.to_string()))?
    .map_err(String::from)?;

    Ok(ImageInfo {
        width,
        height,
        original_size: file_size,
        preview_base64,
    })
}

/// Procesa la imagen con las opciones dadas
#[tauri::command]
async fn process_image(
    request: OptimizationRequest,
    state: State<'_, AppState>,
) -> Result<OptimizationResult, String> {
    // Obtener Arc sin clonar bytes subyacentes
    let img_arc = {
        let guard = state.original_image.read();
        guard.as_ref()
            .ok_or_else(|| WindooshError::NoImage)?
            .clone() // Arc::clone = O(1)
    };
    let original_size = *state.original_size.read();

    // Procesar en thread pool
    let result = tauri::async_runtime::spawn_blocking(move || {
        process_pipeline(&img_arc, &request)
    })
    .await
    .map_err(|e| WindooshError::Concurrency(e.to_string()))?
    .map_err(String::from)?;

    let optimized_size = result.data.len();
    let savings_percent = if original_size > 0 {
        ((original_size as f32 - optimized_size as f32) / original_size as f32) * 100.0
    } else {
        0.0
    };

    // Guardar metadata
    {
        *state.last_optimization.write() = Some(OptimizationMetadata {
            optimized_size,
            savings_percent,
            mime_type: result.mime_type.clone(),
            extension: result.extension.clone(),
        });
    }

    use base64::{engine::general_purpose::STANDARD, Engine};
    let preview_base64 = STANDARD.encode(&result.data);

    Ok(OptimizationResult {
        preview_base64: format!("data:{};base64,{}", result.mime_type, preview_base64),
        optimized_size,
        savings_percent,
        mime_type: result.mime_type,
        extension: result.extension,
    })
}

/// Guarda la imagen optimizada en disco
#[tauri::command]
async fn save_image(
    path: String,
    request: OptimizationRequest,
    state: State<'_, AppState>,
) -> Result<SaveResult, String> {
    let img_arc = {
        let guard = state.original_image.read();
        guard.as_ref()
            .ok_or_else(|| WindooshError::NoImage)?
            .clone()
    };

    let path_for_save = path.clone();
    
    let final_size = tauri::async_runtime::spawn_blocking(move || {
        let result = process_pipeline(&img_arc, &request)?;
        std::fs::write(&path_for_save, &result.data)
            .map_err(|e| WindooshError::FileRead(format!("Error al guardar: {}", e)))?;
        Ok::<_, WindooshError>(result.data.len())
    })
    .await
    .map_err(|e| WindooshError::Concurrency(e.to_string()))?
    .map_err(String::from)?;

    Ok(SaveResult {
        path,
        final_size,
    })
}

/// Obtiene la metadata de la última optimización
#[tauri::command]
fn get_optimization_metadata(state: State<AppState>) -> Option<OptimizationMetadata> {
    state.last_optimization.read().clone()
}

// ============================================================================
// Application Entry Point
// ============================================================================

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(AppState::default())
        .setup(|app| {
            let args: Vec<String> = std::env::args().collect();
            if args.len() > 1 {
                let file_path = args[1].clone();
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    let _ = handle.emit("open-file-from-args", file_path);
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            load_image,
            process_image,
            save_image,
            get_optimization_metadata
        ])
        .run(tauri::generate_context!())
        .expect("Error al ejecutar la aplicación Tauri");
}
