use std::io::{self, Write};
use serde::{Deserialize, Serialize};

use crate::geom::{Point2, Polygon2, Segment2};
use crate::infill::InfillPattern;
use crate::perimeter::SeamPosition;

#[derive(Clone, Debug)]
pub struct PrintConfig {
    pub nozzle_temp: u32,
    pub bed_temp: u32,
    pub layer_height: f64,
    pub line_width: f64,
    pub filament_diameter: f64,
    pub filament_density: f64,      // g/cm3 (PLA: 1.24, PETG: 1.27)
    pub flow_ratio: f64,            // Extrusion multiplier (e.g. 1.0 or 0.96)
    pub bed_x: f64,                 // mm (e.g. 256 for Bambu A1)
    pub bed_y: f64,                 // mm (e.g. 256 for Bambu A1)
    pub bed_z: f64,                 // mm (e.g. 256 for Bambu A1)
    pub print_speed_perimeter: f64, // mm/s
    pub print_speed_infill: f64,    // mm/s
    pub first_layer_speed: f64,     // mm/s
    pub travel_speed: f64,          // mm/s
    pub acceleration: f64,          // mm/s^2
    pub retract_dist: f64,          // mm
    pub retract_speed: f64,         // mm/s
    pub z_hop: f64,                 // mm
    pub top_solid_layers: usize,
    pub bottom_solid_layers: usize,
    pub skirt_loops: usize,
    pub skirt_distance: f64,        // mm
    pub brim_width: f64,            // mm
    pub fan_speed: u8,              // 0-255 PWM
    pub fan_below_layer: usize,     // layer threshold where fan turns on
    pub infill_pattern: InfillPattern,
    pub support_enabled: bool,
    pub support_angle: f64,         // degrees
    pub support_density: f64,
    pub seam_position: SeamPosition,
    pub adaptive_layers: bool,
    pub adaptive_layer_min: f64,
    pub adaptive_layer_max: f64,
    pub spiral_vase: bool,
    pub start_gcode: Option<String>,
    pub end_gcode: Option<String>,
    pub machine_name: String,
    pub material_name: String,
}

impl Default for PrintConfig {
    fn default() -> Self {
        Self {
            nozzle_temp: 210,
            bed_temp: 60,
            layer_height: 0.20,
            line_width: 0.45,
            filament_diameter: 1.75,
            filament_density: 1.24,
            flow_ratio: 1.0,
            bed_x: 220.0,
            bed_y: 220.0,
            bed_z: 250.0,
            print_speed_perimeter: 45.0,
            print_speed_infill: 60.0,
            first_layer_speed: 25.0,
            travel_speed: 150.0,
            acceleration: 1200.0,
            retract_dist: 0.8,
            retract_speed: 35.0,
            z_hop: 0.2,
            top_solid_layers: 4,
            bottom_solid_layers: 4,
            skirt_loops: 2,
            skirt_distance: 4.0,
            brim_width: 0.0,
            fan_speed: 255,
            fan_below_layer: 1,
            infill_pattern: InfillPattern::Rectilinear,
            support_enabled: false,
            support_angle: 45.0,
            support_density: 0.15,
            seam_position: SeamPosition::Aligned,
            adaptive_layers: false,
            adaptive_layer_min: 0.08,
            adaptive_layer_max: 0.28,
            spiral_vase: false,
            start_gcode: None,
            end_gcode: None,
            machine_name: "Generic Cartesian".to_string(),
            material_name: "Generic PLA".to_string(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PrintEstimates {
    pub print_time_seconds: f64,
    pub filament_used_mm: f64,
    pub filament_used_grams: f64,
    pub print_distance_mm: f64,
    pub travel_distance_mm: f64,
}

pub struct ProcessedLayer {
    pub layer_index: usize,
    pub z: f64,
    pub layer_height: f64,
    pub skirt_brim: Vec<Polygon2>,
    pub supports: Vec<Polygon2>,
    pub support_infill: Vec<Segment2>,
    pub perimeters: Vec<Vec<Polygon2>>, // per-contour perimeter loops
    pub infill: Vec<Segment2>,
}

pub struct GCodeWriter<'a> {
    config: &'a PrintConfig,
    current_pos: Point2,
    current_z: f64,
    is_retracted: bool,
    filament_area: f64,
    total_e: f64,
    total_time_secs: f64,
    total_print_dist_mm: f64,
    total_travel_dist_mm: f64,
}

impl<'a> GCodeWriter<'a> {
    pub fn new(config: &'a PrintConfig) -> Self {
        let r = config.filament_diameter * 0.5;
        let filament_area = std::f64::consts::PI * r * r;
        Self {
            config,
            current_pos: Point2::new(0.0, 0.0),
            current_z: 0.0,
            is_retracted: false,
            filament_area,
            total_e: 0.0,
            total_time_secs: 0.0,
            total_print_dist_mm: 0.0,
            total_travel_dist_mm: 0.0,
        }
    }

    pub fn write_gcode<W: Write>(
        &mut self,
        writer: &mut W,
        layers: &[ProcessedLayer],
    ) -> io::Result<PrintEstimates> {
        self.write_header(writer)?;

        for layer in layers {
            self.write_layer(writer, layer)?;
        }

        let estimates = self.compute_estimates();
        self.write_footer(writer, &estimates)?;

        Ok(estimates)
    }

    fn write_header<W: Write>(&mut self, w: &mut W) -> io::Result<()> {
        writeln!(w, "; Generated by Tetanus-Slicer")?;
        writeln!(w, "; Machine: {}", self.config.machine_name)?;
        writeln!(w, "; Material: {}", self.config.material_name)?;
        writeln!(w, "; Fast multi-threaded slicing engine written in Rust")?;

        if let Some(ref start) = self.config.start_gcode {
            let expanded = start
                .replace("{nozzle_temp}", &self.config.nozzle_temp.to_string())
                .replace("{bed_temp}", &self.config.bed_temp.to_string())
                .replace("{travel_speed}", &(self.config.travel_speed * 60.0).to_string())
                .replace("{layer_height}", &format!("{:.3}", self.config.layer_height));
            for line in expanded.lines() {
                writeln!(w, "{}", line)?;
            }
        } else {
            writeln!(w, "G21 ; metric values")?;
            writeln!(w, "G90 ; absolute positioning")?;
            writeln!(w, "M83 ; relative extrusion distances")?;
            writeln!(w, "M104 S{} ; set nozzle temp", self.config.nozzle_temp)?;
            writeln!(w, "M140 S{} ; set bed temp", self.config.bed_temp)?;
            writeln!(w, "M190 S{} ; wait for bed temp", self.config.bed_temp)?;
            writeln!(w, "M109 S{} ; wait for nozzle temp", self.config.nozzle_temp)?;
            writeln!(w, "G28 ; home all axes")?;
            writeln!(w, "G92 E0 ; reset extruder")?;
            writeln!(w, "; Prime nozzle purge line")?;
            writeln!(w, "G1 Z0.3 F1000")?;
            writeln!(w, "G1 X10 Y10 F{}", self.config.travel_speed * 60.0)?;
            writeln!(w, "G1 X100 E8.0 F1200")?;
            writeln!(w, "G1 X100 Y10.5 F{}", self.config.travel_speed * 60.0)?;
            writeln!(w, "G1 X10 E16.0 F1200")?;
            writeln!(w, "G92 E0")?;
        }

        // Prime move stats
        self.total_print_dist_mm += 180.0;
        self.total_e += 24.0;
        self.total_time_secs += 25.0; // purge time

        Ok(())
    }

    fn write_layer<W: Write>(&mut self, w: &mut W, layer: &ProcessedLayer) -> io::Result<()> {
        writeln!(w, "\n; ---------------------------------")?;
        writeln!(
            w,
            "; LAYER: {} | Z = {:.3}",
            layer.layer_index, layer.z
        )?;
        writeln!(w, "; ---------------------------------")?;

        // Cooling fan control
        if layer.layer_index == 0 {
            writeln!(w, "M107 ; cooling fan off for first layer")?;
        } else if layer.layer_index == self.config.fan_below_layer {
            writeln!(w, "M106 S{} ; enable cooling fan", self.config.fan_speed)?;
        }

        let is_spiral_layer = self.config.spiral_vase && layer.layer_index >= self.config.bottom_solid_layers;

        // Discrete layer move (not used during continuous spiral ascent)
        if !is_spiral_layer {
            writeln!(w, "G1 Z{:.3} F{}", layer.z, self.config.travel_speed * 60.0)?;
            self.total_time_secs += layer.layer_height / 10.0; // Z move time
            self.current_z = layer.z;
        }

        let (perimeter_speed, infill_speed) = if layer.layer_index == 0 {
            (self.config.first_layer_speed, self.config.first_layer_speed)
        } else {
            (self.config.print_speed_perimeter, self.config.print_speed_infill)
        };

        let infill_feedrate = infill_speed * 60.0;

        // 1. Skirt / Brim (Layer 0)
        if !layer.skirt_brim.is_empty() {
            writeln!(w, "; Skirt & Brim")?;
            for poly in &layer.skirt_brim {
                self.trace_polygon(w, poly, perimeter_speed, layer.z, layer.layer_height)?;
            }
        }

        if is_spiral_layer {
            writeln!(w, "; Spiral Vase Mode - Continuous Z Ascent")?;
            if let Some(primary_contour) = layer.perimeters.iter().max_by_key(|c| {
                c.first().map(|p| (p.signed_area().abs() * 100.0) as i64).unwrap_or(0)
            }) {
                if let Some(outer_poly) = primary_contour.first() {
                    let aligned_poly = crate::perimeter::PerimeterGenerator::align_seam(
                        outer_poly,
                        crate::perimeter::SeamPosition::Nearest,
                        self.current_pos,
                        layer.layer_index,
                        0,
                    );
                    self.trace_spiral_polygon(w, &aligned_poly, perimeter_speed, layer.layer_height)?;
                }
            }
            return Ok(());
        }

        // 2. Supports
        if !layer.supports.is_empty() {
            writeln!(w, "; Supports")?;
            for poly in &layer.supports {
                self.trace_polygon(w, poly, perimeter_speed, layer.z, layer.layer_height)?;
            }
        }
        if !layer.support_infill.is_empty() {
            writeln!(w, "; Support Infill")?;
            for seg in &layer.support_infill {
                self.travel_to(w, seg.p1, layer.z)?;
                let dist = seg.length();
                let e = self.calculate_e(dist, layer.layer_height);
                self.total_print_dist_mm += dist;
                self.total_e += e;
                self.total_time_secs += move_time(dist, infill_speed, 1200.0);
                writeln!(
                    w,
                    "G1 X{:.3} Y{:.3} E{:.4} F{:.1}",
                    seg.p2.x, seg.p2.y, e, infill_feedrate
                )?;
                self.current_pos = seg.p2;
            }
        }

        // 3. Perimeters (Walls)
        for (c_idx, contour_perimeters) in layer.perimeters.iter().enumerate() {
            for poly in contour_perimeters {
                let aligned_poly = crate::perimeter::PerimeterGenerator::align_seam(
                    poly,
                    self.config.seam_position,
                    self.current_pos,
                    layer.layer_index,
                    c_idx,
                );
                self.trace_polygon(w, &aligned_poly, perimeter_speed, layer.z, layer.layer_height)?;
            }
        }

        // 4. Infill
        if !layer.infill.is_empty() {
            writeln!(w, "; Infill")?;
            for seg in &layer.infill {
                self.travel_to(w, seg.p1, layer.z)?;
                let dist = seg.length();
                let e = self.calculate_e(dist, layer.layer_height);
                self.total_print_dist_mm += dist;
                self.total_e += e;
                self.total_time_secs += move_time(dist, infill_speed, 1200.0);
                writeln!(
                    w,
                    "G1 X{:.3} Y{:.3} E{:.4} F{:.1}",
                    seg.p2.x, seg.p2.y, e, infill_feedrate
                )?;
                self.current_pos = seg.p2;
            }
        }

        Ok(())
    }

    fn trace_spiral_polygon<W: Write>(
        &mut self,
        w: &mut W,
        poly: &Polygon2,
        speed: f64,
        layer_height: f64,
    ) -> io::Result<()> {
        let n = poly.points.len();
        if n < 2 {
            return Ok(());
        }

        // Calculate total perimeter length
        let mut total_perimeter_len = 0.0;
        for i in 0..n {
            let p1 = poly.points[i];
            let p2 = poly.points[(i + 1) % n];
            total_perimeter_len += p1.distance_to(p2);
        }

        if total_perimeter_len <= 1e-4 {
            return Ok(());
        }

        // If not already at starting point, travel to it at current Z
        if self.current_pos.distance_to(poly.points[0]) > 0.05 {
            self.travel_to(w, poly.points[0], self.current_z)?;
        }

        let feedrate = speed * 60.0;

        for i in 0..n {
            let p1 = poly.points[i];
            let p2 = poly.points[(i + 1) % n];
            let seg_len = p1.distance_to(p2);
            if seg_len <= 1e-4 {
                continue;
            }

            let delta_z = layer_height * (seg_len / total_perimeter_len);
            let next_z = self.current_z + delta_z;
            let move_dist = (seg_len * seg_len + delta_z * delta_z).sqrt();
            let e = self.calculate_e(move_dist, layer_height);

            self.total_print_dist_mm += move_dist;
            self.total_e += e;
            self.total_time_secs += move_time(move_dist, speed, 1200.0);

            writeln!(
                w,
                "G1 X{:.3} Y{:.3} Z{:.3} E{:.4} F{:.1}",
                p2.x, p2.y, next_z, e, feedrate
            )?;

            self.current_pos = p2;
            self.current_z = next_z;
        }

        Ok(())
    }

    fn trace_polygon<W: Write>(
        &mut self,
        w: &mut W,
        poly: &Polygon2,
        speed: f64,
        layer_z: f64,
        layer_height: f64,
    ) -> io::Result<()> {
        let n = poly.points.len();
        if n < 2 {
            return Ok(());
        }

        self.travel_to(w, poly.points[0], layer_z)?;
        let feedrate = speed * 60.0;

        let commands = crate::arc::ArcFitter::fit_arcs(&poly.points, true, 0.02);

        for cmd in commands {
            match cmd {
                crate::arc::PathSegment::Linear { end } => {
                    let dist = self.current_pos.distance_to(end);
                    let e = self.calculate_e(dist, layer_height);
                    self.total_print_dist_mm += dist;
                    self.total_e += e;
                    self.total_time_secs += move_time(dist, speed, 1200.0);
                    writeln!(
                        w,
                        "G1 X{:.3} Y{:.3} E{:.4} F{:.1}",
                        end.x, end.y, e, feedrate
                    )?;
                    self.current_pos = end;
                }
                crate::arc::PathSegment::Arc { end, i, j, radius, clockwise } => {
                    let start = self.current_pos;
                    let center = Point2::new(start.x + i, start.y + j);
                    let a1 = (start.y - center.y).atan2(start.x - center.x);
                    let a2 = (end.y - center.y).atan2(end.x - center.x);
                    let mut d_theta = if clockwise { a1 - a2 } else { a2 - a1 };
                    while d_theta < 0.0 { d_theta += std::f64::consts::TAU; }
                    while d_theta >= std::f64::consts::TAU { d_theta -= std::f64::consts::TAU; }
                    let arc_len = radius * d_theta;

                    let e = self.calculate_e(arc_len, layer_height);
                    self.total_print_dist_mm += arc_len;
                    self.total_e += e;
                    self.total_time_secs += move_time(arc_len, speed, 1200.0);

                    let g_cmd = if clockwise { "G2" } else { "G3" };
                    writeln!(
                        w,
                        "{} X{:.3} Y{:.3} I{:.3} J{:.3} E{:.4} F{:.1}",
                        g_cmd, end.x, end.y, i, j, e, feedrate
                    )?;
                    self.current_pos = end;
                }
            }
        }

        Ok(())
    }

    fn travel_to<W: Write>(&mut self, w: &mut W, target: Point2, layer_z: f64) -> io::Result<()> {
        let dist = self.current_pos.distance_to(target);
        if dist < 1e-4 {
            return Ok(());
        }

        // Retract before travel
        if !self.is_retracted && self.config.retract_dist > 0.0 {
            writeln!(
                w,
                "G1 E-{:.2} F{:.1}",
                self.config.retract_dist,
                self.config.retract_speed * 60.0
            )?;
            self.total_time_secs += self.config.retract_dist / self.config.retract_speed;
            self.is_retracted = true;

            // Z-Hop lift
            if self.config.z_hop > 0.0 {
                writeln!(
                    w,
                    "G1 Z{:.3} F{:.1}",
                    layer_z + self.config.z_hop,
                    self.config.travel_speed * 60.0
                )?;
            }
        }

        // Travel move
        writeln!(
            w,
            "G1 X{:.3} Y{:.3} F{:.1}",
            target.x,
            target.y,
            self.config.travel_speed * 60.0
        )?;
        self.total_travel_dist_mm += dist;
        self.total_time_secs += move_time(dist, self.config.travel_speed, 1500.0);
        self.current_pos = target;

        // Lower Z-Hop and unretract upon arrival
        if self.is_retracted {
            if self.config.z_hop > 0.0 {
                writeln!(
                    w,
                    "G1 Z{:.3} F{:.1}",
                    layer_z,
                    self.config.travel_speed * 60.0
                )?;
            }
            if self.config.retract_dist > 0.0 {
                writeln!(
                    w,
                    "G1 E{:.2} F{:.1}",
                    self.config.retract_dist,
                    self.config.retract_speed * 60.0
                )?;
                self.total_time_secs += self.config.retract_dist / self.config.retract_speed;
            }
            self.is_retracted = false;
        }

        Ok(())
    }

    fn calculate_e(&self, dist: f64, layer_height: f64) -> f64 {
        let bead_volume = dist * self.config.line_width * layer_height;
        (bead_volume / self.filament_area) * self.config.flow_ratio
    }

    fn compute_estimates(&self) -> PrintEstimates {
        let filament_vol_mm3 = self.total_e * self.filament_area;
        let filament_grams = filament_vol_mm3 * 0.001 * self.config.filament_density;
        let print_time_seconds = self.total_time_secs + 60.0; // 60s preheat buffer

        PrintEstimates {
            print_time_seconds,
            filament_used_mm: self.total_e,
            filament_used_grams: filament_grams,
            print_distance_mm: self.total_print_dist_mm,
            travel_distance_mm: self.total_travel_dist_mm,
        }
    }

    fn write_footer<W: Write>(&mut self, w: &mut W, est: &PrintEstimates) -> io::Result<()> {
        if let Some(ref end) = self.config.end_gcode {
            for line in end.lines() {
                writeln!(w, "{}", line)?;
            }
        } else {
            writeln!(w, "\n; End G-code")?;
            writeln!(w, "M104 S0 ; turn off nozzle")?;
            writeln!(w, "M140 S0 ; turn off bed")?;
            writeln!(w, "M107 ; turn off fan")?;
            writeln!(w, "G91 ; relative positioning")?;
            writeln!(w, "G1 E-2 F1800 ; retract filament")?;
            writeln!(w, "G1 Z10 F3000 ; lift nozzle")?;
            writeln!(w, "G90 ; absolute positioning")?;
            writeln!(w, "G28 X0 Y0 ; home X and Y")?;
            writeln!(w, "M84 ; disable motors")?;
        }

        let hours = (est.print_time_seconds / 3600.0).floor() as u64;
        let mins = ((est.print_time_seconds % 3600.0) / 60.0).floor() as u64;
        let secs = (est.print_time_seconds % 60.0).floor() as u64;
        let meters = est.filament_used_mm / 1000.0;

        writeln!(w, "\n; ============================================================")?;
        writeln!(w, "; Print Statistics & Estimates")?;
        writeln!(w, "; Estimated Print Time: {}h {}m {}s", hours, mins, secs)?;
        writeln!(w, "; Filament Used: {:.2} m ({:.1} g)", meters, est.filament_used_grams)?;
        writeln!(w, "; ============================================================")?;

        Ok(())
    }
}

fn move_time(dist: f64, speed: f64, accel: f64) -> f64 {
    if dist <= 1e-4 {
        return 0.0;
    }
    let d_accel = (speed * speed) / (2.0 * accel);
    if 2.0 * d_accel <= dist {
        2.0 * (speed / accel) + (dist - 2.0 * d_accel) / speed
    } else {
        2.0 * (dist / accel).sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::{Point2, Polygon2};

    #[test]
    fn test_spiral_vase_continuous_z() {
        let config = PrintConfig {
            spiral_vase: true,
            bottom_solid_layers: 1,
            layer_height: 0.2,
            ..PrintConfig::default()
        };

        let mut writer = GCodeWriter::new(&config);
        let mut buf = Vec::new();

        // Layer 0: flat bottom shell
        let square = Polygon2::new(vec![
            Point2::new(10.0, 10.0),
            Point2::new(20.0, 10.0),
            Point2::new(20.0, 20.0),
            Point2::new(10.0, 20.0),
        ]);
        let layer0 = ProcessedLayer {
            layer_index: 0,
            z: 0.2,
            layer_height: 0.2,
            skirt_brim: Vec::new(),
            supports: Vec::new(),
            support_infill: Vec::new(),
            perimeters: vec![vec![square.clone()]],
            infill: Vec::new(),
        };
        writer.write_layer(&mut buf, &layer0).unwrap();

        // Layer 1: spiral mode layer
        let layer1 = ProcessedLayer {
            layer_index: 1,
            z: 0.4,
            layer_height: 0.2,
            skirt_brim: Vec::new(),
            supports: Vec::new(),
            support_infill: Vec::new(),
            perimeters: vec![vec![square.clone()]],
            infill: Vec::new(),
        };
        writer.write_layer(&mut buf, &layer1).unwrap();

        let gcode = String::from_utf8(buf).unwrap();
        assert!(gcode.contains("; Spiral Vase Mode - Continuous Z Ascent"));

        // Verify that moves in layer 1 have continuous simultaneous Z and X/Y coordinates
        let spiral_section = gcode.split("; Spiral Vase Mode - Continuous Z Ascent").nth(1).unwrap();
        let mut found_z_moves = 0;
        let mut last_z = 0.2;
        for line in spiral_section.lines() {
            if line.starts_with("G1 X") && line.contains(" Z") {
                found_z_moves += 1;
                // Parse Z coordinate
                let z_part = line.split(" Z").nth(1).unwrap().split(' ').next().unwrap();
                let z_val: f64 = z_part.parse().unwrap();
                assert!(z_val >= last_z, "Z must ascend monotonically in spiral mode: {} < {}", z_val, last_z);
                last_z = z_val;
            }
        }
        assert!(found_z_moves >= 4, "Expected at least 4 spiral perimeter segments with Z move");
        assert!((last_z - 0.4).abs() < 1e-3, "Spiral layer should finish at layer Z (0.4), got {}", last_z);
    }
}
