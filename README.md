# Bryce 7 Pro Studio (Rust + wgpu)

A comprehensive 3D environment creation studio inspired by **DAZ 3D Bryce 7 Pro** and the pioneering procedural graphics work of **Ken Musgrave, Eric Wenger, and Kai Krause**.

Built with **Rust**, **`wgpu`**, and **`egui`**, featuring real-time procedural raytracing, volumetric atmosphere, infinite ocean surfaces, 3D spatial procedural materials (Deep Texture Editor), interactive 2D heightmap sculpting, CSG boolean modeling, and one-click world generation.

---

## Highlights & Capabilities

### 1. ⛰️ Procedural Terrain Generation & Interactive Sculptor
- **Ken Musgrave Procedural Noise Algorithms**:
  - Fractional Brownian Motion (`fBm`)
  - Ridged Multifractal (sharp mountain crests and canyons)
  - Billow Noise (rolling sand dunes and smooth hills)
  - Worley Cellular (columnar basalt formations and mesas)
  - Domain-Warped Fractal (alien swirls and twisted strata)
  - Hybrid Multifractal (smooth valleys transitioning to jagged ridges)
- **Live 2D Heightmap Canvas Sculptor**:
  - Interactive paint canvas with hypsometric topographic Bryce colormap (deep ocean $\to$ sand $\to$ forest $\to$ highland rock $\to$ alpine snow).
  - Procedural sculpting brushes: **Raise**, **Lower**, **Smooth**, **Flatten**, **Noise**.
  - Interactive brush radius and strength with mouse hover circle indicators.
- **Geological Erosion Simulation**:
  - Controllable **Hydraulic Rain Erosion** (water droplet runoff and sediment deposition).
  - Controllable **Thermal Scree Weathering** (talus angle of repose breakdown).
- **Terrain Exporters**:
  - **OBJ 3D Mesh Exporter**: exports vertices, normals, and UV faces.
  - **16-bit Heightmap PNG Exporter**: suitable for external game engines and digital elevation models.

### 2. 🌅 Procedural Sky & Atmosphere Lab
- **Atmospheric Optics**:
  - Analytic Rayleigh scattering (zenith to horizon gradients).
  - Mie scattering (solar corona glare, flare, and disc illumination).
  - Atmospheric perspective, height-based distance haze, and ground fog.
  - Procedural twinkling starfield activated at dusk/night.
  - Procedural cloud ceiling at altitude with directional wind drift.
- **Celestial Mechanics**:
  - 360° Sun azimuth orbit dial.
  - Sun elevation dial (from deep twilight $-15^\circ$ to midday zenith $+90^\circ$).
- **Sky Presets**:
  - `Clear Blue Sky`, `Bryce 90s Sunset`, `Dramatic Storm`, `Twilight Glow`, `Starry Night`, `Alien Violet`, `Cyberpunk Neon`.

### 3. 🌊 Water & Ocean Generation
- Physical Fresnel surface reflections.
- Multi-component Gerstner wave displacement and animated wind chop.
- Beer-Lambert depth extinction (deep ocean blues to shallow emerald shores).
- Presets: `Open Ocean`, `Glacial Lake`, `Calm Mirror Surface`, `Alien Bioluminescent Liquid`.

### 4. 🎨 Materials Lab & Deep Texture Editor (DTE)
- **100% Procedural 3D Textures** — No UV unwrapping or external image files required:
  - Base and Secondary color ramps.
  - Roughness, Metallic, Reflectivity, and Transparency.
  - Physical Index of Refraction (`IOR`).
  - Procedural bump map normal perturbations evaluated via spatial gradient estimation.
  - Emissive vein channels and glow.
- **16 Authentic Bryce Material Presets**:
  - `Mountain Rock`, `Blue Marble`, `90s Mirror Chrome`, `Emerald Glass`, `Hammered Gold`, `Basalt Obsidian`, `Ocean Water`, `Alien Bioluminescent`, `Bryce Plastic`, `Weathered Sandstone`, `Glacial Ice`, `Wood Grain`, `Rusted Iron`, `Amethyst Crystal`, `Cyber Grid`, `Red Desert Rock`.

### 5. 🔮 3D Scene Composition, CSG Booleans & Scatter
- **3D Primitives**: Spheres, Cubes, Cylinders, Toruses.
- **Full 3-Axis Transforms**: Position, Rotation (Pitch, Yaw, Roll), and Non-Uniform Scale.
- **Constructive Solid Geometry (CSG)**:
  - `Positive (Solid Union)`: regular solid objects.
  - `Negative (Subtractive Cutout)`: punches physical holes and hollow cavities through solids and terrain, with inverted normals and interior material inheritance!
  - `Intersecting`: boolean intersection.
- **🌲 Vegetation & Rock Scatter System**:
  - Automatically scatters rock boulders and vegetation spires across the terrain.
  - Filtered by configurable **Altitude Limits** and **Slope Angles**.

### 6. 🎲 One-Click Procedural Worlds
- Generate complete environments with a single click:
  - `🏜️ Classic Bryce 1994 Canyon`: Red rock mesas, golden sunset, and floating chrome orb.
  - `🏔️ Alpine Fjord & Glacial Lake`: Rugged peaks, turquoise water, midday sun.
  - `🪐 Alien World of Pandora`: Bioluminescent spires, purple sky, and void ring.
  - `🐪 Sahara Desert Oasis`: Rolling dunes, desert haze, ancient altar, and emerald oasis pond.
  - `🌋 Volcanic Caldera & Ash Sky`: Dark obsidian cliffs, brooding storm clouds, and magma core.
  - `🔮 Surrealist Geometric Gallery`: Chrome spheres, golden toruses, and mirror plane.
- **Independent Component Rerolling**:
  - Reroll Sky without modifying terrain.
  - Reroll Terrain without modifying sky or objects.
  - Reroll Water waves and sea levels independently.

### 7. 🎥 Render Lab & Retro 90s Aesthetics
- **Render Quality Presets**: `Interactive Preview`, `Draft`, `Standard`, `High Quality`, `Maximum Production`.
- **Retro 1990s CGI Mode**:
  - Authentic 1990s surreal CGI look with punchy contrast, saturated vibrant colors, and optional 16-bit / 24-bit color quantization.
- **Post-Processing**:
  - Bloom, contrast, saturation, vignetting, and chromatic aberration.
- **Project Serialization**:
  - Save and load projects natively as `.bryce.json` files.
  - Render and export still image artwork as PNG.

---

### 8. 🎓 In-Studio Interactive Tutorial & Controls Guide
- **Interactive 9-Step Studio Walkthrough**:
  - Step 1: 🚀 6-DOF Fly Camera & Viewport Navigation (with framing presets: Default Vista, Ground Low-Angle, Aerial Bird's Eye)
  - Step 2: 🎲 One-Click Procedural Worlds & Component Rerolling
  - Step 3: ⛰️ Ken Musgrave Fractal Terrains & 2D Heightmap Canvas Sculptor
  - Step 4: 🌅 Sky Lab & Celestial Atmospheric Volumetrics
  - Step 5: 🌊 Infinite Water Plane & Ocean Physics
  - Step 6: 🎨 Deep Texture Editor (DTE) & 3D Spatial Texturing
  - Step 7: 🔮 3D Scene Composition & CSG Booleans
  - Step 8: 🌲 Vegetation & Rock Scatter System
  - Step 9: 🎥 Render Lab, Retro 1990s CGI & Project Export
- **Hands-On Action Buttons**: Every tutorial step includes one-click action buttons to test the features and jump directly to the relevant lab.
- **Floating Controls HUD**: Sleek semi-transparent floating card displaying all keyboard & mouse shortcuts at a glance.

---

## Studio Controls

| Input | Action |
| :--- | :--- |
| **`Tab`** | Toggle Bryce 7 Studio GUI on / off |
| **`F1`** | Open In-Studio Interactive Tutorial & Controls Guide |
| **Mouse Left Drag** | Look around / rotate camera (6-DOF) |
| **`W`, `A`, `S`, `D`** | Fly camera forward, left, backward, right |
| **`Space` / `E`** | Ascend vertically |
| **`Shift` / `Q`** | Descend vertically |
| **`Left` / `Right Arrow`** | Rotate Sun Azimuth (360° celestial orbit) |
| **`Up` / `Down Arrow`** | Adjust Sun Elevation (dawn to noon to dusk) |
| **`1` through `8`** | Quick-swap procedural material on central orb |
| **`Esc`** | Exit application |

---

## Build & Run

### Prerequisites
- Rust 1.80+ (Rust 2024 edition supported)
- Vulkan, Metal, or DirectX 12 capable GPU

### Running the Studio
```bash
cargo run --release
```

### Running the Integration Test Suite
```bash
cargo test
```
