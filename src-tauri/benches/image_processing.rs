// Windoosh Benchmark Suite
// Mide el rendimiento de las operaciones críticas de procesamiento de imágenes
//
// Ejecutar con: cargo bench --manifest-path src-tauri/Cargo.toml

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use image::{DynamicImage, RgbaImage};
use std::time::Duration;

/// Genera una imagen de prueba con dimensiones específicas
fn generate_test_image(width: u32, height: u32) -> DynamicImage {
    let mut img = RgbaImage::new(width, height);
    
    // Llenar con patrón de gradiente para simular imagen real
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        let r = ((x as f32 / width as f32) * 255.0) as u8;
        let g = ((y as f32 / height as f32) * 255.0) as u8;
        let b = (((x + y) as f32 / (width + height) as f32) * 255.0) as u8;
        *pixel = image::Rgba([r, g, b, 255]);
    }
    
    DynamicImage::ImageRgba8(img)
}

/// Benchmark de resize con image-rs (baseline)
fn bench_resize_image_rs(c: &mut Criterion) {
    let mut group = c.benchmark_group("resize_image_rs");
    group.measurement_time(Duration::from_secs(10));
    
    let sizes = [
        (1920, 1080, "1080p"),
        (3840, 2160, "4K"),
        (7680, 4320, "8K"),
    ];
    
    for (width, height, name) in sizes {
        let img = generate_test_image(width, height);
        let target_width = width / 2;
        let target_height = height / 2;
        
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(
            BenchmarkId::new("Lanczos3", name),
            &img,
            |b, img| {
                b.iter(|| {
                    img.resize_exact(
                        black_box(target_width),
                        black_box(target_height),
                        image::imageops::FilterType::Lanczos3,
                    )
                });
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("Triangle", name),
            &img,
            |b, img| {
                b.iter(|| {
                    img.resize_exact(
                        black_box(target_width),
                        black_box(target_height),
                        image::imageops::FilterType::Triangle,
                    )
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark de resize con fast_image_resize (optimizado)
fn bench_resize_fast(c: &mut Criterion) {
    use fast_image_resize::{images::Image, PixelType, ResizeAlg, ResizeOptions, Resizer};
    
    let mut group = c.benchmark_group("resize_fast_image_resize");
    group.measurement_time(Duration::from_secs(10));
    
    let sizes = [
        (1920, 1080, "1080p"),
        (3840, 2160, "4K"),
        (7680, 4320, "8K"),
    ];
    
    for (width, height, name) in sizes {
        let img = generate_test_image(width, height);
        let rgba = img.to_rgba8();
        let target_width = width / 2;
        let target_height = height / 2;
        
        group.throughput(Throughput::Elements(1));
        
        // Lanczos3
        group.bench_with_input(
            BenchmarkId::new("Lanczos3_SIMD", name),
            &rgba,
            |b, rgba| {
                b.iter(|| {
                    let src_image = Image::from_vec_u8(
                        width,
                        height,
                        rgba.clone().into_raw(),
                        PixelType::U8x4,
                    ).unwrap();
                    
                    let mut dst_image = Image::new(target_width, target_height, PixelType::U8x4);
                    let mut resizer = Resizer::new();
                    let options = ResizeOptions::new()
                        .resize_alg(ResizeAlg::Convolution(fast_image_resize::FilterType::Lanczos3));
                    
                    resizer.resize(&src_image, &mut dst_image, Some(&options)).unwrap();
                    black_box(dst_image)
                });
            },
        );
        
        // Bilinear (fast)
        group.bench_with_input(
            BenchmarkId::new("Bilinear_SIMD", name),
            &rgba,
            |b, rgba| {
                b.iter(|| {
                    let src_image = Image::from_vec_u8(
                        width,
                        height,
                        rgba.clone().into_raw(),
                        PixelType::U8x4,
                    ).unwrap();
                    
                    let mut dst_image = Image::new(target_width, target_height, PixelType::U8x4);
                    let mut resizer = Resizer::new();
                    let options = ResizeOptions::new()
                        .resize_alg(ResizeAlg::Convolution(fast_image_resize::FilterType::Bilinear));
                    
                    resizer.resize(&src_image, &mut dst_image, Some(&options)).unwrap();
                    black_box(dst_image)
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark de encoding JPEG
fn bench_jpeg_encode(c: &mut Criterion) {
    let mut group = c.benchmark_group("jpeg_encode");
    group.measurement_time(Duration::from_secs(10));
    
    let sizes = [
        (1920, 1080, "1080p"),
        (3840, 2160, "4K"),
    ];
    
    for (width, height, name) in sizes {
        let img = generate_test_image(width, height);
        
        group.throughput(Throughput::Bytes((width * height * 4) as u64));
        
        for quality in [75, 85, 95] {
            group.bench_with_input(
                BenchmarkId::new(format!("quality_{}", quality), name),
                &img,
                |b, img| {
                    b.iter(|| {
                        let mut output = Vec::with_capacity(1024 * 1024);
                        let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(
                            &mut output,
                            black_box(quality),
                        );
                        img.write_with_encoder(encoder).unwrap();
                        black_box(output)
                    });
                },
            );
        }
    }
    
    group.finish();
}

/// Benchmark de encoding PNG con OxiPNG
fn bench_png_encode(c: &mut Criterion) {
    use oxipng::{Options, RawImage};
    
    let mut group = c.benchmark_group("png_encode_oxipng");
    group.measurement_time(Duration::from_secs(15));
    
    let img = generate_test_image(1920, 1080);
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    let raw_data = rgba.into_raw();
    
    for level in [1, 2, 4, 6] {
        group.throughput(Throughput::Bytes((width * height * 4) as u64));
        
        group.bench_with_input(
            BenchmarkId::new("level", level),
            &raw_data,
            |b, data| {
                b.iter(|| {
                    let opts = Options::from_preset(black_box(level));
                    let raw_image = RawImage::new(
                        width,
                        height,
                        oxipng::ColorType::RGBA,
                        oxipng::BitDepth::Eight,
                        data.clone(),
                    ).unwrap();
                    
                    black_box(raw_image.create_optimized_png(&opts).unwrap())
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark de Base64 encoding (para medir overhead a eliminar)
fn bench_base64_overhead(c: &mut Criterion) {
    use base64::{engine::general_purpose::STANDARD, Engine};
    
    let mut group = c.benchmark_group("base64_overhead");
    
    // Simular diferentes tamaños de imagen comprimida
    let sizes = [
        (100 * 1024, "100KB"),
        (500 * 1024, "500KB"),
        (1024 * 1024, "1MB"),
        (5 * 1024 * 1024, "5MB"),
        (10 * 1024 * 1024, "10MB"),
    ];
    
    for (size, name) in sizes {
        let data: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();
        
        group.throughput(Throughput::Bytes(size as u64));
        
        group.bench_with_input(
            BenchmarkId::new("encode", name),
            &data,
            |b, data| {
                b.iter(|| {
                    black_box(STANDARD.encode(data))
                });
            },
        );
    }
    
    group.finish();
}

criterion_group!(
    benches,
    bench_resize_image_rs,
    bench_resize_fast,
    bench_jpeg_encode,
    bench_png_encode,
    bench_base64_overhead,
);

criterion_main!(benches);
