use super::traits::{EncodingResult, ImageEncoder};
use image::DynamicImage;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub struct JpegCodec;

#[derive(Debug, Serialize, Deserialize)]
struct JpegOptions {
    quality: u8, // 1-100
    // Placeholders para paridad futura con MozJPEG
    // trellis: bool,
    // progressive: bool,
}

impl Default for JpegOptions {
    fn default() -> Self {
        Self {
            quality: 75,
        }
    }
}

impl ImageEncoder for JpegCodec {
    fn name(&self) -> &str {
        "mozjpeg" // Usamos este nombre para compatibilidad UI con Squoosh, aunque backend sea standard por ahora
    }

    fn supported_formats(&self) -> Vec<&str> {
        vec!["jpeg", "jpg"]
    }

    fn encode(&self, image: &DynamicImage, options: &Value) -> Result<EncodingResult, String> {
        let opts: JpegOptions = serde_json::from_value(options.clone()).unwrap_or_default();

        let mut output_bytes = Vec::new();
        // Usamos el encoder estándar de Rust que es seguro y multiplataforma
        let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output_bytes, opts.quality);
        image.write_with_encoder(encoder).map_err(|e| e.to_string())?;

        Ok(EncodingResult {
            data: output_bytes,
            mime_type: "image/jpeg".to_string(),
            extension: "jpg".to_string(),
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
            }
            // A futuro: añadir checkboxes para Progressive, Trellis, etc.
        })
    }
}
