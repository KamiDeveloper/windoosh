# Windoosh

A high-performance image optimization application built with modern web technologies and native desktop capabilities. Windoosh provides real-time visual comparison between original and optimized images with advanced editing tools.

## Overview

Windoosh is a desktop application that enables users to compress and optimize images while maintaining visual quality control. The application features a split-view comparison interface with synchronized pan and zoom capabilities, allowing precise evaluation of compression results.

## Technology Stack

### Frontend
- **SvelteKit** - Reactive UI framework with optimal bundle size
- **TypeScript** - Type-safe development environment
- **Vite** - Next-generation frontend tooling with HMR

### Backend
- **Tauri 2.0** - Rust-powered native desktop runtime
- **Rust** - Systems programming for performance-critical operations

### Image Processing Libraries
- **MozJPEG** - Advanced JPEG encoder with superior compression
- **OxiPNG** - Multithreaded PNG optimizer
- **WebP** - Modern image format with lossy and lossless compression
- **image-rs** - Pure Rust image processing library
- **fast_image_resize** - High-performance image resizing with multiple algorithms

## Core Features

### Image Optimization
- Support for multiple formats: PNG, JPEG, WebP, GIF, BMP
- Three encoder options with customizable parameters:
  - MozJPEG with quality control
  - OxiPNG with compression level selection
  - WebP with lossy/lossless modes and method selection
- Real-time preview generation
- Automatic file size calculation and savings metrics

### Visual Comparison Interface
- Split-view slider with synchronized image layers
- Viewport-anchored clipping mask (independent of pan/zoom transformations)
- Static checkerboard background for transparency visualization
- Infinite zoom with cursor-centered scaling
- Pan and zoom controls with keyboard shortcuts
- Context menu reset functionality

### Image Editing
- Resize with multiple interpolation algorithms:
  - Lanczos3 (high quality)
  - CatmullRom (balanced)
  - Mitchell (sharp)
  - Bilinear (fast)
- Aspect ratio lock/unlock
- Preset size options (25%, 50%, 75%, 100%)
- Custom dimension input
- Color quantization with dithering control

### User Experience
- Drag and drop support via Tauri native events
- Click-to-open file dialog
- Keyboard shortcut (Ctrl+O) for quick access
- Loading states with visual feedback
- Zero flash-of-white on startup
- Animated splash screen with smooth transitions
- Breathing animation for empty state logo
- Dark mode optimized interface

### Performance Optimizations
- Native window background color to eliminate startup flicker
- Inline critical CSS for instant rendering
- Hidden window pattern with programmatic reveal
- GPU-accelerated animations using transform and opacity
- Efficient memory management with automatic cleanup
- Debounced processing for real-time parameter adjustments
- WebAssembly-ready architecture

## Architecture

### Component Structure
```
src/
├── routes/
│   ├── +page.svelte          # Main application container
│   ├── +layout.svelte         # Global layout wrapper
│   └── +layout.ts             # Layout configuration
├── lib/
│   ├── components/
│   │   ├── CompareSlider.svelte    # Visual comparison component
│   │   └── ControlPanel.svelte     # Settings and controls sidebar
│   ├── stores/
│   │   └── imageStore.ts           # Centralized state management
│   └── icons.ts                     # SVG icon definitions
└── app.css                          # Global styles and design system
```

### State Management
Centralized Svelte stores for reactive data flow:
- Original image metadata and preview
- Optimized result with compression statistics
- Encoder configuration and parameters
- Loading and processing states
- Derived computed values for formatted output

### Rust Backend
```
src-tauri/
├── src/
│   ├── main.rs              # Application entry point and commands
│   ├── lib.rs               # Core library exports
│   └── codecs/
│       ├── mod.rs           # Codec trait definitions
│       ├── jpeg.rs          # MozJPEG implementation
│       ├── png.rs           # OxiPNG implementation
│       └── webp.rs          # WebP implementation
└── capabilities/
    └── default.json         # Security permissions configuration
```

### Design System
- Color palette: Zinc-based dark theme
- Typography: Inter font family with system fallbacks
- Spacing: 8px base unit with consistent multipliers
- Transitions: Cubic-bezier easing for smooth animations
- Components: Squoosh-inspired visual language

## Installation

### Prerequisites
- Node.js 18 or higher
- Rust 1.70 or higher
- Platform-specific build tools:
  - Windows: Visual Studio Build Tools
  - macOS: Xcode Command Line Tools
  - Linux: GCC, pkg-config, webkit2gtk

### Setup
```bash
# Install dependencies
npm install

# Run development server
npm run tauri dev

# Build for production
npm run tauri build
```

## Usage

### Opening Images
1. Drag and drop an image file into the application window
2. Click anywhere in the empty state area
3. Press Ctrl+O to open file dialog
4. Use the upload button in the control panel

### Adjusting Compression
1. Select encoder from dropdown (MozJPEG, OxiPNG, or WebP)
2. Adjust quality/compression sliders
3. Preview updates automatically with debouncing
4. View real-time statistics in footer panel

### Resizing Images
1. Enable resize toggle in Edit section
2. Select preset percentage or enter custom dimensions
3. Toggle aspect ratio lock as needed
4. Choose interpolation algorithm for quality/speed balance

### Comparing Results
1. Drag the vertical slider to reveal original vs optimized
2. Click and drag to pan the image
3. Scroll to zoom in/out (cursor-centered)
4. Right-click to reset view

### Saving Optimized Images
1. Review statistics (size reduction percentage)
2. Click "Save Optimized Image" button
3. Choose destination path and filename
4. Confirm to export

## Performance Characteristics

### Memory Efficiency
- Zero-copy image data transfer between Rust and JavaScript
- Base64 encoding only for preview generation
- Automatic resource cleanup on component unmount
- Stream-based file operations for large images

### Render Performance
- CSS containment for layout optimization
- Will-change hints for animated properties
- RequestAnimationFrame-based smooth scrolling
- Debounced processing prevents redundant calculations

### Startup Time
- Sub-second cold start with optimized bundle
- Instant hot reload during development
- Progressive enhancement pattern

## Security Model

Tauri's security-first architecture with explicit permissions:
- Filesystem access limited to user-selected files
- No network requests required for core functionality
- Dialog-scoped file operations
- Event-driven IPC with type validation

## License

This project uses Tauri, which is licensed under Apache-2.0 or MIT.

## Development

### Recommended IDE Setup
- Visual Studio Code
- Extensions:
  - Svelte for VS Code
  - rust-analyzer
  - Tauri
  - Prettier
  - ESLint

### Code Style
- TypeScript strict mode enabled
- Rust clippy lints enforced
- Prettier for consistent formatting
- Conventional commit messages

### Testing
```bash
# Run frontend tests
npm test

# Run Rust tests
cargo test --manifest-path src-tauri/Cargo.toml
```

## Contributing

Contributions are welcome. Please ensure:
- Code passes TypeScript type checking
- Rust code compiles without warnings
- UI changes maintain accessibility standards
- Performance benchmarks remain stable
