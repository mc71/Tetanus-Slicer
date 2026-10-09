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

### Run
Slice an STL file into G-code:
```bash
./target/release/rust_slicer path/to/model.stl output.gcode --layer-height 0.2 --perimeters 2 --infill 0.20
```

If run without arguments, it automatically generates a `cube.stl` calibration cube and slices it:
```bash
./target/release/rust_slicer
```

### Options
* `--layer-height <mm>`: Layer thickness (default: `0.20`)
* `--perimeters <n>`: Number of wall shells (default: `2`)
* `--infill <float>`: Infill ratio between 0.0 and 1.0 (default: `0.20` for 20%)
