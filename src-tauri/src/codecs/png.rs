use super::traits::{EncodingResult, ImageEncoder};
use image::{DynamicImage, GenericImageView, ImageFormat};
use oxipng::{Options, RawImage};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::Cursor;

pub struct OxiPngCodec;

#[derive(Debug, Serialize, Deserialize)]
struct OxiPngOptions {
    level: u8, // 0-6
    interlace: bool,
}

impl Default for OxiPngOptions {
    fn default() -> Self {
        Self {
            level: 2,
            interlace: false,
        }
    }
}

impl ImageEncoder for OxiPngCodec {
    fn name(&self) -> &str {
        "oxipng"
    }

    fn supported_formats(&self) -> Vec<&str> {
        vec!["png"]
    }

    fn encode(&self, image: &DynamicImage, options: &Value) -> Result<EncodingResult, String> {
        let opts: OxiPngOptions = serde_json::from_value(options.clone()).unwrap_or_default();

        // Configurar OxiPNG
        let mut oxipng_opts = Options::from_preset(opts.level);
        oxipng_opts.interlace = if opts.interlace { 
            Some(oxipng::Interlacing::Adam7) 
        } else { 
            None 
        };

        // Intentar usar RawImage para evitar doble encoding
        // Si falla, usar el método tradicional como fallback
        let optimized_bytes = match try_encode_raw(image, &oxipng_opts) {
            Ok(bytes) => bytes,
            Err(_) => {
                // Fallback: encode a PNG primero y luego optimizar
                let mut raw_png_bytes = Vec::new();
                image.write_to(&mut Cursor::new(&mut raw_png_bytes), ImageFormat::Png)
                    .map_err(|e| e.to_string())?;
                oxipng::optimize_from_memory(&raw_png_bytes, &oxipng_opts)
                    .map_err(|e| e.to_string())?
            }
        };

        Ok(EncodingResult {
            data: optimized_bytes,
            mime_type: "image/png".to_string(),
            extension: "png".to_string(),
        })
    }

    fn options_schema(&self) -> Value {
        json!({
            "level": {
                "type": "slider",
                "label": "Optimization Effort",
                "min": 0,
                "max": 6,
                "default": 2
            },
            "interlace": {
                "type": "checkbox",
                "label": "Interlace (Adam7)",
                "default": false
            }
        })
    }
}

/// Intenta codificar usando RawImage directamente (evita PNG encode + re-optimize)
fn try_encode_raw(image: &DynamicImage, opts: &Options) -> Result<Vec<u8>, String> {
    let (width, height) = image.dimensions();
    
    // Siempre usar RGBA para compatibilidad
    let rgba = image.to_rgba8();
    let raw_data = rgba.into_raw();
    
    let raw_image = RawImage::new(
        width,
        height,
        oxipng::ColorType::RGBA,
        oxipng::BitDepth::Eight,
        raw_data,
    ).map_err(|e| format!("Error creando RawImage RGBA: {:?}", e))?;
    
    raw_image.create_optimized_png(opts)
        .map_err(|e| format!("Error optimizando PNG: {:?}", e))
}
