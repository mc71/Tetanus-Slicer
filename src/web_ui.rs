pub const INDEX_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Tetanus Slicer | High-Speed Rust 3D Slicing Engine</title>
  <script src="https://cdnjs.cloudflare.com/ajax/libs/three.js/r128/three.min.js"></script>
  <script src="https://cdn.jsdelivr.net/npm/three@0.128.0/examples/js/controls/OrbitControls.js"></script>
  <script src="https://cdnjs.cloudflare.com/ajax/libs/jszip/3.10.1/jszip.min.js"></script>
  <style>
    :root {
      --bg: #090d16;
      --panel: rgba(15, 23, 42, 0.85);
      --panel-border: rgba(51, 65, 85, 0.7);
      --accent: #f97316;
      --accent-glow: rgba(249, 115, 22, 0.35);
      --text: #f8fafc;
      --text-muted: #94a3b8;
      --success: #10b981;
      --cyan: #06b6d4;
      --amber: #f59e0b;
    }
    * { box-sizing: border-box; margin: 0; padding: 0; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", sans-serif; }
    body { background: var(--bg); color: var(--text); overflow: hidden; height: 100vh; width: 100vw; }
    
    #canvas-container { position: absolute; inset: 0; z-index: 1; }

    /* Header Bar */
    header {
      position: absolute; top: 0; left: 0; right: 0; height: 56px;
      background: rgba(15, 23, 42, 0.75); backdrop-filter: blur(12px);
      border-bottom: 1px solid var(--panel-border);
      display: flex; align-items: center; justify-content: space-between;
      padding: 0 20px; z-index: 10;
    }
    .logo { display: flex; align-items: center; gap: 10px; font-weight: 800; font-size: 18px; letter-spacing: 0.5px; }
    .logo-badge { background: linear-gradient(135deg, #ea580c, #f97316); color: white; padding: 3px 8px; border-radius: 6px; font-size: 11px; font-weight: 700; text-transform: uppercase; }
    .tagline { font-size: 12px; color: var(--text-muted); font-weight: 500; }
    .stats-pill { display: none; align-items: center; gap: 8px; background: rgba(16, 185, 129, 0.15); border: 1px solid var(--success); color: #34d399; padding: 4px 12px; border-radius: 20px; font-size: 12px; font-weight: 600; }

    /* Control Sidebar */
    #sidebar {
      position: absolute; top: 72px; left: 20px; width: 330px; max-height: calc(100vh - 92px);
      background: var(--panel); backdrop-filter: blur(16px);
      border: 1px solid var(--panel-border); border-radius: 14px;
      padding: 20px; z-index: 10; display: flex; flex-direction: column; gap: 16px;
      overflow-y: auto; box-shadow: 0 20px 40px rgba(0, 0, 0, 0.4);
    }
    #sidebar::-webkit-scrollbar { width: 6px; }
    #sidebar::-webkit-scrollbar-thumb { background: #334155; border-radius: 3px; }

    /* Drop Zone */
    .dropzone {
      border: 2px dashed #475569; border-radius: 10px; padding: 18px 12px;
      text-align: center; cursor: pointer; transition: all 0.2s ease;
      background: rgba(30, 41, 59, 0.4);
    }
    .dropzone:hover, .dropzone.dragover { border-color: var(--accent); background: rgba(249, 115, 22, 0.08); }
    .dropzone svg { width: 28px; height: 28px; fill: var(--text-muted); margin-bottom: 6px; }
    .dropzone p { font-size: 13px; font-weight: 600; color: var(--text); }
    .dropzone span { font-size: 11px; color: var(--text-muted); }

    /* Model Info */
    #model-info { display: none; background: rgba(30, 41, 59, 0.6); border-radius: 8px; padding: 10px; font-size: 11px; }
    #model-info .row { display: flex; justify-content: space-between; margin-bottom: 4px; }
    #model-info .label { color: var(--text-muted); }
    #model-info .val { font-weight: 600; color: var(--text); font-family: monospace; }

    /* Form Inputs */
    .section-title { font-size: 11px; font-weight: 700; color: var(--text-muted); text-transform: uppercase; letter-spacing: 0.8px; margin-top: 4px; }
    .field { display: flex; flex-direction: column; gap: 5px; }
    .field-header { display: flex; justify-content: space-between; font-size: 12px; }
    .field-header label { font-weight: 500; color: var(--text); }
    .field-header span { color: var(--accent); font-weight: 700; font-family: monospace; font-size: 12px; }
    input[type=range] {
      -webkit-appearance: none; width: 100%; height: 6px; background: #334155; border-radius: 3px; outline: none;
    }
    input[type=range]::-webkit-slider-thumb {
      -webkit-appearance: none; width: 16px; height: 16px; border-radius: 50%; background: var(--accent); cursor: pointer; box-shadow: 0 0 8px var(--accent-glow);
    }
    .grid-2 { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; }

    /* Slice Button */
    #slice-btn {
      background: linear-gradient(135deg, #ea580c, #f97316); color: white; border: none;
      padding: 13px; border-radius: 10px; font-size: 14px; font-weight: 700; cursor: pointer;
      display: flex; align-items: center; justify-content: center; gap: 8px;
      box-shadow: 0 8px 20px var(--accent-glow); transition: all 0.2s ease;
    }
    #slice-btn:hover { transform: translateY(-1px); box-shadow: 0 10px 24px rgba(249, 115, 22, 0.5); }
    #slice-btn:disabled { opacity: 0.6; cursor: not-allowed; transform: none; }

    /* Bottom Layer Scrubber (Visible after slice) */
    #scrubber-panel {
      display: none; position: absolute; bottom: 24px; left: 50%; transform: translateX(-50%);
      width: min(650px, 90vw); background: var(--panel); backdrop-filter: blur(16px);
      border: 1px solid var(--panel-border); border-radius: 14px; padding: 14px 20px;
      z-index: 10; flex-direction: column; gap: 10px; box-shadow: 0 20px 40px rgba(0, 0, 0, 0.4);
    }
    .scrubber-header { display: flex; align-items: center; justify-content: space-between; }
    .layer-badge { font-family: monospace; font-size: 13px; font-weight: 700; color: var(--accent); }
    .scrubber-controls { display: flex; align-items: center; gap: 12px; }
    .btn-icon {
      background: #334155; color: white; border: none; width: 32px; height: 32px;
      border-radius: 8px; cursor: pointer; display: flex; align-items: center; justify-content: center;
      transition: background 0.15s ease;
    }
    .btn-icon:hover { background: #475569; }
    .toggles { display: flex; gap: 14px; font-size: 11px; color: var(--text-muted); align-items: center; }
    .toggles label { display: flex; align-items: center; gap: 5px; cursor: pointer; }
    .download-btn {
      background: #10b981; color: white; border: none; padding: 6px 14px; border-radius: 7px;
      font-size: 12px; font-weight: 700; cursor: pointer; transition: background 0.15s ease;
    }
    .download-btn:hover { background: #059669; }

    /* Legend */
    .legend { display: flex; gap: 12px; font-size: 11px; margin-top: 2px; }
    .legend-item { display: flex; align-items: center; gap: 4px; }
    .legend-dot { width: 8px; height: 8px; border-radius: 50%; }

    /* Spinner */
    .spinner {
      width: 16px; height: 16px; border: 2px solid white; border-top-color: transparent;
      border-radius: 50%; animation: spin 0.8s linear infinite; display: none;
    }
    @keyframes spin { to { transform: rotate(360deg); } }
  </style>
</head>
<body>
  <header>
    <div class="logo">
      <span>TETANUS SLICER</span>
      <span class="logo-badge">Rust Engine</span>
      <span class="tagline">High-Performance 3D Slicing</span>
    </div>
    <div id="stats-pill" class="stats-pill">
      <span>⚡ Sliced in <b id="stat-time">0 ms</b> (<b id="stat-layers">0</b> layers)</span>
      <span style="opacity:0.35">|</span>
      <span>⏱️ Est. <b id="stat-print-time">0m</b></span>
      <span style="opacity:0.35">|</span>
      <span>🧵 <b id="stat-filament">0g</b></span>
    </div>
  </header>

  <div id="canvas-container"></div>

  <!-- Sidebar -->
  <aside id="sidebar">
    <div class="dropzone" id="dropzone">
      <svg viewBox="0 0 24 24"><path d="M19.35 10.04C18.67 6.59 15.64 4 12 4 9.11 4 6.6 5.64 5.35 8.04 2.34 8.36 0 10.91 0 14c0 3.31 2.69 6 6 6h13c2.76 0 5-2.24 5-5 0-2.64-2.05-4.78-4.65-4.96zM14 13v4h-4v-4H7l5-5 5 5h-3z"/></svg>
      <p>Drop STL or 3MF model(s) here</p>
      <span>or click to browse (supports multi-model plater)</span>
      <input type="file" id="file-input" accept=".stl,.3mf" multiple style="display:none">
    </div>

    <div id="model-info">
      <div class="row"><span class="label">Model:</span><span class="val" id="info-name">-</span></div>
      <div class="row"><span class="label">Dimensions:</span><span class="val" id="info-size">-</span></div>
      <div class="row"><span class="label">Triangles:</span><span class="val" id="info-triangles">-</span></div>
    </div>

    <div class="section-title">Machine & Material</div>
    <div class="field">
      <div class="field-header"><label>3D Printer</label></div>
      <select id="inp-machine" style="background:#1e293b; color:#f8fafc; border:1px solid #475569; padding:7px 10px; border-radius:8px; outline:none; font-size:12px; font-weight:600; cursor:pointer;">
        <option value="bambu_a1" selected>Bambu Lab A1 (256×256 mm)</option>
        <option value="generic_cartesian">Generic Cartesian (220×220 mm)</option>
      </select>
    </div>
    <div class="field">
      <div class="field-header"><label>Filament Material</label></div>
      <select id="inp-material" style="background:#1e293b; color:#f8fafc; border:1px solid #475569; padding:7px 10px; border-radius:8px; outline:none; font-size:12px; font-weight:600; cursor:pointer;">
        <option value="pla" selected>Generic PLA (210°C / 60°C)</option>
        <option value="petg">Generic PETG (245°C / 70°C)</option>
      </select>
    </div>

    <div class="section-title">Print Quality</div>
    <div class="field">
      <div class="field-header"><label>Layer Height</label><span id="val-layer-height">0.20 mm</span></div>
      <input type="range" id="inp-layer-height" min="0.08" max="0.32" step="0.04" value="0.20">
    </div>
    <div class="field">
      <div class="field-header"><label>Variable Layer Heights</label></div>
      <label style="display:flex; align-items:center; gap:8px; font-size:12px; cursor:pointer; height:24px;">
        <input type="checkbox" id="inp-adaptive-layers" style="accent-color:var(--accent); width:16px; height:16px; cursor:pointer;">
        <span>Adaptive (0.08 - 0.28 mm by slope)</span>
      </label>
    </div>
    <div class="field">
      <div class="field-header"><label>Vase Mode</label></div>
      <label style="display:flex; align-items:center; gap:8px; font-size:12px; cursor:pointer; height:24px;">
        <input type="checkbox" id="inp-spiral-vase" style="accent-color:var(--accent); width:16px; height:16px; cursor:pointer;">
        <span>Spiralize Outer Contour</span>
      </label>
    </div>
    <div class="field">
      <div class="field-header"><label>Seam Placement</label></div>
      <select id="inp-seam" style="background:#1e293b; color:#f8fafc; border:1px solid #475569; padding:7px 10px; border-radius:8px; outline:none; font-size:12px; font-weight:600; cursor:pointer;">
        <option value="aligned" selected>Aligned (Convex Corners)</option>
        <option value="rear">Rear (+Y Back of Bed)</option>
        <option value="nearest">Nearest (Fastest Travel)</option>
        <option value="random">Random (Scattered)</option>
      </select>
    </div>
    <div class="field">
      <div class="field-header"><label>Wall Count (Perimeters)</label><span id="val-perimeters">2</span></div>
      <input type="range" id="inp-perimeters" min="1" max="5" step="1" value="2">
    </div>
    <div class="field">
      <div class="field-header"><label>Infill Pattern</label></div>
      <select id="inp-infill-pattern" style="background:#1e293b; color:#f8fafc; border:1px solid #475569; padding:7px 10px; border-radius:8px; outline:none; font-size:12px; font-weight:600; cursor:pointer;">
        <option value="rectilinear" selected>Rectilinear (Standard 45°/135°)</option>
        <option value="grid">Grid (Square Crosshatch)</option>
        <option value="triangles">Triangles (Isotropic Rigidity)</option>
        <option value="gyroid">Gyroid (Continuous 3D Sinusoidal)</option>
      </select>
    </div>
    <div class="field">
      <div class="field-header"><label>Infill Density</label><span id="val-infill">20%</span></div>
      <input type="range" id="inp-infill" min="0" max="100" step="5" value="20">
    </div>
    <div class="grid-2">
      <div class="field">
        <div class="field-header"><label>Top Shells</label><span id="val-top-solid">4</span></div>
        <input type="range" id="inp-top-solid" min="0" max="10" step="1" value="4">
      </div>
      <div class="field">
        <div class="field-header"><label>Bottom Shells</label><span id="val-bottom-solid">4</span></div>
        <input type="range" id="inp-bottom-solid" min="0" max="10" step="1" value="4">
      </div>
    </div>

    <div class="section-title">Support Structures</div>
    <div class="grid-2">
      <div class="field">
        <div class="field-header"><label>Supports</label></div>
        <label style="display:flex; align-items:center; gap:8px; font-size:12px; cursor:pointer; height:28px;">
          <input type="checkbox" id="inp-support-enabled" style="accent-color:var(--accent); width:16px; height:16px; cursor:pointer;">
          <span>Generate</span>
        </label>
      </div>
      <div class="field">
        <div class="field-header"><label>Overhang Angle</label><span id="val-support-angle">45°</span></div>
        <input type="range" id="inp-support-angle" min="30" max="75" step="5" value="45">
      </div>
    </div>

    <div class="section-title">Adhesion & Retraction</div>
    <div class="grid-2">
      <div class="field">
        <div class="field-header"><label>Skirt Loops</label><span id="val-skirt">2</span></div>
        <input type="range" id="inp-skirt" min="0" max="6" step="1" value="2">
      </div>
      <div class="field">
        <div class="field-header"><label>Brim Width</label><span id="val-brim">0 mm</span></div>
        <input type="range" id="inp-brim" min="0" max="15" step="1" value="0">
      </div>
    </div>
    <div class="grid-2">
      <div class="field">
        <div class="field-header"><label>Z-Hop</label><span id="val-z-hop">0.20 mm</span></div>
        <input type="range" id="inp-z-hop" min="0" max="1.0" step="0.05" value="0.20">
      </div>
      <div class="field">
        <div class="field-header"><label>Cooling Fan</label><span id="val-fan">100%</span></div>
        <input type="range" id="inp-fan" min="0" max="255" step="15" value="255">
      </div>
    </div>

    <div class="section-title">Speeds & Thermal</div>
    <div class="grid-2">
      <div class="field">
        <div class="field-header"><label>Speed</label><span id="val-speed">50</span></div>
        <input type="range" id="inp-speed" min="20" max="120" step="5" value="50">
      </div>
      <div class="field">
        <div class="field-header"><label>Nozzle</label><span id="val-nozzle">210°C</span></div>
        <input type="range" id="inp-nozzle" min="180" max="260" step="5" value="210">
      </div>
    </div>

    <button id="slice-btn">
      <span class="spinner" id="slice-spinner"></span>
      <span id="slice-btn-text">⚡ Slice Model</span>
    </button>
  </aside>

  <!-- Bottom Layer Scrubber -->
  <div id="scrubber-panel">
    <div class="scrubber-header">
      <div class="layer-badge" id="layer-badge">Layer 1 / 100 (Z = 0.20 mm)</div>
      <div class="scrubber-controls">
        <button class="btn-icon" id="btn-play" title="Auto play layers">▶</button>
        <button class="download-btn" id="btn-download">💾 Download .gcode</button>
      </div>
    </div>
    <input type="range" id="layer-slider" min="1" max="100" value="100">
    <div class="scrubber-header">
      <div class="legend">
        <div class="legend-item"><div class="legend-dot" style="background:#a855f7"></div> Skirt/Brim</div>
        <div class="legend-item"><div class="legend-dot" style="background:#14b8a6"></div> Support</div>
        <div class="legend-item"><div class="legend-dot" style="background:#10b981"></div> Outer Wall</div>
        <div class="legend-item"><div class="legend-dot" style="background:#f59e0b"></div> Inner Wall</div>
        <div class="legend-item"><div class="legend-dot" style="background:#06b6d4"></div> Infill</div>
        <div class="legend-item"><div class="legend-dot" style="background:#ffffff; box-shadow:0 0 6px #fff;"></div> Seam</div>
      </div>
      <div class="toggles">
        <label><input type="checkbox" id="chk-show-mesh" checked> Show Mesh</label>
        <label><input type="checkbox" id="chk-show-seams" checked> Seams</label>
        <label><input type="checkbox" id="chk-accumulate" checked> Build Up</label>
      </div>
    </div>
  </div>

  <script>
    // --- State ---
    let currentStlBase64 = null;
    let additionalModelsBase64 = [];
    let currentFileName = "cube.stl";
    let loadedMesh = null;
    let toolpathGroup = new THREE.Group();
    let slicedData = null;
    let isPlaying = false;
    let playTimer = null;

    // --- Three.js Scene Setup ---
    const container = document.getElementById('canvas-container');
    const scene = new THREE.Scene();
    scene.background = new THREE.Color(0x090d16);

    const camera = new THREE.PerspectiveCamera(45, window.innerWidth / window.innerHeight, 0.1, 2000);
    camera.position.set(160, -220, 180);
    camera.up.set(0, 0, 1); // Z is UP in 3D printing

    const renderer = new THREE.WebGLRenderer({ antialias: true });
    renderer.setSize(window.innerWidth, window.innerHeight);
    renderer.setPixelRatio(window.devicePixelRatio);
    renderer.shadowMap.enabled = true;
    container.appendChild(renderer.domElement);

    let currentBedX = 256;
    let currentBedY = 256;
    const controls = new THREE.OrbitControls(camera, renderer.domElement);
    controls.enableDamping = true;
    controls.dampingFactor = 0.05;

    // --- Dynamic Build Plate ---
    const bedGroup = new THREE.Group();
    scene.add(bedGroup);
    scene.add(toolpathGroup);

    function updateBuildPlate(sizeX = 256, sizeY = 256) {
      currentBedX = sizeX;
      currentBedY = sizeY;
      while (bedGroup.children.length > 0) {
        const obj = bedGroup.children[0];
        if (obj.geometry) obj.geometry.dispose();
        bedGroup.remove(obj);
      }

      const cx = sizeX * 0.5;
      const cy = sizeY * 0.5;
      controls.target.set(cx, cy, 20);

      const bedGrid = new THREE.GridHelper(Math.max(sizeX, sizeY), Math.round(Math.max(sizeX, sizeY) / 10), 0x475569, 0x1e293b);
      bedGrid.rotation.x = Math.PI / 2;
      bedGrid.position.set(cx, cy, 0);
      bedGroup.add(bedGrid);

      const bedBoxGeo = new THREE.BufferGeometry().setFromPoints([
        new THREE.Vector3(0, 0, 0), new THREE.Vector3(sizeX, 0, 0),
        new THREE.Vector3(sizeX, sizeY, 0), new THREE.Vector3(0, sizeY, 0),
        new THREE.Vector3(0, 0, 0)
      ]);
      const bedBoxMat = new THREE.LineBasicMaterial({ color: 0x64748b, linewidth: 2 });
      bedGroup.add(new THREE.Line(bedBoxGeo, bedBoxMat));

      const bedPlaneGeo = new THREE.PlaneGeometry(sizeX, sizeY);
      const bedPlaneMat = new THREE.MeshBasicMaterial({ color: 0x0f172a, transparent: true, opacity: 0.6 });
      const bedPlane = new THREE.Mesh(bedPlaneGeo, bedPlaneMat);
      bedPlane.position.set(cx, cy, -0.1);
      bedGroup.add(bedPlane);
    }
    updateBuildPlate(256, 256); // default to Bambu Lab A1

    // Lighting
    const ambientLight = new THREE.AmbientLight(0xffffff, 0.6);
    scene.add(ambientLight);
    const dirLight1 = new THREE.DirectionalLight(0xffffff, 0.8);
    dirLight1.position.set(150, -100, 250);
    scene.add(dirLight1);
    const dirLight2 = new THREE.DirectionalLight(0x38bdf8, 0.3);
    dirLight2.position.set(-100, 150, 100);
    scene.add(dirLight2);

    window.addEventListener('resize', () => {
      camera.aspect = window.innerWidth / window.innerHeight;
      camera.updateProjectionMatrix();
      renderer.setSize(window.innerWidth, window.innerHeight);
    });

    function animate() {
      requestAnimationFrame(animate);
      controls.update();
      renderer.render(scene, camera);
    }
    animate();

    // --- UI Listeners & Sliders ---
    function linkSlider(id, targetId, unit, mult = 1, decimals = null) {
      const inp = document.getElementById(id);
      const val = document.getElementById(targetId);
      if (!inp || !val) return;
      inp.addEventListener('input', () => {
        const dec = decimals !== null ? decimals : (mult < 1 || (inp.step && inp.step.includes('.')) ? 2 : 0);
        val.innerText = (parseFloat(inp.value) * mult).toFixed(dec) + unit;
      });
    }
    linkSlider('inp-layer-height', 'val-layer-height', ' mm', 1);
    linkSlider('inp-perimeters', 'val-perimeters', '', 1);
    linkSlider('inp-infill', 'val-infill', '%', 1);
    linkSlider('inp-top-solid', 'val-top-solid', '', 1);
    linkSlider('inp-bottom-solid', 'val-bottom-solid', '', 1);
    linkSlider('inp-skirt', 'val-skirt', '', 1);
    linkSlider('inp-brim', 'val-brim', ' mm', 1);
    linkSlider('inp-z-hop', 'val-z-hop', ' mm', 1, 2);
    linkSlider('inp-speed', 'val-speed', ' mm/s', 1);
    linkSlider('inp-nozzle', 'val-nozzle', '°C', 1);
    linkSlider('inp-support-angle', 'val-support-angle', '°', 1);

    const inpFan = document.getElementById('inp-fan');
    if (inpFan) {
      inpFan.addEventListener('input', () => {
        document.getElementById('val-fan').innerText = Math.round((parseInt(inpFan.value) / 255) * 100) + '%';
      });
    }

    // Machine & Material Profiles
    const inpMachine = document.getElementById('inp-machine');
    if (inpMachine) {
      inpMachine.addEventListener('change', () => {
        const val = inpMachine.value;
        if (val === 'bambu_a1') {
          updateBuildPlate(256, 256);
        } else if (val === 'generic_cartesian') {
          updateBuildPlate(220, 220);
        }
      });
    }

    const inpMaterial = document.getElementById('inp-material');
    if (inpMaterial) {
      inpMaterial.addEventListener('change', () => {
        const val = inpMaterial.value;
        const inpNozzle = document.getElementById('inp-nozzle');
        const valNozzle = document.getElementById('val-nozzle');
        const inpFan = document.getElementById('inp-fan');
        const valFan = document.getElementById('val-fan');
        if (val === 'petg') {
          if (inpNozzle) { inpNozzle.value = 245; valNozzle.innerText = '245°C'; }
          if (inpFan) { inpFan.value = 128; valFan.innerText = '50%'; }
        } else if (val === 'pla') {
          if (inpNozzle) { inpNozzle.value = 210; valNozzle.innerText = '210°C'; }
          if (inpFan) { inpFan.value = 255; valFan.innerText = '100%'; }
        }
      });
    }

    // Fetch and populate available profiles dynamically
    fetch('/api/profiles')
      .then(res => res.json())
      .then(data => {
        if (data.machines && data.machines.length > 0 && inpMachine) {
          const curr = inpMachine.value;
          inpMachine.innerHTML = '';
          for (let m of data.machines) {
            const opt = document.createElement('option');
            opt.value = m.id;
            opt.innerText = `${m.name} (${m.bed_size[0]}×${m.bed_size[1]} mm)`;
            if (m.id === curr) opt.selected = true;
            inpMachine.appendChild(opt);
          }
        }
        if (data.materials && data.materials.length > 0 && inpMaterial) {
          const curr = inpMaterial.value;
          inpMaterial.innerHTML = '';
          for (let mat of data.materials) {
            const opt = document.createElement('option');
            opt.value = mat.id;
            opt.innerText = `${mat.name} (${mat.nozzle_temp || 210}°C / ${mat.bed_temp || 60}°C)`;
            if (mat.id === curr) opt.selected = true;
            inpMaterial.appendChild(opt);
          }
        }
      })
      .catch(e => console.warn('Profiles load error:', e));

    // --- File Handling (Drop & Browse) ---
    const dropzone = document.getElementById('dropzone');
    const fileInput = document.getElementById('file-input');
    dropzone.addEventListener('click', () => fileInput.click());
    dropzone.addEventListener('dragover', (e) => { e.preventDefault(); dropzone.classList.add('dragover'); });
    dropzone.addEventListener('dragleave', () => dropzone.classList.remove('dragover'));
    dropzone.addEventListener('drop', (e) => {
      e.preventDefault();
      dropzone.classList.remove('dragover');
      if (e.dataTransfer.files.length > 0) handleFiles(e.dataTransfer.files);
    });
    fileInput.addEventListener('change', () => {
      if (fileInput.files.length > 0) handleFiles(fileInput.files);
    });

    function isZipBuffer(buffer) {
      if (buffer.byteLength < 4) return false;
      const b = new Uint8Array(buffer, 0, 4);
      return b[0] === 0x50 && b[1] === 0x4B && b[2] === 0x03 && b[3] === 0x04;
    }

    async function handleFiles(files) {
      if (!files || files.length === 0) return;
      additionalModelsBase64 = [];
      const file0 = files[0];
      currentFileName = files.length > 1 ? `${files.length} models arranged` : file0.name;

      const readBuffer = (f) => new Promise((resolve) => {
        const r = new FileReader();
        r.onload = (e) => resolve(e.target.result);
        r.readAsArrayBuffer(f);
      });

      const buf0 = await readBuffer(file0);
      currentStlBase64 = arrayBufferToBase64(buf0);

      for (let i = 1; i < files.length; i++) {
        const bufi = await readBuffer(files[i]);
        additionalModelsBase64.push(arrayBufferToBase64(bufi));
      }

      if (file0.name.toLowerCase().endsWith('.3mf') || isZipBuffer(buf0)) {
        try {
          await load3mfToScene(buf0);
          performSlice(true);
        } catch (err) {
          alert('Failed to load 3MF: ' + err.message);
        }
      } else {
        loadStlToScene(buf0);
        performSlice(true);
      }
    }

    function arrayBufferToBase64(buffer) {
      let binary = '';
      const bytes = new Uint8Array(buffer);
      const len = bytes.byteLength;
      for (let i = 0; i < len; i++) {
        binary += String.fromCharCode(bytes[i]);
      }
      return window.btoa(binary);
    }

    function displayGeometryInScene(geometry) {
      geometry.computeVertexNormals();

      if (loadedMesh) scene.remove(loadedMesh);

      // Center geometry on 220x220 build plate
      geometry.computeBoundingBox();
      const bb = geometry.boundingBox;
      const sizeX = bb.max.x - bb.min.x;
      const sizeY = bb.max.y - bb.min.y;
      const sizeZ = bb.max.z - bb.min.z;

      const centerX = (bb.max.x + bb.min.x) / 2;
      const centerY = (bb.max.y + bb.min.y) / 2;
      const minZ = bb.min.z;

      geometry.translate(currentBedX * 0.5 - centerX, currentBedY * 0.5 - centerY, -minZ);

      const material = new THREE.MeshPhongMaterial({
        color: 0x94a3b8,
        specular: 0x334155,
        shininess: 30,
        transparent: true,
        opacity: 0.8
      });
      loadedMesh = new THREE.Mesh(geometry, material);
      scene.add(loadedMesh);

      // Update UI Info
      document.getElementById('model-info').style.display = 'block';
      document.getElementById('info-name').innerText = currentFileName;
      document.getElementById('info-size').innerText = `${sizeX.toFixed(1)} × ${sizeY.toFixed(1)} × ${sizeZ.toFixed(1)} mm`;
      document.getElementById('info-triangles').innerText = (geometry.attributes.position.count / 3).toLocaleString();

      // Clear previous toolpaths
      clearToolpaths();
      document.getElementById('scrubber-panel').style.display = 'none';
      document.getElementById('stats-pill').style.display = 'none';
    }

    // --- Client-side STL Parser & Renderer ---
    function loadStlToScene(buffer) {
      const geometry = parseSTL(buffer);
      displayGeometryInScene(geometry);
    }

    // --- Client-side 3MF Parser & Renderer ---
    async function load3mfToScene(buffer) {
      const zip = await JSZip.loadAsync(buffer);
      const parser = new DOMParser();
      const triPositions = [];

      for (let fname of Object.keys(zip.files)) {
        if (fname.endsWith('.model')) {
          const modelXml = await zip.files[fname].async('text');
          const doc = parser.parseFromString(modelXml, 'text/xml');
          const modelEl = doc.querySelector('model');
          const unit = modelEl ? modelEl.getAttribute('unit') : 'millimeter';
          let scale = 1.0;
          if (unit === 'meter') scale = 1000.0;
          else if (unit === 'centimeter') scale = 10.0;
          else if (unit === 'inch') scale = 25.4;
          else if (unit === 'micron') scale = 0.001;

          const vertexEls = doc.querySelectorAll('vertex');
          const vertices = [];
          vertexEls.forEach(v => {
            vertices.push(
              parseFloat(v.getAttribute('x')) * scale,
              parseFloat(v.getAttribute('y')) * scale,
              parseFloat(v.getAttribute('z')) * scale
            );
          });

          const triangleEls = doc.querySelectorAll('triangle');
          triangleEls.forEach(t => {
            const v1 = parseInt(t.getAttribute('v1'));
            const v2 = parseInt(t.getAttribute('v2'));
            const v3 = parseInt(t.getAttribute('v3'));
            if (v1 * 3 + 2 >= vertices.length || v2 * 3 + 2 >= vertices.length || v3 * 3 + 2 >= vertices.length) return;
            triPositions.push(
              vertices[v1 * 3], vertices[v1 * 3 + 1], vertices[v1 * 3 + 2],
              vertices[v2 * 3], vertices[v2 * 3 + 1], vertices[v2 * 3 + 2],
              vertices[v3 * 3], vertices[v3 * 3 + 1], vertices[v3 * 3 + 2]
            );
          });
        }
      }

      if (triPositions.length === 0) throw new Error('No 3D meshes found in 3MF archive');

      const geom = new THREE.BufferGeometry();
      geom.setAttribute('position', new THREE.Float32BufferAttribute(triPositions, 3));
      displayGeometryInScene(geom);
    }

    function parseSTL(buffer) {
      const reader = new DataView(buffer);
      const isBinary = buffer.byteLength > 84 && (reader.getUint32(80, true) * 50 + 84 === buffer.byteLength);

      const vertices = [];
      if (isBinary) {
        const count = reader.getUint32(80, true);
        let offset = 84;
        for (let i = 0; i < count; i++) {
          offset += 12; // skip normal
          for (let v = 0; v < 3; v++) {
            vertices.push(
              reader.getFloat32(offset, true),
              reader.getFloat32(offset + 4, true),
              reader.getFloat32(offset + 8, true)
            );
            offset += 12;
          }
          offset += 2; // skip attribute
        }
      } else {
        const text = new TextDecoder().decode(buffer);
        const lines = text.split('\n');
        for (let line of lines) {
          const parts = line.trim().split(/\s+/);
          if (parts[0] === 'vertex') {
            vertices.push(parseFloat(parts[1]), parseFloat(parts[2]), parseFloat(parts[3]));
          }
        }
      }

      const geom = new THREE.BufferGeometry();
      geom.setAttribute('position', new THREE.Float32BufferAttribute(vertices, 3));
      return geom;
    }

    // --- Slicing API Request & Auto-Slice ---
    const sliceBtn = document.getElementById('slice-btn');
    const sliceSpinner = document.getElementById('slice-spinner');
    const sliceBtnText = document.getElementById('slice-btn-text');
    let isSlicing = false;
    let slicePending = false;
    let autoSliceTimer = null;

    async function performSlice(isAuto = false) {
      if (!currentStlBase64) {
        if (!isAuto) alert('Please load a 3D model first.');
        return;
      }

      if (autoSliceTimer) {
        clearTimeout(autoSliceTimer);
        autoSliceTimer = null;
      }

      if (isSlicing) {
        slicePending = true;
        return;
      }

      isSlicing = true;
      sliceBtn.disabled = true;
      sliceSpinner.style.display = 'inline-block';
      sliceBtnText.innerText = 'Slicing in Rust...';

      const payload = {
        stl_base64: currentStlBase64,
        additional_models_base64: additionalModelsBase64,
        layer_height: parseFloat(document.getElementById('inp-layer-height').value),
        adaptive_layers: document.getElementById('inp-adaptive-layers').checked,
        spiral_vase: document.getElementById('inp-spiral-vase') ? document.getElementById('inp-spiral-vase').checked : false,
        seam_position: document.getElementById('inp-seam').value,
        perimeters: parseInt(document.getElementById('inp-perimeters').value),
        infill_pattern: document.getElementById('inp-infill-pattern').value,
        infill_density: parseFloat(document.getElementById('inp-infill').value) / 100.0,
        print_speed: parseFloat(document.getElementById('inp-speed').value),
        nozzle_temp: parseInt(document.getElementById('inp-nozzle').value),
        bed_temp: 60,
        top_solid_layers: parseInt(document.getElementById('inp-top-solid').value),
        bottom_solid_layers: parseInt(document.getElementById('inp-bottom-solid').value),
        support_enabled: document.getElementById('inp-support-enabled').checked,
        support_angle: parseFloat(document.getElementById('inp-support-angle').value),
        skirt_loops: parseInt(document.getElementById('inp-skirt').value),
        brim_width: parseFloat(document.getElementById('inp-brim').value),
        z_hop: parseFloat(document.getElementById('inp-z-hop').value),
        machine: document.getElementById('inp-machine') ? document.getElementById('inp-machine').value : 'bambu_a1',
        material: document.getElementById('inp-material') ? document.getElementById('inp-material').value : 'pla',
        fan_speed: parseInt(document.getElementById('inp-fan').value)
      };

      try {
        const res = await fetch('/api/slice', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(payload)
        });
        const data = await res.json();
        if (!data.success) throw new Error(data.error || 'Slicing failed');

        slicedData = data;
        onSliceSuccess(data);
      } catch (err) {
        if (!isAuto) {
          alert('Slicing error: ' + err.message);
        } else {
          console.warn('Auto-slice error:', err);
        }
      } finally {
        isSlicing = false;
        sliceBtn.disabled = false;
        sliceSpinner.style.display = 'none';
        sliceBtnText.innerText = '⚡ Slice Model';
        if (slicePending) {
          slicePending = false;
          performSlice(true);
        }
      }
    }

    function triggerDebouncedSlice() {
      if (!currentStlBase64) return;
      if (autoSliceTimer) clearTimeout(autoSliceTimer);
      sliceBtnText.innerText = '⚡ Slicing in 0.5s...';
      autoSliceTimer = setTimeout(() => {
        autoSliceTimer = null;
        performSlice(true);
      }, 500);
    }

    // Auto-slice when any setting input, range, select, or checkbox changes (500ms debounce)
    const sidebar = document.getElementById('sidebar');
    const handleSettingChange = (e) => {
      if (e.target && e.target.id && e.target.id.startsWith('inp-')) {
        triggerDebouncedSlice();
      }
    };
    sidebar.addEventListener('input', handleSettingChange);
    sidebar.addEventListener('change', handleSettingChange);

    sliceBtn.addEventListener('click', () => {
      performSlice(false);
    });

    function onSliceSuccess(data) {
      // Update stats pill
      document.getElementById('stat-time').innerText = `${data.stats.elapsed_ms.toFixed(1)} ms`;
      document.getElementById('stat-layers').innerText = data.stats.layer_count;
      document.getElementById('stat-print-time').innerText = data.stats.print_time_formatted || '-';
      document.getElementById('stats-pill').style.display = 'flex';

      // Update build plate size if provided
      if (data.bed_size && (data.bed_size[0] !== currentBedX || data.bed_size[1] !== currentBedY)) {
        updateBuildPlate(data.bed_size[0], data.bed_size[1]);
      }

      // Setup scrubber
      const slider = document.getElementById('layer-slider');
      const wasAtTop = !slider.max || parseInt(slider.value) >= parseInt(slider.max);
      const prevVal = parseInt(slider.value) || data.layers.length;
      slider.min = 1;
      slider.max = data.layers.length;
      slider.value = wasAtTop ? data.layers.length : Math.min(prevVal, data.layers.length);
      document.getElementById('scrubber-panel').style.display = 'flex';

      // Mesh transparency
      if (loadedMesh) loadedMesh.material.opacity = 0.25;

      renderToolpaths(parseInt(slider.value));
    }

    // --- Toolpath Visualization ---
    function clearToolpaths() {
      while (toolpathGroup.children.length > 0) {
        const obj = toolpathGroup.children[0];
        obj.geometry.dispose();
        obj.material.dispose();
        toolpathGroup.remove(obj);
      }
    }

    function renderToolpaths(maxLayerIndex) {
      clearToolpaths();
      if (!slicedData || !slicedData.layers) return;

      const accumulate = document.getElementById('chk-accumulate').checked;
      const showSeams = document.getElementById('chk-show-seams') ? document.getElementById('chk-show-seams').checked : true;
      const startIdx = accumulate ? 0 : maxLayerIndex - 1;
      const endIdx = maxLayerIndex;

      const positions = [];
      const colors = [];
      const seamPositions = [];

      const colSkirt = new THREE.Color(0xa855f7); // Violet/Purple for skirt & brim
      const colSupport = new THREE.Color(0x14b8a6); // Vibrant Teal/Seafoam for supports
      const colOuter = new THREE.Color(0x10b981); // Emerald green
      const colInner = new THREE.Color(0xf59e0b); // Amber
      const colInfill = new THREE.Color(0x06b6d4); // Cyan

      for (let i = startIdx; i < endIdx; i++) {
        const layer = slicedData.layers[i];
        if (!layer) continue;
        const z = layer.z;

        // Skirt / Brim
        if (layer.skirt_brim) {
          for (let poly of layer.skirt_brim) {
            const n = poly.length;
            for (let j = 0; j < n; j++) {
              const p1 = poly[j];
              const p2 = poly[(j + 1) % n];
              positions.push(p1[0], p1[1], z, p2[0], p2[1], z);
              colors.push(colSkirt.r, colSkirt.g, colSkirt.b, colSkirt.r, colSkirt.g, colSkirt.b);
            }
          }
        }

        // Supports
        if (layer.supports) {
          for (let poly of layer.supports) {
            const n = poly.length;
            for (let j = 0; j < n; j++) {
              const p1 = poly[j];
              const p2 = poly[(j + 1) % n];
              positions.push(p1[0], p1[1], z, p2[0], p2[1], z);
              colors.push(colSupport.r, colSupport.g, colSupport.b, colSupport.r, colSupport.g, colSupport.b);
            }
          }
        }
        if (layer.support_infill) {
          for (let seg of layer.support_infill) {
            positions.push(seg[0][0], seg[0][1], z, seg[1][0], seg[1][1], z);
            colors.push(colSupport.r, colSupport.g, colSupport.b, colSupport.r, colSupport.g, colSupport.b);
          }
        }

        // Perimeters
        if (layer.perimeters) {
          const isSpiral = slicedData.spiral_vase && i >= (document.getElementById('inp-bottom-solid') ? parseInt(document.getElementById('inp-bottom-solid').value) : 4);
          const prevZ = (i > 0 && slicedData.layers[i - 1]) ? slicedData.layers[i - 1].z : (z - 0.2);
          const deltaZ = z - prevZ;

          for (let perim of layer.perimeters) {
            const color = perim.is_outer ? colOuter : colInner;
            const poly = perim.points;
            if (!poly || poly.length < 2) continue;
            const n = poly.length;
            for (let j = 0; j < n; j++) {
              const p1 = poly[j];
              const p2 = poly[(j + 1) % n];
              if (isSpiral) {
                const z1 = prevZ + (j / n) * deltaZ;
                const z2 = prevZ + ((j + 1) / n) * deltaZ;
                positions.push(p1[0], p1[1], z1, p2[0], p2[1], z2);
              } else {
                positions.push(p1[0], p1[1], z, p2[0], p2[1], z);
              }
              colors.push(color.r, color.g, color.b, color.r, color.g, color.b);
            }
          }
        }

        // Seams
        if (showSeams && layer.seam_points) {
          for (let sp of layer.seam_points) {
            seamPositions.push(sp[0], sp[1], sp[2]);
          }
        }

        // Infill
        if (layer.infill) {
          for (let seg of layer.infill) {
            positions.push(seg[0][0], seg[0][1], z, seg[1][0], seg[1][1], z);
            colors.push(colInfill.r, colInfill.g, colInfill.b, colInfill.r, colInfill.g, colInfill.b);
          }
        }
      }

      if (positions.length > 0) {
        const geom = new THREE.BufferGeometry();
        geom.setAttribute('position', new THREE.Float32BufferAttribute(positions, 3));
        geom.setAttribute('color', new THREE.Float32BufferAttribute(colors, 3));
        const mat = new THREE.LineBasicMaterial({ vertexColors: true, linewidth: 2 });
        const lines = new THREE.LineSegments(geom, mat);
        toolpathGroup.add(lines);
      }

      // Render glowing seam markers
      if (seamPositions.length > 0) {
        const seamGeom = new THREE.BufferGeometry();
        seamGeom.setAttribute('position', new THREE.Float32BufferAttribute(seamPositions, 3));
        const seamMat = new THREE.PointsMaterial({
          color: 0xffffff,
          size: 5,
          sizeAttenuation: false
        });
        const seamPoints = new THREE.Points(seamGeom, seamMat);
        toolpathGroup.add(seamPoints);
      }

      // Update badge
      const curL = slicedData.layers[maxLayerIndex - 1];
      if (curL) {
        document.getElementById('layer-badge').innerText =
          `Layer ${maxLayerIndex} / ${slicedData.layers.length} (Z = ${curL.z.toFixed(2)} mm)`;
      }
    }

    // --- Scrubber Controls ---
    const layerSlider = document.getElementById('layer-slider');
    layerSlider.addEventListener('input', () => {
      renderToolpaths(parseInt(layerSlider.value));
    });

    document.getElementById('chk-accumulate').addEventListener('change', () => {
      renderToolpaths(parseInt(layerSlider.value));
    });

    const chkShowSeams = document.getElementById('chk-show-seams');
    if (chkShowSeams) {
      chkShowSeams.addEventListener('change', () => {
        renderToolpaths(parseInt(layerSlider.value));
      });
    }

    document.getElementById('chk-show-mesh').addEventListener('change', (e) => {
      if (loadedMesh) loadedMesh.visible = e.target.checked;
    });

    // Auto Play Layers
    const btnPlay = document.getElementById('btn-play');
    btnPlay.addEventListener('click', () => {
      if (isPlaying) {
        clearInterval(playTimer);
        isPlaying = false;
        btnPlay.innerText = '▶';
      } else {
        isPlaying = true;
        btnPlay.innerText = '⏸';
        if (parseInt(layerSlider.value) >= slicedData.layers.length) {
          layerSlider.value = 1;
        }
        playTimer = setInterval(() => {
          let v = parseInt(layerSlider.value);
          if (v < slicedData.layers.length) {
            layerSlider.value = v + 1;
            renderToolpaths(v + 1);
          } else {
            clearInterval(playTimer);
            isPlaying = false;
            btnPlay.innerText = '▶';
          }
        }, 60);
      }
    });

    // Download G-code
    document.getElementById('btn-download').addEventListener('click', () => {
      if (!slicedData || !slicedData.gcode) return;
      const blob = new Blob([slicedData.gcode], { type: 'text/plain' });
      const a = document.createElement('a');
      a.href = URL.createObjectURL(blob);
      a.download = currentFileName.replace(/\.(stl|3mf)$/i, '') + '.gcode';
      a.click();
    });

    // Trigger default cube load on startup
    window.addEventListener('load', () => {
      fetch('/api/default_cube')
        .then(res => res.arrayBuffer())
        .then(buffer => {
          currentFileName = "cube.stl";
          currentStlBase64 = arrayBufferToBase64(buffer);
          loadStlToScene(buffer);
        })
        .catch(() => {});
    });
  </script>
</body>
</html>
"#;
