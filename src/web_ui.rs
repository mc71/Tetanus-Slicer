pub const INDEX_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Tetanus Slicer | High-Speed Rust 3D Slicing Engine</title>
  <script src="https://cdnjs.cloudflare.com/ajax/libs/three.js/r128/three.min.js"></script>
  <script src="https://cdn.jsdelivr.net/npm/three@0.128.0/examples/js/controls/OrbitControls.js"></script>
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
    </div>
  </header>

  <div id="canvas-container"></div>

  <!-- Sidebar -->
  <aside id="sidebar">
    <div class="dropzone" id="dropzone">
      <svg viewBox="0 0 24 24"><path d="M19.35 10.04C18.67 6.59 15.64 4 12 4 9.11 4 6.6 5.64 5.35 8.04 2.34 8.36 0 10.91 0 14c0 3.31 2.69 6 6 6h13c2.76 0 5-2.24 5-5 0-2.64-2.05-4.78-4.65-4.96zM14 13v4h-4v-4H7l5-5 5 5h-3z"/></svg>
      <p>Drop STL model here</p>
      <span>or click to browse file</span>
      <input type="file" id="file-input" accept=".stl" style="display:none">
    </div>

    <div id="model-info">
      <div class="row"><span class="label">Model:</span><span class="val" id="info-name">-</span></div>
      <div class="row"><span class="label">Dimensions:</span><span class="val" id="info-size">-</span></div>
      <div class="row"><span class="label">Triangles:</span><span class="val" id="info-triangles">-</span></div>
    </div>

    <div class="section-title">Print Quality</div>
    <div class="field">
      <div class="field-header"><label>Layer Height</label><span id="val-layer-height">0.20 mm</span></div>
      <input type="range" id="inp-layer-height" min="0.08" max="0.32" step="0.04" value="0.20">
    </div>
    <div class="field">
      <div class="field-header"><label>Wall Count (Perimeters)</label><span id="val-perimeters">2</span></div>
      <input type="range" id="inp-perimeters" min="1" max="5" step="1" value="2">
    </div>
    <div class="field">
      <div class="field-header"><label>Infill Density</label><span id="val-infill">20%</span></div>
      <input type="range" id="inp-infill" min="0" max="100" step="5" value="20">
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
        <div class="legend-item"><div class="legend-dot" style="background:#10b981"></div> Outer Wall</div>
        <div class="legend-item"><div class="legend-dot" style="background:#f59e0b"></div> Inner Wall</div>
        <div class="legend-item"><div class="legend-dot" style="background:#06b6d4"></div> Infill</div>
      </div>
      <div class="toggles">
        <label><input type="checkbox" id="chk-show-mesh" checked> Show Mesh</label>
        <label><input type="checkbox" id="chk-accumulate" checked> Build Up</label>
      </div>
    </div>
  </div>

  <script>
    // --- State ---
    let currentStlBase64 = null;
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

    const controls = new THREE.OrbitControls(camera, renderer.domElement);
    controls.enableDamping = true;
    controls.dampingFactor = 0.05;
    controls.target.set(110, 110, 20); // Center of 220x220 bed

    // --- Build Plate (220x220mm) ---
    const bedGroup = new THREE.Group();
    const bedGrid = new THREE.GridHelper(220, 22, 0x475569, 0x1e293b);
    bedGrid.rotation.x = Math.PI / 2;
    bedGrid.position.set(110, 110, 0);
    bedGroup.add(bedGrid);

    // Bed frame outline
    const bedBoxGeo = new THREE.BufferGeometry().setFromPoints([
      new THREE.Vector3(0, 0, 0), new THREE.Vector3(220, 0, 0),
      new THREE.Vector3(220, 220, 0), new THREE.Vector3(0, 220, 0),
      new THREE.Vector3(0, 0, 0)
    ]);
    const bedBoxMat = new THREE.LineBasicMaterial({ color: 0x64748b, linewidth: 2 });
    bedGroup.add(new THREE.Line(bedBoxGeo, bedBoxMat));

    // Bed surface plane
    const bedPlaneGeo = new THREE.PlaneGeometry(220, 220);
    const bedPlaneMat = new THREE.MeshBasicMaterial({ color: 0x0f172a, transparent: true, opacity: 0.6 });
    const bedPlane = new THREE.Mesh(bedPlaneGeo, bedPlaneMat);
    bedPlane.position.set(110, 110, -0.1);
    bedGroup.add(bedPlane);

    scene.add(bedGroup);
    scene.add(toolpathGroup);

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
    function linkSlider(id, targetId, unit, mult = 1) {
      const inp = document.getElementById(id);
      const val = document.getElementById(targetId);
      inp.addEventListener('input', () => {
        val.innerText = (parseFloat(inp.value) * mult).toFixed(mult < 1 ? 2 : 0) + unit;
      });
    }
    linkSlider('inp-layer-height', 'val-layer-height', ' mm', 1);
    linkSlider('inp-perimeters', 'val-perimeters', '', 1);
    linkSlider('inp-infill', 'val-infill', '%', 1);
    linkSlider('inp-speed', 'val-speed', ' mm/s', 1);
    linkSlider('inp-nozzle', 'val-nozzle', '°C', 1);

    // --- File Handling (Drop & Browse) ---
    const dropzone = document.getElementById('dropzone');
    const fileInput = document.getElementById('file-input');
    dropzone.addEventListener('click', () => fileInput.click());
    dropzone.addEventListener('dragover', (e) => { e.preventDefault(); dropzone.classList.add('dragover'); });
    dropzone.addEventListener('dragleave', () => dropzone.classList.remove('dragover'));
    dropzone.addEventListener('drop', (e) => {
      e.preventDefault();
      dropzone.classList.remove('dragover');
      if (e.dataTransfer.files.length > 0) handleFile(e.dataTransfer.files[0]);
    });
    fileInput.addEventListener('change', () => {
      if (fileInput.files.length > 0) handleFile(fileInput.files[0]);
    });

    function handleFile(file) {
      currentFileName = file.name;
      const reader = new FileReader();
      reader.onload = (e) => {
        const buffer = e.target.result;
        currentStlBase64 = arrayBufferToBase64(buffer);
        loadStlToScene(buffer);
      };
      reader.readAsArrayBuffer(file);
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

    // --- Client-side STL Parser & Renderer ---
    function loadStlToScene(buffer) {
      const geometry = parseSTL(buffer);
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

      geometry.translate(110 - centerX, 110 - centerY, -minZ);

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

    // --- Slicing API Request ---
    const sliceBtn = document.getElementById('slice-btn');
    const sliceSpinner = document.getElementById('slice-spinner');
    const sliceBtnText = document.getElementById('slice-btn-text');

    sliceBtn.addEventListener('click', async () => {
      sliceBtn.disabled = true;
      sliceSpinner.style.display = 'inline-block';
      sliceBtnText.innerText = 'Slicing in Rust...';

      const payload = {
        stl_base64: currentStlBase64,
        layer_height: parseFloat(document.getElementById('inp-layer-height').value),
        perimeters: parseInt(document.getElementById('inp-perimeters').value),
        infill_density: parseFloat(document.getElementById('inp-infill').value) / 100.0,
        print_speed: parseFloat(document.getElementById('inp-speed').value),
        nozzle_temp: parseInt(document.getElementById('inp-nozzle').value),
        bed_temp: 60
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
        alert('Slicing error: ' + err.message);
      } finally {
        sliceBtn.disabled = false;
        sliceSpinner.style.display = 'none';
        sliceBtnText.innerText = '⚡ Slice Model';
      }
    });

    function onSliceSuccess(data) {
      // Update stats pill
      document.getElementById('stat-time').innerText = `${data.stats.elapsed_ms.toFixed(1)} ms`;
      document.getElementById('stat-layers').innerText = data.stats.layer_count;
      document.getElementById('stats-pill').style.display = 'flex';

      // Setup scrubber
      const slider = document.getElementById('layer-slider');
      slider.min = 1;
      slider.max = data.layers.length;
      slider.value = data.layers.length;
      document.getElementById('scrubber-panel').style.display = 'flex';

      // Mesh transparency
      if (loadedMesh) loadedMesh.material.opacity = 0.25;

      renderToolpaths(data.layers.length);
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
      const startIdx = accumulate ? 0 : maxLayerIndex - 1;
      const endIdx = maxLayerIndex;

      const positions = [];
      const colors = [];

      const colOuter = new THREE.Color(0x10b981); // Emerald green
      const colInner = new THREE.Color(0xf59e0b); // Amber
      const colInfill = new THREE.Color(0x06b6d4); // Cyan

      for (let i = startIdx; i < endIdx; i++) {
        const layer = slicedData.layers[i];
        if (!layer) continue;
        const z = layer.z;

        // Perimeters
        if (layer.perimeters) {
          for (let pIdx = 0; pIdx < layer.perimeters.length; pIdx++) {
            const poly = layer.perimeters[pIdx];
            const color = (pIdx === 0) ? colOuter : colInner;
            const n = poly.length;
            for (let j = 0; j < n; j++) {
              const p1 = poly[j];
              const p2 = poly[(j + 1) % n];
              positions.push(p1[0], p1[1], z, p2[0], p2[1], z);
              colors.push(color.r, color.g, color.b, color.r, color.g, color.b);
            }
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
      a.download = currentFileName.replace(/\.stl$/i, '') + '.gcode';
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
