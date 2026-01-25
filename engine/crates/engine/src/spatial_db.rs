use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

// --- Tipos Geométricos ---

#[derive(Clone, Debug)]
pub enum Shape {
    Circle { radius: f32 },
    Segment { x2: f32, y2: f32 }, // Relativo ao x1,y1 do objeto para simplificar movimento? Não, melhor absoluto para paredes estáticas e relativo para móveis. Vamos simplificar: Posição central + dados
}

#[derive(Clone, Debug)]
struct Entity {
    id: u64,
    x: f32,
    y: f32,
    // Se for segmento: x,y é o ponto inicial. data guarda o delta ou ponto final.
    // Para simplificar essa engine genérica, vamos ter tipos explícitos.
    kind: EntityKind,
    tag_hash: u64, // Hash da string "wall", "enemy", etc.
}

#[derive(Clone, Debug)]
pub enum EntityKind {
    Circle { radius: f32 },
    Segment { x2: f32, y2: f32 }, // x,y no Entity é o start. x2,y2 aqui é o end.
}

// --- Spatial DB ---

pub struct SpatialDb {
    next_id: u64,
    cell_size: f32,
    entities: HashMap<u64, Entity>,
    grid: HashMap<(i32, i32), Vec<u64>>, // Cell Coordinate -> List of Entity IDs
}

impl SpatialDb {
    pub fn new(cell_size: f32) -> Self {
        Self {
            next_id: 1,
            cell_size,
            entities: HashMap::new(),
            grid: HashMap::new(),
        }
    }

    fn calculate_hash(tag: &str) -> u64 {
        let mut s = DefaultHasher::new();
        tag.hash(&mut s);
        s.finish()
    }

    // --- Helpers de Grid ---

    fn get_cell(&self, x: f32, y: f32) -> (i32, i32) {
        (
            (x / self.cell_size).floor() as i32,
            (y / self.cell_size).floor() as i32,
        )
    }

    // Retorna as células que um objeto toca
    fn get_cells_for_entity(&self, e: &Entity) -> Vec<(i32, i32)> {
        let mut cells = HashSet::new();

        match e.kind {
            EntityKind::Circle { radius } => {
                let min_c = self.get_cell(e.x - radius, e.y - radius);
                let max_c = self.get_cell(e.x + radius, e.y + radius);
                for x in min_c.0..=max_c.0 {
                    for y in min_c.1..=max_c.1 {
                        cells.insert((x, y));
                    }
                }
            }
            EntityKind::Segment { x2, y2 } => {
                // Algoritmo de traço de linha simples (grid traversal) ou apenas AABB para simplificar
                // AABB para segmentos é seguro e fácil
                let min_x = e.x.min(x2);
                let max_x = e.x.max(x2);
                let min_y = e.y.min(y2);
                let max_y = e.y.max(y2);

                let min_c = self.get_cell(min_x, min_y);
                let max_c = self.get_cell(max_x, max_y);

                for x in min_c.0..=max_c.0 {
                    for y in min_c.1..=max_c.1 {
                        cells.insert((x, y));
                    }
                }
            }
        }
        cells.into_iter().collect()
    }

    fn add_to_grid(&mut self, id: u64) {
        if let Some(e) = self.entities.get(&id) {
            let cells = self.get_cells_for_entity(e);
            for cell in cells {
                self.grid.entry(cell).or_insert_with(Vec::new).push(id);
            }
        }
    }

    fn remove_from_grid(&mut self, id: u64) {
        if let Some(e) = self.entities.get(&id) {
            let cells = self.get_cells_for_entity(e);
            for cell in cells {
                if let Some(list) = self.grid.get_mut(&cell) {
                    if let Some(pos) = list.iter().position(|&x| x == id) {
                        list.swap_remove(pos);
                    }
                }
            }
        }
    }

    // --- Public API ---

    pub fn add_circle(&mut self, x: f32, y: f32, radius: f32, tag: &str) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let e = Entity {
            id,
            x,
            y,
            kind: EntityKind::Circle { radius },
            tag_hash: Self::calculate_hash(tag),
        };
        self.entities.insert(id, e);
        self.add_to_grid(id);
        id
    }

    pub fn add_segment(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, tag: &str) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let e = Entity {
            id,
            x: x1,
            y: y1,
            kind: EntityKind::Segment { x2, y2 },
            tag_hash: Self::calculate_hash(tag),
        };
        self.entities.insert(id, e);
        self.add_to_grid(id);
        id
    }

    pub fn get_position(&self, id: u64) -> Option<(f32, f32)> {
        self.entities.get(&id).map(|e| (e.x, e.y))
    }

    pub fn get_entity_info(&self, id: u64) -> Option<(f32, f32, EntityKind)> {
        self.entities.get(&id).map(|e| (e.x, e.y, e.kind.clone()))
    }

    pub fn update_position(&mut self, id: u64, x: f32, y: f32) {
        // Remove old position from grid, update, add new
        // Optimization: Check if cell changed? For now, brute force safety.
        if self.entities.contains_key(&id) {
            self.remove_from_grid(id);
            if let Some(e) = self.entities.get_mut(&id) {
                // Only valid for moving entities (usually Circles in this game context)
                // If moving a segment, we'd need to update x2/y2 too (offset).
                // Assuming simple translation for circles mostly.
                if let EntityKind::Segment { x2, y2 } = e.kind {
                    // If moving segment, we treat input x,y as new x1,y1 and shift x2,y2
                    let dx = x - e.x;
                    let dy = y - e.y;
                    let new_x2 = x2 + dx;
                    let new_y2 = y2 + dy;
                    e.kind = EntityKind::Segment {
                        x2: new_x2,
                        y2: new_y2,
                    };
                }
                e.x = x;
                e.y = y;
            }
            self.add_to_grid(id);
        }
    }

    pub fn remove(&mut self, id: u64) {
        if self.entities.contains_key(&id) {
            self.remove_from_grid(id);
            self.entities.remove(&id);
        }
    }

    // --- Queries ---

    pub fn query_rect(
        &self,
        min_x: f32,
        min_y: f32,
        max_x: f32,
        max_y: f32,
        tag_filter: Option<&str>,
    ) -> Vec<u64> {
        let mut result = HashSet::new();
        let target_hash = tag_filter.map(Self::calculate_hash);

        let min_c = self.get_cell(min_x, min_y);
        let max_c = self.get_cell(max_x, max_y);

        for cx in min_c.0..=max_c.0 {
            for cy in min_c.1..=max_c.1 {
                if let Some(list) = self.grid.get(&(cx, cy)) {
                    for &id in list {
                        if result.contains(&id) {
                            continue;
                        }

                        if let Some(e) = self.entities.get(&id) {
                            if let Some(th) = target_hash {
                                if e.tag_hash != th {
                                    continue;
                                }
                            }

                            // AABB Intersection check (Simple and fast for culling)
                            let (e_min_x, e_min_y, e_max_x, e_max_y) = match e.kind {
                                EntityKind::Circle { radius } => {
                                    (e.x - radius, e.y - radius, e.x + radius, e.y + radius)
                                }
                                EntityKind::Segment { x2, y2 } => {
                                    (e.x.min(x2), e.y.min(y2), e.x.max(x2), e.y.max(y2))
                                }
                            };

                            if e_max_x >= min_x
                                && e_min_x <= max_x
                                && e_max_y >= min_y
                                && e_min_y <= max_y
                            {
                                result.insert(id);
                            }
                        }
                    }
                }
            }
        }
        result.into_iter().collect()
    }

    pub fn query_range(&self, x: f32, y: f32, range: f32, tag_filter: Option<&str>) -> Vec<u64> {
        let mut result = HashSet::new();
        let target_hash = tag_filter.map(Self::calculate_hash);

        // AABB check first
        let min_c = self.get_cell(x - range, y - range);
        let max_c = self.get_cell(x + range, y + range);

        for cx in min_c.0..=max_c.0 {
            for cy in min_c.1..=max_c.1 {
                if let Some(list) = self.grid.get(&(cx, cy)) {
                    for &id in list {
                        if result.contains(&id) {
                            continue;
                        }

                        if let Some(e) = self.entities.get(&id) {
                            // Filter Tag
                            if let Some(th) = target_hash {
                                if e.tag_hash != th {
                                    continue;
                                }
                            }

                            // Precise Check
                            let dist_sq = match e.kind {
                                EntityKind::Circle { radius } => {
                                    // Distance between centers minus radius
                                    let dx = x - e.x;
                                    let dy = y - e.y;
                                    let d2 = dx * dx + dy * dy;
                                    let r_sum = range + radius;
                                    if d2 <= r_sum * r_sum {
                                        Some(d2)
                                    } else {
                                        None
                                    }
                                }
                                EntityKind::Segment { x2, y2 } => {
                                    // Point to Segment distance
                                    let seg_len2 = (x2 - e.x).powi(2) + (y2 - e.y).powi(2);
                                    let mut t = ((x - e.x) * (x2 - e.x) + (y - e.y) * (y2 - e.y))
                                        / seg_len2;
                                    t = t.max(0.0).min(1.0);
                                    let closest_x = e.x + t * (x2 - e.x);
                                    let closest_y = e.y + t * (y2 - e.y);
                                    let dist2 = (x - closest_x).powi(2) + (y - closest_y).powi(2);
                                    if dist2 <= range * range {
                                        Some(dist2)
                                    } else {
                                        None
                                    }
                                }
                            };

                            if dist_sq.is_some() {
                                result.insert(id);
                            }
                        }
                    }
                }
            }
        }

        result.into_iter().collect()
    }

    // Raycast simples (Naive traversal, optimizing via Grid cells is harder but doable)
    // Retorna (id, dist_fraction, x, y)
    pub fn cast_ray(
        &self,
        x1: f32,
        y1: f32,
        angle_deg: f32,
        max_dist: f32,
        tag_filter: Option<&str>,
    ) -> Option<(u64, f32, f32, f32)> {
        let rad = angle_deg.to_radians();
        let dx = rad.cos();
        let dy = rad.sin();
        let x2 = x1 + dx * max_dist;
        let y2 = y1 + dy * max_dist;

        let target_hash = tag_filter.map(Self::calculate_hash);

        // Ray Traversal (DDA-like or simple stepping)
        // Para simplificar e garantir robustez, vamos coletar candidatos via células atravessadas.
        let mut candidates = HashSet::new();

        // Passo de amostragem na grid (grosseiro mas funcional para 2D top down)
        let steps = (max_dist / self.cell_size).ceil() as i32;
        let step_x = dx * self.cell_size;
        let step_y = dy * self.cell_size;

        for i in 0..=steps {
            let cx = x1 + step_x * (i as f32);
            let cy = y1 + step_y * (i as f32);
            let cell = self.get_cell(cx, cy);

            // Check vizinhos para bordas
            for ox in -1..=1 {
                for oy in -1..=1 {
                    if let Some(list) = self.grid.get(&(cell.0 + ox, cell.1 + oy)) {
                        for &id in list {
                            candidates.insert(id);
                        }
                    }
                }
            }
        }

        let mut closest: Option<(u64, f32, f32, f32)> = None;

        for id in candidates {
            if let Some(e) = self.entities.get(&id) {
                if let Some(th) = target_hash {
                    if e.tag_hash != th {
                        continue;
                    }
                }

                match e.kind {
                    EntityKind::Circle { radius } => {
                        // Ray vs Circle
                        let fx = x1 - e.x;
                        let fy = y1 - e.y;
                        let a = dx * dx + dy * dy;
                        let b = 2.0 * (fx * dx + fy * dy);
                        let c = (fx * fx + fy * fy) - radius * radius;
                        let discriminant = b * b - 4.0 * a * c;
                        if discriminant >= 0.0 {
                            let t = (-b - discriminant.sqrt()) / (2.0 * a);
                            if t >= 0.0 && t <= max_dist {
                                let hit_dist = t / max_dist; // Normalize 0..1
                                if closest.map_or(true, |(_, cd, _, _)| hit_dist < cd) {
                                    closest = Some((id, hit_dist, x1 + dx * t, y1 + dy * t));
                                }
                            }
                        }
                    }
                    EntityKind::Segment { x2: wx2, y2: wy2 } => {
                        // Ray vs Segment
                        // Line-Line Intersection
                        let den = (x1 - x2) * (e.y - wy2) - (y1 - y2) * (e.x - wx2);
                        if den != 0.0 {
                            let t = ((x1 - e.x) * (e.y - wy2) - (y1 - e.y) * (e.x - wx2)) / den;
                            let u = -((x1 - x2) * (y1 - e.y) - (y1 - y2) * (x1 - e.x)) / den;

                            if t >= 0.0 && t <= 1.0 && u >= 0.0 && u <= 1.0 {
                                // t é fração do raio
                                if closest.map_or(true, |(_, cd, _, _)| t < cd) {
                                    closest = Some((id, t, x1 + t * (x2 - x1), y1 + t * (y2 - y1)));
                                }
                            }
                        }
                    }
                }
            }
        }

        closest
    }

    pub fn compute_visibility(
        &self,
        origin_x: f32,
        origin_y: f32,
        radius: f32,
        tag_filter: Option<&str>,
    ) -> Vec<(f32, f32)> {
        let _target_hash = tag_filter.map(Self::calculate_hash);

        // 1. Collect Segments
        let candidates = self.query_rect(
            origin_x - radius,
            origin_y - radius,
            origin_x + radius,
            origin_y + radius,
            tag_filter,
        );

        let mut segments = Vec::new();
        let mut points = Vec::new();

        for id in candidates {
            if let Some(e) = self.entities.get(&id) {
                if let EntityKind::Segment { x2, y2 } = e.kind {
                    segments.push((e.x, e.y, x2, y2));
                    points.push((e.x, e.y));
                    points.push((x2, y2));
                }
            }
        }

        // Add corners of the bounding box to ensure the polygon is closed/bounded
        // Actually, just sorting angles handles the order.
        // We just need to make sure we cast rays to the "corners" of the view if they are not blocked?
        // Standard algorithm usually includes the bounding box corners as "points" to cast at.
        points.push((origin_x - radius, origin_y - radius));
        points.push((origin_x + radius, origin_y - radius));
        points.push((origin_x + radius, origin_y + radius));
        points.push((origin_x - radius, origin_y + radius));

        let mut unique_angles = Vec::new();
        for (px, py) in points {
            let angle = (py - origin_y).atan2(px - origin_x);
            unique_angles.push(angle);
            unique_angles.push(angle - 0.0001);
            unique_angles.push(angle + 0.0001);
        }

        unique_angles.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let mut polygon = Vec::new();

        for angle in unique_angles {
            let dx = angle.cos();
            let dy = angle.sin();

            let mut min_t = radius;
            let mut hit_x = origin_x + dx * radius;
            let mut hit_y = origin_y + dy * radius;

            // Ray: origin + t * (dx, dy)
            // Segment: (sx1, sy1) + u * (sx2-sx1, sy2-sy1)

            for &(sx1, sy1, sx2, sy2) in &segments {
                let sdx = sx2 - sx1;
                let sdy = sy2 - sy1;
                let rdx = dx;
                let rdy = dy;

                // Solve: origin + t*rd = start + u*sd
                // t*rd - u*sd = start - origin

                // Det = rdx * (-sdy) - rdy * (-sdx) = -rdx*sdy + rdy*sdx
                let det = rdx * sdy - rdy * sdx; // Cross product of directions

                if det.abs() > 0.00001 {
                    let ox = origin_x - sx1;
                    let oy = origin_y - sy1;

                    // u = (ox * rdy - oy * rdx) / det
                    // t = (ox * sdy - oy * sdx) / det

                    // Note: signs might be flipped depending on derivation.
                    // Let's use standard line intersection:
                    // P + t R = Q + u S
                    // (P-Q) x S = -t (R x S)  => t = (Q-P) x S / (R x S)
                    // (Q-P) = (sx1 - origin_x, sy1 - origin_y)

                    let qpx = sx1 - origin_x;
                    let qpy = sy1 - origin_y;

                    let r_cross_s = rdx * sdy - rdy * sdx;
                    let qp_cross_s = qpx * sdy - qpy * sdx;
                    let qp_cross_r = qpx * rdy - qpy * rdx;

                    let t = qp_cross_s / r_cross_s;
                    let u = qp_cross_r / r_cross_s;

                    if t > 0.0 && t <= min_t && u >= 0.0 && u <= 1.0 {
                        min_t = t;
                        hit_x = origin_x + t * rdx;
                        hit_y = origin_y + t * rdy;
                    }
                }
            }

            polygon.push((hit_x, hit_y));
        }

        polygon
    }
}
