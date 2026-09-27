use glam::Vec3;
use rayon::prelude::*;

use crate::{components::{Entity, World}, constants::SIMULATION_SPEED};

pub struct PhysicsWorld {
    gravitational_constant: f32,
    softening: f32,
    substeps: u32,
    running: bool,
    simulation_speed: f32
}

impl PhysicsWorld {
    pub fn new(gravitational_constant: f32) -> Self {
        Self {
            gravitational_constant,
            softening: 1e-3,
            substeps: 1,
            running: false,
            simulation_speed: SIMULATION_SPEED
        }
    }

    pub fn increase_sim_speed(&mut self)
    {
        self.simulation_speed *= 10.0;
        self.simulation_speed = self.simulation_speed.min(1e12);
    }

    pub fn decrease_sim_speed(&mut self)
    {
        self.simulation_speed *= 0.1;
        self.simulation_speed = self.simulation_speed.max(1.0);
    }

    pub fn set_running(&mut self, running: bool)
    {
        self.running = running;
    }

    pub fn get_running(&mut self) -> bool
    {
        self.running
    }

    pub fn set_substeps(&mut self, substeps: u32) {
        self.substeps = substeps.max(1);
    }

    pub fn step(&self, world: &mut World, entities: &[Entity], dt: f32) {
        if !self.running { return; }
        if entities.is_empty() { return; }

        let steps = self.substeps.max(1);
        let h = dt * self.simulation_speed / steps as f32;
        for _ in 0..steps {
            self.integrate(world, entities, h);
        }
    }

    fn integrate(&self, world: &mut World, entities: &[Entity], dt: f32) {
        struct Snapshot {
            id: u32,
            position: Vec3,
            velocity: Vec3,
            mass: f32,
        }

        let snapshots: Vec<Snapshot> = entities.iter()
            .filter_map(|&entity| {
                let transform = world.transform(entity)?;
                let physics = world.physics(entity)?;
                Some(Snapshot {
                    id: entity,
                    position: transform.position,
                    velocity: physics.velocity,
                    mass: physics.mass,
                })
            })
            .collect();

        let accelerations: Vec<Vec3> = snapshots.par_iter()
            .map(|snap| {
                let mut acc = Vec3::ZERO;
                for body in snapshots.iter() {
                    if body.id == snap.id { continue; }
                    let diff = body.position - snap.position;
                    let dist_sq = diff.length_squared() + self.softening;
                    let inv_dist = dist_sq.sqrt().recip();
                    let dir = diff * inv_dist;
                    acc += dir * (self.gravitational_constant * body.mass / dist_sq);
                }
                acc
            })
            .collect();

        let updates: Vec<(u32, Vec3, Vec3)> = snapshots.par_iter()
            .zip(accelerations.par_iter())
            .map(|(snap, acc)| {
                let new_velocity = snap.velocity + *acc * dt;
                let new_position = snap.position + new_velocity * dt;
                (snap.id, new_position, new_velocity)
            })
            .collect();

        for (id, new_position, new_velocity) in updates {
            if let Some(physics) = world.physics_mut(id) {
                physics.velocity = new_velocity;
            }
            if let Some(transform) = world.transform_mut(id) {
                transform.position = new_position;
            }
        }
    }
}
