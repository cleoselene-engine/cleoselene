use crate::spatial_db::{EntityKind, SpatialDb};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub struct RigidBody {
    pub vx: f32,
    pub vy: f32,
    pub mass: f32, // 0.0 = static (infinite mass)
    pub inv_mass: f32,
    pub restitution: f32, // 0.0 to 1.0 (bounciness)
    pub drag: f32,        // Air resistance
    pub is_static: bool,
}

impl RigidBody {
    pub fn new(mass: f32, restitution: f32, drag: f32) -> Self {
        let is_static = mass <= 0.0;
        let inv_mass = if is_static { 0.0 } else { 1.0 / mass };
        Self {
            vx: 0.0,
            vy: 0.0,
            mass,
            inv_mass,
            restitution,
            drag,
            is_static,
        }
    }
}

pub struct PhysicsWorld {
    db: Arc<Mutex<SpatialDb>>,
    bodies: HashMap<u64, RigidBody>,
    gravity_x: f32,
    gravity_y: f32,
    collisions: HashSet<(u64, u64)>, // Unique pairs per step
}

impl PhysicsWorld {
    pub fn new(db: Arc<Mutex<SpatialDb>>) -> Self {
        Self {
            db,
            bodies: HashMap::new(),
            gravity_x: 0.0,
            gravity_y: 0.0,
            collisions: HashSet::new(),
        }
    }

    pub fn get_collision_events(&mut self) -> Vec<(u64, u64)> {
        self.collisions.drain().collect()
    }

    pub fn set_gravity(&mut self, x: f32, y: f32) {
        self.gravity_x = x;
        self.gravity_y = y;
    }

    pub fn add_body(&mut self, id: u64, mass: f32, restitution: f32, drag: f32) {
        self.bodies
            .insert(id, RigidBody::new(mass, restitution, drag));
    }

    pub fn remove_body(&mut self, id: u64) {
        self.bodies.remove(&id);
    }

    pub fn set_velocity(&mut self, id: u64, vx: f32, vy: f32) {
        if let Some(body) = self.bodies.get_mut(&id) {
            body.vx = vx;
            body.vy = vy;
        }
    }

    pub fn get_velocity(&self, id: u64) -> Option<(f32, f32)> {
        self.bodies.get(&id).map(|b| (b.vx, b.vy))
    }

    pub fn step(&mut self, dt: f32) {
        // 1. Integration (Move Bodies)
        let mut updates = Vec::new();

        // Lock DB once to read positions for all bodies
        let mut db = self.db.lock().unwrap();

        for (id, body) in self.bodies.iter_mut() {
            if body.is_static {
                continue;
            }

            // Apply Gravity
            body.vx += self.gravity_x * dt;
            body.vy += self.gravity_y * dt;

            // Apply Drag
            if body.drag > 0.0 {
                body.vx *= 1.0 - body.drag * dt;
                body.vy *= 1.0 - body.drag * dt;
            }

            // Get current pos from DB
            if let Some(pos) = db.get_position(*id) {
                let new_x = pos.0 + body.vx * dt;
                let new_y = pos.1 + body.vy * dt;

                updates.push((*id, new_x, new_y));
            }
        }

        // Apply Position Updates to DB
        for (id, x, y) in updates {
            db.update_position(id, x, y);
        }

        // 2. Collision Detection & Resolution

        let dynamic_ids: Vec<u64> = self
            .bodies
            .keys()
            .cloned()
            .filter(|id| !self.bodies[id].is_static)
            .collect();

        for id_a in dynamic_ids {
            let (pos_a, radius_a) = match db.get_entity_info(id_a) {
                Some((x, y, EntityKind::Circle { radius })) => ((x, y), radius),
                _ => continue,
            };

            let body_a = self.bodies[&id_a].clone();

            // Query potential colliders
            let nearby = db.query_range(pos_a.0, pos_a.1, radius_a + 50.0, None);

            for id_b in nearby {
                if id_a == id_b {
                    continue;
                }

                // Avoid double processing for same pair in this loop iteration?
                // Collision resolution should ideally be symmetric.
                // We store the pair.

                let info_b = db.get_entity_info(id_b);
                if info_b.is_none() {
                    continue;
                }
                let (x_b, y_b, kind_b) = info_b.unwrap();

                // Check collision
                let collision = match kind_b {
                    EntityKind::Circle { radius: radius_b } => {
                        let dx = x_b - pos_a.0;
                        let dy = y_b - pos_a.1;
                        let dist_sq = dx * dx + dy * dy;
                        let r_sum = radius_a + radius_b;

                        if dist_sq < r_sum * r_sum && dist_sq > 0.0001 {
                            let dist = dist_sq.sqrt();
                            let normal_x = dx / dist;
                            let normal_y = dy / dist;
                            let penetration = r_sum - dist;
                            Some((normal_x, normal_y, penetration))
                        } else {
                            None
                        }
                    }
                    EntityKind::Segment { x2, y2 } => {
                        let dx_seg = x2 - x_b;
                        let dy_seg = y2 - y_b;
                        let len_sq = dx_seg * dx_seg + dy_seg * dy_seg;
                        let mut t: f32 = 0.0;
                        if len_sq > 0.0 {
                            t = ((pos_a.0 - x_b) * dx_seg + (pos_a.1 - y_b) * dy_seg) / len_sq;
                            t = t.max(0.0).min(1.0);
                        }
                        let closest_x = x_b + t * dx_seg;
                        let closest_y = y_b + t * dy_seg;

                        let dx = pos_a.0 - closest_x;
                        let dy = pos_a.1 - closest_y;
                        let dist_sq = dx * dx + dy * dy;

                        if dist_sq < radius_a * radius_a {
                            let dist = dist_sq.sqrt();
                            let (nx, ny) = if dist > 0.0001 {
                                (dx / dist, dy / dist)
                            } else {
                                (0.0, 1.0)
                            };
                            let penetration = radius_a - dist;
                            Some((-nx, -ny, penetration))
                        } else {
                            None
                        }
                    }
                };

                if let Some((nx, ny, penetration)) = collision {
                    // Store Collision Event
                    // Normalize order to avoid duplicates (A,B) and (B,A)
                    let pair = if id_a < id_b {
                        (id_a, id_b)
                    } else {
                        (id_b, id_a)
                    };
                    self.collisions.insert(pair);

                    // RESOLVE
                    let body_b_opt = self.bodies.get(&id_b).cloned();

                    let (inv_mass_b, vel_bx, vel_by, restitution_b) = if let Some(bb) = body_b_opt {
                        (bb.inv_mass, bb.vx, bb.vy, bb.restitution)
                    } else {
                        (0.0, 0.0, 0.0, 1.0) // Infinite mass wall
                    };

                    let total_inv_mass = body_a.inv_mass + inv_mass_b;
                    if total_inv_mass <= 0.0 {
                        continue;
                    }

                    let percent = 0.8;
                    let slop = 0.01;
                    let correction_mag =
                        (penetration - slop).max(0.0f32) / total_inv_mass * percent;
                    let cx = nx * correction_mag;
                    let cy = ny * correction_mag;

                    let new_ax = pos_a.0 - cx * body_a.inv_mass;
                    let new_ay = pos_a.1 - cy * body_a.inv_mass;

                    db.update_position(id_a, new_ax, new_ay);
                    if inv_mass_b > 0.0 {
                        if let Some(pos_b) = db.get_position(id_b) {
                            db.update_position(
                                id_b,
                                pos_b.0 + cx * inv_mass_b,
                                pos_b.1 + cy * inv_mass_b,
                            );
                        }
                    }

                    // Velocity Impulse
                    let ba = self.bodies.get_mut(&id_a).unwrap();
                    let vax = ba.vx;
                    let vay = ba.vy;

                    let rvx = vel_bx - vax;
                    let rvy = vel_by - vay;

                    let vel_along_normal = rvx * nx + rvy * ny;

                    if vel_along_normal > 0.0 {
                        continue;
                    }

                    let e = body_a.restitution.min(restitution_b);
                    let j = -(1.0 + e) * vel_along_normal;
                    let j = j / total_inv_mass;

                    let impulse_x = j * nx;
                    let impulse_y = j * ny;

                    self.bodies.get_mut(&id_a).unwrap().vx -= impulse_x * body_a.inv_mass;
                    self.bodies.get_mut(&id_a).unwrap().vy -= impulse_y * body_a.inv_mass;

                    if inv_mass_b > 0.0 {
                        if let Some(bb) = self.bodies.get_mut(&id_b) {
                            bb.vx += impulse_x * inv_mass_b;
                            bb.vy += impulse_y * inv_mass_b;
                        }
                    }
                }
            }
        }
    }
}
