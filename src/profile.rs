use std::collections::HashMap;
use std::fs;
use std::path::Path;
use serde::{Deserialize, Serialize};

use crate::gcode::PrintConfig;
use crate::infill::InfillPattern;
use crate::perimeter::SeamPosition;

// Embedded fallbacks for zero-config portability across environments
pub const DEFAULT_BAMBU_A1_YAML: &str = include_str!("../profiles/machines/bambu_a1.yaml");
pub const DEFAULT_GENERIC_CARTESIAN_YAML: &str = include_str!("../profiles/machines/generic_cartesian.yaml");
pub const DEFAULT_PLA_YAML: &str = include_str!("../profiles/materials/pla.yaml");
pub const DEFAULT_PETG_YAML: &str = include_str!("../profiles/materials/petg.yaml");
pub const DEFAULT_STANDARD_PROCESS_YAML: &str = include_str!("../profiles/processes/standard_0.20.yaml");
pub const DEFAULT_FINE_PROCESS_YAML: &str = include_str!("../profiles/processes/fine_0.12.yaml");
pub const DEFAULT_DRAFT_PROCESS_YAML: &str = include_str!("../profiles/processes/draft_0.28.yaml");

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MachineProfile {
    pub name: String,
    pub id: String,
    #[serde(default)]
    pub manufacturer: Option<String>,
    #[serde(default = "default_bed_size")]
    pub bed_size: [f64; 3],
    #[serde(default)]
    pub nozzle_diameter: Option<f64>,
    #[serde(default)]
    pub max_print_speed: Option<f64>,
    #[serde(default)]
    pub travel_speed: Option<f64>,
    #[serde(default)]
    pub acceleration: Option<f64>,
    #[serde(default)]
    pub retract_dist: Option<f64>,
    #[serde(default)]
    pub retract_speed: Option<f64>,
    #[serde(default)]
    pub z_hop: Option<f64>,
    #[serde(default)]
    pub start_gcode: Option<String>,
    #[serde(default)]
    pub end_gcode: Option<String>,
}

fn default_bed_size() -> [f64; 3] {
    [220.0, 220.0, 250.0]
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MaterialProfile {
    pub name: String,
    pub id: String,
    #[serde(default)]
    pub material_type: Option<String>,
    #[serde(default)]
    pub density: Option<f64>,
    #[serde(default)]
    pub nozzle_temp: Option<u32>,
    #[serde(default)]
    pub bed_temp: Option<u32>,
    #[serde(default)]
    pub fan_speed: Option<u8>,
    #[serde(default)]
    pub fan_below_layer: Option<usize>,
    #[serde(default)]
    pub flow_ratio: Option<f64>,
    #[serde(default)]
    pub retract_dist: Option<f64>,
    #[serde(default)]
    pub max_print_speed: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProcessProfile {
    pub name: String,
    pub id: String,
    #[serde(default)]
    pub layer_height: Option<f64>,
    #[serde(default)]
    pub line_width: Option<f64>,
    #[serde(default)]
    pub perimeters: Option<usize>,
    #[serde(default)]
    pub top_solid_layers: Option<usize>,
    #[serde(default)]
    pub bottom_solid_layers: Option<usize>,
    #[serde(default)]
    pub infill_pattern: Option<InfillPattern>,
    #[serde(default)]
    pub infill_density: Option<f64>,
    #[serde(default)]
    pub print_speed_perimeter: Option<f64>,
    #[serde(default)]
    pub print_speed_infill: Option<f64>,
    #[serde(default)]
    pub first_layer_speed: Option<f64>,
    #[serde(default)]
    pub seam_position: Option<SeamPosition>,
}

#[derive(Clone, Debug, Default)]
pub struct ConfigOverrides {
    pub nozzle_temp: Option<u32>,
    pub bed_temp: Option<u32>,
    pub layer_height: Option<f64>,
    pub line_width: Option<f64>,
    pub perimeters: Option<usize>,
    pub infill_density: Option<f64>,
    pub infill_pattern: Option<InfillPattern>,
    pub print_speed: Option<f64>,
    pub top_solid_layers: Option<usize>,
    pub bottom_solid_layers: Option<usize>,
    pub skirt_loops: Option<usize>,
    pub brim_width: Option<f64>,
    pub z_hop: Option<f64>,
    pub fan_speed: Option<u8>,
    pub support_enabled: Option<bool>,
    pub support_angle: Option<f64>,
    pub seam_position: Option<SeamPosition>,
    pub adaptive_layers: Option<bool>,
    pub spiral_vase: Option<bool>,
}

#[derive(Clone, Debug)]
pub struct ProfileManager {
    pub machines: HashMap<String, MachineProfile>,
    pub materials: HashMap<String, MaterialProfile>,
    pub processes: HashMap<String, ProcessProfile>,
}

impl ProfileManager {
    pub fn new() -> Self {
        let mut mgr = Self {
            machines: HashMap::new(),
            materials: HashMap::new(),
            processes: HashMap::new(),
        };

        // Load built-in embedded fallbacks
        if let Ok(m) = serde_yaml::from_str::<MachineProfile>(DEFAULT_BAMBU_A1_YAML) {
            mgr.machines.insert(m.id.clone(), m);
        }
        if let Ok(m) = serde_yaml::from_str::<MachineProfile>(DEFAULT_GENERIC_CARTESIAN_YAML) {
            mgr.machines.insert(m.id.clone(), m);
        }

        if let Ok(m) = serde_yaml::from_str::<MaterialProfile>(DEFAULT_PLA_YAML) {
            mgr.materials.insert(m.id.clone(), m);
        }
        if let Ok(m) = serde_yaml::from_str::<MaterialProfile>(DEFAULT_PETG_YAML) {
            mgr.materials.insert(m.id.clone(), m);
        }

        if let Ok(p) = serde_yaml::from_str::<ProcessProfile>(DEFAULT_STANDARD_PROCESS_YAML) {
            mgr.processes.insert(p.id.clone(), p);
        }
        if let Ok(p) = serde_yaml::from_str::<ProcessProfile>(DEFAULT_FINE_PROCESS_YAML) {
            mgr.processes.insert(p.id.clone(), p);
        }
        if let Ok(p) = serde_yaml::from_str::<ProcessProfile>(DEFAULT_DRAFT_PROCESS_YAML) {
            mgr.processes.insert(p.id.clone(), p);
        }

        // Scan filesystem for user-defined or project profiles
        mgr.scan_directory("profiles");

        mgr
    }

    pub fn scan_directory<P: AsRef<Path>>(&mut self, base_dir: P) {
        let base = base_dir.as_ref();
        if !base.is_dir() {
            return;
        }

        let m_dir = base.join("machines");
        if m_dir.is_dir() {
            if let Ok(entries) = fs::read_dir(m_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("yaml") {
                        if let Ok(content) = fs::read_to_string(&path) {
                            if let Ok(m) = serde_yaml::from_str::<MachineProfile>(&content) {
                                self.machines.insert(m.id.clone(), m);
                            }
                        }
                    }
                }
            }
        }

        let mat_dir = base.join("materials");
        if mat_dir.is_dir() {
            if let Ok(entries) = fs::read_dir(mat_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("yaml") {
                        if let Ok(content) = fs::read_to_string(&path) {
                            if let Ok(m) = serde_yaml::from_str::<MaterialProfile>(&content) {
                                self.materials.insert(m.id.clone(), m);
                            }
                        }
                    }
                }
            }
        }

        let p_dir = base.join("processes");
        if p_dir.is_dir() {
            if let Ok(entries) = fs::read_dir(p_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("yaml") {
                        if let Ok(content) = fs::read_to_string(&path) {
                            if let Ok(p) = serde_yaml::from_str::<ProcessProfile>(&content) {
                                self.processes.insert(p.id.clone(), p);
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn find_machine(&self, query: &str) -> Option<MachineProfile> {
        let q_lower = query.trim().to_lowercase();
        if let Some(m) = self.machines.get(&q_lower) {
            return Some(m.clone());
        }
        for m in self.machines.values() {
            if m.id.to_lowercase() == q_lower || m.name.to_lowercase() == q_lower {
                return Some(m.clone());
            }
        }
        // Try file path
        let path = Path::new(query);
        if path.is_file() {
            if let Ok(content) = fs::read_to_string(path) {
                if let Ok(m) = serde_yaml::from_str::<MachineProfile>(&content) {
                    return Some(m);
                }
            }
        }
        None
    }

    pub fn find_material(&self, query: &str) -> Option<MaterialProfile> {
        let q_lower = query.trim().to_lowercase();
        if let Some(m) = self.materials.get(&q_lower) {
            return Some(m.clone());
        }
        for m in self.materials.values() {
            if m.id.to_lowercase() == q_lower || m.name.to_lowercase() == q_lower {
                return Some(m.clone());
            }
        }
        let path = Path::new(query);
        if path.is_file() {
            if let Ok(content) = fs::read_to_string(path) {
                if let Ok(m) = serde_yaml::from_str::<MaterialProfile>(&content) {
                    return Some(m);
                }
            }
        }
        None
    }

    pub fn find_process(&self, query: &str) -> Option<ProcessProfile> {
        let q_lower = query.trim().to_lowercase();
        if let Some(p) = self.processes.get(&q_lower) {
            return Some(p.clone());
        }
        for p in self.processes.values() {
            if p.id.to_lowercase() == q_lower || p.name.to_lowercase() == q_lower {
                return Some(p.clone());
            }
        }
        let path = Path::new(query);
        if path.is_file() {
            if let Ok(content) = fs::read_to_string(path) {
                if let Ok(p) = serde_yaml::from_str::<ProcessProfile>(&content) {
                    return Some(p);
                }
            }
        }
        None
    }

    /// Cascades profiles in priority order:
    /// Tier 1: Base Engine Defaults
    /// Tier 2: Machine Profile
    /// Tier 3: Material Profile
    /// Tier 4: Process / Quality Profile
    /// Tier 5: Explicit User / CLI Overrides
    pub fn cascade(
        &self,
        machine_req: Option<&str>,
        material_req: Option<&str>,
        process_req: Option<&str>,
        overrides: &ConfigOverrides,
    ) -> PrintConfig {
        // Tier 1: Hardcoded Engine Defaults
        let mut cfg = PrintConfig::default();

        // Tier 2: Machine Profile
        let machine_opt = machine_req.and_then(|m| self.find_machine(m));
        if let Some(m) = machine_opt {
            cfg.machine_name = m.name.clone();
            cfg.bed_x = m.bed_size[0];
            cfg.bed_y = m.bed_size[1];
            cfg.bed_z = m.bed_size[2];
            if let Some(travel) = m.travel_speed {
                cfg.travel_speed = travel;
            }
            if let Some(accel) = m.acceleration {
                cfg.acceleration = accel;
            }
            if let Some(retract) = m.retract_dist {
                cfg.retract_dist = retract;
            }
            if let Some(retract_spd) = m.retract_speed {
                cfg.retract_speed = retract_spd;
            }
            if let Some(zh) = m.z_hop {
                cfg.z_hop = zh;
            }
            if let Some(start) = m.start_gcode {
                cfg.start_gcode = Some(start);
            }
            if let Some(end) = m.end_gcode {
                cfg.end_gcode = Some(end);
            }
        }

        // Tier 3: Material Profile
        let material_opt = material_req.and_then(|m| self.find_material(m));
        if let Some(mat) = material_opt {
            cfg.material_name = mat.name.clone();
            if let Some(t) = mat.nozzle_temp {
                cfg.nozzle_temp = t;
            }
            if let Some(t) = mat.bed_temp {
                cfg.bed_temp = t;
            }
            if let Some(fan) = mat.fan_speed {
                cfg.fan_speed = fan;
            }
            if let Some(l) = mat.fan_below_layer {
                cfg.fan_below_layer = l;
            }
            if let Some(d) = mat.density {
                cfg.filament_density = d;
            }
            if let Some(flow) = mat.flow_ratio {
                cfg.flow_ratio = flow;
            }
            if let Some(retract) = mat.retract_dist {
                cfg.retract_dist = retract;
            }
            if let Some(max_spd) = mat.max_print_speed {
                if cfg.print_speed_perimeter > max_spd {
                    cfg.print_speed_perimeter = max_spd * 0.75;
                }
                if cfg.print_speed_infill > max_spd {
                    cfg.print_speed_infill = max_spd;
                }
            }
        }

        // Tier 4: Process Profile
        let process_opt = process_req.and_then(|p| self.find_process(p));
        if let Some(p) = process_opt {
            if let Some(lh) = p.layer_height {
                cfg.layer_height = lh;
            }
            if let Some(lw) = p.line_width {
                cfg.line_width = lw;
            }
            if let Some(_perims) = p.perimeters {
                // Handled in caller/overrides
            }
            if let Some(top) = p.top_solid_layers {
                cfg.top_solid_layers = top;
            }
            if let Some(bot) = p.bottom_solid_layers {
                cfg.bottom_solid_layers = bot;
            }
            if let Some(pat) = p.infill_pattern {
                cfg.infill_pattern = pat;
            }
            if let Some(spd_p) = p.print_speed_perimeter {
                cfg.print_speed_perimeter = spd_p;
            }
            if let Some(spd_i) = p.print_speed_infill {
                cfg.print_speed_infill = spd_i;
            }
            if let Some(spd_1) = p.first_layer_speed {
                cfg.first_layer_speed = spd_1;
            }
            if let Some(seam) = p.seam_position {
                cfg.seam_position = seam;
            }
        }

        // Tier 5: Explicit User / CLI Overrides
        if let Some(t) = overrides.nozzle_temp {
            cfg.nozzle_temp = t;
        }
        if let Some(t) = overrides.bed_temp {
            cfg.bed_temp = t;
        }
        if let Some(lh) = overrides.layer_height {
            cfg.layer_height = lh;
        }
        if let Some(lw) = overrides.line_width {
            cfg.line_width = lw;
        }
        if let Some(top) = overrides.top_solid_layers {
            cfg.top_solid_layers = top;
        }
        if let Some(bot) = overrides.bottom_solid_layers {
            cfg.bottom_solid_layers = bot;
        }
        if let Some(pat) = overrides.infill_pattern {
            cfg.infill_pattern = pat;
        }
        if let Some(spd) = overrides.print_speed {
            cfg.print_speed_perimeter = spd * 0.75;
            cfg.print_speed_infill = spd;
            cfg.first_layer_speed = (spd * 0.5).min(30.0);
        }
        if let Some(skirt) = overrides.skirt_loops {
            cfg.skirt_loops = skirt;
        }
        if let Some(brim) = overrides.brim_width {
            cfg.brim_width = brim;
        }
        if let Some(zh) = overrides.z_hop {
            cfg.z_hop = zh;
        }
        if let Some(fan) = overrides.fan_speed {
            cfg.fan_speed = fan;
        }
        if let Some(sup) = overrides.support_enabled {
            cfg.support_enabled = sup;
        }
        if let Some(ang) = overrides.support_angle {
            cfg.support_angle = ang;
        }
        if let Some(seam) = overrides.seam_position {
            cfg.seam_position = seam;
        }
        if let Some(ad) = overrides.adaptive_layers {
            cfg.adaptive_layers = ad;
        }
        if let Some(v) = overrides.spiral_vase {
            cfg.spiral_vase = v;
        }

        cfg
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_cascading_bambu_a1_petg() {
        let mgr = ProfileManager::new();

        // Check Bambu A1 + PETG cascade
        let config = mgr.cascade(
            Some("bambu_a1"),
            Some("petg"),
            Some("standard_0.20"),
            &ConfigOverrides::default(),
        );

        assert_eq!(config.machine_name, "Bambu Lab A1");
        assert_eq!(config.material_name, "Generic PETG");
        assert_eq!(config.bed_x, 256.0);
        assert_eq!(config.bed_y, 256.0);
        assert_eq!(config.nozzle_temp, 245);
        assert_eq!(config.bed_temp, 70);
        assert_eq!(config.fan_speed, 128);
        assert_eq!(config.fan_below_layer, 3);
        assert_eq!(config.retract_dist, 1.0); // PETG overrides machine 0.8
        assert_eq!(config.travel_speed, 400.0); // Bambu A1 travel speed
        assert!((config.filament_density - 1.27).abs() < 1e-4);

        // Verify Bambu start G-code has template placeholders filled in
        assert!(config.start_gcode.is_some());
        let start = config.start_gcode.as_ref().unwrap();
        assert!(start.contains("Bambu Lab A1"));
    }

    #[test]
    fn test_user_override_precedence() {
        let mgr = ProfileManager::new();

        let mut overrides = ConfigOverrides::default();
        overrides.nozzle_temp = Some(260); // User explicit tweak
        overrides.layer_height = Some(0.16);

        let config = mgr.cascade(
            Some("bambu_a1"),
            Some("pla"),
            Some("standard_0.20"),
            &overrides,
        );

        // PLA defaults to 210, but user override 260 must win!
        assert_eq!(config.nozzle_temp, 260);
        assert_eq!(config.layer_height, 0.16);
        assert_eq!(config.bed_x, 256.0);
    }
}
