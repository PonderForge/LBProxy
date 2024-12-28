use geo::{coord, line_string, Area, BooleanOps, Coord, EuclideanDistance, LineString, Polygon};

/// Minimum Bounding Rectangle.
#[derive(Clone, PartialEq)]
pub struct Mbr {
    ls: LineString,
    id: isize,
    confidence: f32,
    name: Option<String>,
}

impl Default for Mbr {
    fn default() -> Self {
        Self {
            ls: line_string![],
            id: -1,
            confidence: 0.,
            name: None,
        }
    }
}

impl std::fmt::Debug for Mbr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Mbr")
            .field("vertices", &self.ls)
            .field("id", &self.id)
            .field("name", &self.name)
            .field("confidence", &self.confidence)
            .finish()
    }
}

impl Mbr {
    /// Returns the confidence score of the bounding box.
    pub fn confidence(&self) -> f32 {
        self.confidence
    }

    /// Computes the intersection over union (IoU) between this bounding box and another.
    pub fn iou(&self, other: &Self) -> f32 {
        self.intersect(other) / self.union(other)
    }
    /// Build from (cx, cy, width, height, degrees)
    pub fn from_cxcywhd(cx: f64, cy: f64, w: f64, h: f64, d: f64) -> Self {
        Self::from_cxcywhr(cx, cy, w, h, d.to_radians())
    }

    /// Build from (cx, cy, width, height, radians)
    pub fn from_cxcywhr(cx: f64, cy: f64, w: f64, h: f64, r: f64) -> Self {
        // [[cos -sin], [sin cos]]
        let m = [
            [r.cos() * 0.5 * w, -r.sin() * 0.5 * h],
            [r.sin() * 0.5 * w, r.cos() * 0.5 * h],
        ];
        let c = coord! {
            x: cx,
            y: cy,
        };

        let a_ = coord! {
            x: m[0][0] + m[0][1],
            y: m[1][0] + m[1][1],
        };

        let b_ = coord! {
            x: m[0][0] - m[0][1],
            y: m[1][0] - m[1][1],
        };

        let v1 = c + a_;
        let v2 = c + b_;
        let v3 = c * 2. - v1;
        let v4 = c * 2. - v2;

        Self {
            ls: vec![v1, v2, v3, v4].into(),
            ..Default::default()
        }
    }

    pub fn vertices(&self) -> Vec<Coord> {
        self.ls.0.clone()
    }

    pub fn top(&self) -> &Coord {
        self.ls
            .0
            .iter()
            .min_by(|a, b| a.y.partial_cmp(&b.y).unwrap())
            .unwrap()
    }

    pub fn xmin(&self) -> f32 {
        self.ls
            .0
            .iter()
            .min_by(|a, b| a.x.partial_cmp(&b.x).unwrap())
            .unwrap()
            .x as f32
    }

    pub fn ymin(&self) -> f32 {
        self.ls
            .0
            .iter()
            .min_by(|a, b| a.y.partial_cmp(&b.y).unwrap())
            .unwrap()
            .y as f32
    }

    pub fn xmax(&self) -> f32 {
        self.ls
            .0
            .iter()
            .max_by(|a, b| a.x.partial_cmp(&b.x).unwrap())
            .unwrap()
            .x as f32
    }

    pub fn ymax(&self) -> f32 {
        self.ls
            .0
            .iter()
            .max_by(|a, b| a.y.partial_cmp(&b.y).unwrap())
            .unwrap()
            .y as f32
    }
    pub fn intersect(&self, other: &Mbr) -> f32 {
        let p1 = Polygon::new(self.ls.clone(), vec![]);
        let p2 = Polygon::new(other.ls.clone(), vec![]);
        p1.intersection(&p2).unsigned_area() as f32
    }

    pub fn union(&self, other: &Mbr) -> f32 {
        let p1 = Polygon::new(self.ls.clone(), vec![]);
        let p2 = Polygon::new(other.ls.clone(), vec![]);
        p1.union(&p2).unsigned_area() as f32
    }
}