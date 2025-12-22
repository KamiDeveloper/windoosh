pub mod traits;
pub mod jpeg;
pub mod png;
pub mod webp;
// pub mod avif;

// Re-exportar traits y codecs
pub use traits::{EncodingResult, ImageEncoder};
pub use jpeg::JpegCodec;
pub use png::OxiPngCodec;
pub use webp::WebPCodec;
