# Tetanus-Slicer

A standalone, high-performance 3D slicing engine written in pure Rust.

## Features & Architecture

* **Multi-threaded Slicing Engine**: Every layer is computed in parallel across all CPU cores using `rayon`.
* **Spatial Z-Interval Index**: Reduces triangle-plane intersection queries from $O(N \times L)$ to $O(\log N + K)$.
* **Binary & ASCII STL Support**: Fast file loading with bounds calculation.
* **Segment Chaining**: Reconstructs closed 2D polygon loops from unordered triangle intersection segments.
* **Perimeter Generation**: Concentric polygon insetting with miter clamping for shell generation.
* **Rectilinear Infill**: Alternating 45° and 135° scanline ray-casting for solid interior infill.
* **Volumetric G-code Generation**: Computes exact extrusion lengths ($E$-axis) based on bead geometry and $1.75\,\text{mm}$ filament diameter, with automatic retractions, travel moves, bed/nozzle temperature staging, and homing.

## Quick Start

### Build
```bash
cargo build --release
```

### Web GUI Mode (Interactive 3D Viewport)
Launch the embedded web server and open the interactive 3D slicer in your browser:
```bash
./target/release/tetanus-slicer --web
```
Then navigate to **`http://localhost:8080`**.
* **3D Build Plate**: 220 × 220 mm bed grid with orbit/zoom controls.
* **Drag-and-Drop STL**: Drop any STL file onto the viewport for instant 3D rendering.
* **Instant Slicing**: Slices models in milliseconds powered by the native multi-core Rust engine.
* **Interactive Toolpath Scrubber**: Scrub through layers, inspect color-coded outer walls, inner walls, and infill, and auto-play the print buildup.
* **One-Click G-code Export**: Download the sliced `.gcode` file ready for printing.

### CLI Mode
Slice an STL file directly from the terminal:
```bash
./target/release/tetanus-slicer path/to/model.stl output.gcode --layer-height 0.2 --perimeters 2 --infill 0.20
```

If run without arguments, it generates a `cube.stl` calibration cube and slices it:
```bash
./target/release/tetanus-slicer
```

### CLI Options
* `--web` / `-w`: Start the embedded interactive Web GUI server
* `--port <n>`: Port for the web server (default: `8080`)
* `--layer-height <mm>`: Layer thickness (default: `0.20`)
* `--perimeters <n>`: Number of wall shells (default: `2`)
* `--infill <float>`: Infill ratio between 0.0 and 1.0 (default: `0.20` for 20%)
