use glam::{Quat, Vec3};

pub type Entity = u32;

#[derive(Clone, Debug)]
pub struct Transform {
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Transform {
    pub fn new(position: Vec3, rotation: Quat, scale: Vec3) -> Self {
        Self { position, rotation, scale }
    }

    pub fn identity() -> Self {
        Self { position: Vec3::ZERO, rotation: Quat::IDENTITY, scale: Vec3::ONE }
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::identity()
    }
}

#[derive(Clone, Debug)]
pub struct PhysicsBody {
    pub velocity: Vec3,
    pub mass: f32,
}

impl PhysicsBody {
    pub fn new(velocity: Vec3, mass: f32) -> Self {
        Self { velocity, mass }
    }
}

pub struct World {
    next_entity: Entity,
    transforms: Vec<Option<Transform>>,
    physics: Vec<Option<PhysicsBody>>,
}

impl World {
    pub fn new() -> Self {
        Self {
            next_entity: 0,
            transforms: Vec::new(),
            physics: Vec::new(),
        }
    }

    pub fn spawn(&mut self) -> Entity {
        let id = self.next_entity;
        self.next_entity += 1;
        id
    }

    fn ensure_capacity<T>(store: &mut Vec<Option<T>>, entity: Entity) {
        let idx = entity as usize;
        if store.len() <= idx {
            store.resize_with(idx + 1, || None);
        }
    }

    pub fn insert_transform(&mut self, entity: Entity, transform: Transform) {
        Self::ensure_capacity(&mut self.transforms, entity);
        self.transforms[entity as usize] = Some(transform);
    }

    pub fn insert_physics(&mut self, entity: Entity, body: PhysicsBody) {
        Self::ensure_capacity(&mut self.physics, entity);
        self.physics[entity as usize] = Some(body);
    }

    pub fn set_velocity(&mut self, entity: Entity, velocity: Vec3) {
        if let Some(body) = self.physics_mut(entity) {
            body.velocity = velocity;
        }
    }

    pub fn transform(&self, entity: Entity) -> Option<&Transform> {
        self.transforms.get(entity as usize).and_then(|t| t.as_ref())
    }

    pub fn transform_mut(&mut self, entity: Entity) -> Option<&mut Transform> {
        self.transforms.get_mut(entity as usize).and_then(|t| t.as_mut())
    }

    pub fn physics(&self, entity: Entity) -> Option<&PhysicsBody> {
        self.physics.get(entity as usize).and_then(|p| p.as_ref())
    }

    pub fn physics_mut(&mut self, entity: Entity) -> Option<&mut PhysicsBody> {
        self.physics.get_mut(entity as usize).and_then(|p| p.as_mut())
    }
}
