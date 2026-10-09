#![allow(dead_code)]

use std::ops::{Add, Sub, Mul, Div};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Point3 {
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    pub fn to_2d(self) -> Point2 {
        Point2::new(self.x, self.y)
    }
}

impl Add for Point3 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }
}

impl Sub for Point3 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}

impl Mul<f64> for Point3 {
    type Output = Self;
    fn mul(self, rhs: f64) -> Self {
        Self::new(self.x * rhs, self.y * rhs, self.z * rhs)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point2 {
    pub x: f64,
    pub y: f64,
}

impl Point2 {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn distance_to(self, other: Self) -> f64 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }

    pub fn length(self) -> f64 {
        (self.x.powi(2) + self.y.powi(2)).sqrt()
    }

    pub fn normalized(self) -> Self {
        let len = self.length();
        if len > 1e-9 {
            Self::new(self.x / len, self.y / len)
        } else {
            Self::new(0.0, 0.0)
        }
    }

    pub fn dot(self, other: Self) -> f64 {
        self.x * other.x + self.y * other.y
    }

    pub fn cross(self, other: Self) -> f64 {
        self.x * other.y - self.y * other.x
    }

    pub fn quantize(self, grid_size: f64) -> (i64, i64) {
        ((self.x / grid_size).round() as i64, (self.y / grid_size).round() as i64)
    }
}

impl Add for Point2 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl Sub for Point2 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl Mul<f64> for Point2 {
    type Output = Self;
    fn mul(self, rhs: f64) -> Self {
        Self::new(self.x * rhs, self.y * rhs)
    }
}

impl Div<f64> for Point2 {
    type Output = Self;
    fn div(self, rhs: f64) -> Self {
        Self::new(self.x / rhs, self.y / rhs)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Triangle {
    pub normal: Point3,
    pub v: [Point3; 3],
}

impl Triangle {
    pub fn min_z(&self) -> f64 {
        self.v[0].z.min(self.v[1].z).min(self.v[2].z)
    }

    pub fn max_z(&self) -> f64 {
        self.v[0].z.max(self.v[1].z).max(self.v[2].z)
    }

    pub fn bounds_3d(&self) -> (Point3, Point3) {
        let min_x = self.v[0].x.min(self.v[1].x).min(self.v[2].x);
        let max_x = self.v[0].x.max(self.v[1].x).max(self.v[2].x);
        let min_y = self.v[0].y.min(self.v[1].y).min(self.v[2].y);
        let max_y = self.v[0].y.max(self.v[1].y).max(self.v[2].y);
        let min_z = self.min_z();
        let max_z = self.max_z();
        (Point3::new(min_x, min_y, min_z), Point3::new(max_x, max_y, max_z))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Segment2 {
    pub p1: Point2,
    pub p2: Point2,
}

impl Segment2 {
    pub const fn new(p1: Point2, p2: Point2) -> Self {
        Self { p1, p2 }
    }

    pub fn length(&self) -> f64 {
        self.p1.distance_to(self.p2)
    }
}

#[derive(Clone, Debug)]
pub struct Polygon2 {
    pub points: Vec<Point2>,
}

impl Polygon2 {
    pub fn new(points: Vec<Point2>) -> Self {
        Self { points }
    }

    /// Signed area using the Shoelace formula.
    /// Positive area indicates counter-clockwise (CCW) winding (outer boundary).
    /// Negative area indicates clockwise (CW) winding (hole).
    pub fn signed_area(&self) -> f64 {
        let n = self.points.len();
        if n < 3 {
            return 0.0;
        }
        let mut area = 0.0;
        for i in 0..n {
            let j = (i + 1) % n;
            area += self.points[i].cross(self.points[j]);
        }
        area * 0.5
    }

    pub fn is_ccw(&self) -> bool {
        self.signed_area() > 0.0
    }

    pub fn ensure_ccw(&mut self) {
        if self.signed_area() < 0.0 {
            self.points.reverse();
        }
    }

    pub fn bounds(&self) -> (Point2, Point2) {
        if self.points.is_empty() {
            return (Point2::new(0.0, 0.0), Point2::new(0.0, 0.0));
        }
        let mut min_x = f64::MAX;
        let mut max_x = f64::MIN;
        let mut min_y = f64::MAX;
        let mut max_y = f64::MIN;
        for p in &self.points {
            min_x = min_x.min(p.x);
            max_x = max_x.max(p.x);
            min_y = min_y.min(p.y);
            max_y = max_y.max(p.y);
        }
        (Point2::new(min_x, min_y), Point2::new(max_x, max_y))
    }

    pub fn contains_point(&self, pt: Point2) -> bool {
        let n = self.points.len();
        if n < 3 {
            return false;
        }
        let mut inside = false;
        let mut j = n - 1;
        for i in 0..n {
            let pi = self.points[i];
            let pj = self.points[j];
            if ((pi.y > pt.y) != (pj.y > pt.y))
                && (pt.x < (pj.x - pi.x) * (pt.y - pi.y) / (pj.y - pi.y + 1e-12) + pi.x)
            {
                inside = !inside;
            }
            j = i;
        }
        inside
    }

    pub fn min_distance_to_boundary(&self, pt: Point2) -> f64 {
        let n = self.points.len();
        if n == 0 {
            return f64::MAX;
        }
        if n == 1 {
            return pt.distance_to(self.points[0]);
        }
        let mut min_d = f64::MAX;
        for i in 0..n {
            let p1 = self.points[i];
            let p2 = self.points[(i + 1) % n];
            let v = p2 - p1;
            let len_sq = v.x * v.x + v.y * v.y;
            if len_sq < 1e-9 {
                min_d = min_d.min(pt.distance_to(p1));
                continue;
            }
            let t = ((pt.x - p1.x) * v.x + (pt.y - p1.y) * v.y) / len_sq;
            let closest = if t <= 0.0 {
                p1
            } else if t >= 1.0 {
                p2
            } else {
                Point2::new(p1.x + t * v.x, p1.y + t * v.y)
            };
            min_d = min_d.min(pt.distance_to(closest));
        }
        min_d
    }

    pub fn distance_to_solid(&self, pt: Point2) -> f64 {
        if self.contains_point(pt) {
            0.0
        } else {
            self.min_distance_to_boundary(pt)
        }
    }
}
