use image::DynamicImage;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EncodingResult {
    pub data: Vec<u8>,
    pub mime_type: String,
    pub extension: String,
}

#[allow(dead_code)] // Métodos planeados para API futura
pub trait ImageEncoder: Send + Sync {
    /// Nombre identificador del encoder (ej: "mozjpeg", "oxipng")
    fn name(&self) -> &str;
    
    /// Formatos soportados por este encoder
    fn supported_formats(&self) -> Vec<&str>;

    /// Comprime la imagen con las opciones dadas
    fn encode(&self, image: &DynamicImage, options: &serde_json::Value) -> Result<EncodingResult, String>;
    
    /// Retorna el esquema de opciones soportadas para generar la UI en el frontend
    fn options_schema(&self) -> serde_json::Value;
}
