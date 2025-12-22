use super::traits::{EncodingResult, ImageEncoder};
use image::DynamicImage;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub struct WebPCodec;

#[derive(Debug, Serialize, Deserialize)]
struct WebPOptions {
    quality: f32, // 0.0 - 100.0
    lossless: bool,
    method: i32, // 0 (fast) - 6 (slowest/best), default 4
}

impl Default for WebPOptions {
    fn default() -> Self {
        Self {
            quality: 75.0,
            lossless: false,
            method: 4,
        }
    }
}

impl ImageEncoder for WebPCodec {
    fn name(&self) -> &str {
        "webp"
    }

    fn supported_formats(&self) -> Vec<&str> {
        vec!["webp"]
    }

    fn encode(&self, image: &DynamicImage, options: &Value) -> Result<EncodingResult, String> {
        let opts: WebPOptions = serde_json::from_value(options.clone()).unwrap_or_default();

        let encoder = webp::Encoder::from_image(image).map_err(|e| format!("Error creando WebP encoder: {}", e))?;

        let memory = if opts.lossless {
            encoder.encode_lossless()
        } else {
            encoder.encode(opts.quality)
        };

        // Note: 'webp' crate handling related to 'method' is implicit in standard encode for simple API, 
        // to use advanced config (method, thread_level) we might need unsafe access or waiting for crate update.
        // For now, simpler implementation with just Quality/Lossless.
        // If exact parity with Squoosh method (0-6) is needed, we need to check if 'webp' crate exposes Config or advanced encoding.
        // Currently 'webp' crate 0.3 exposes simple encode(quality) and encode_lossless().

        let bytes = memory.to_vec();

        Ok(EncodingResult {
            data: bytes,
            mime_type: "image/webp".to_string(),
            extension: "webp".to_string(),
        })
    }

    fn options_schema(&self) -> Value {
        json!({
            "quality": {
                "type": "slider",
                "label": "Quality",
                "min": 0,
                "max": 100,
                "default": 75
            },
            "lossless": {
                "type": "checkbox",
                "label": "Lossless",
                "default": false
            }
        })
    }
}
