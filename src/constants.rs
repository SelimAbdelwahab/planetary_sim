pub const DOWN_SCALE: f32 = 1e6; // meters -> scene units

pub const G: f32 = 6.67430e-11 / (DOWN_SCALE * DOWN_SCALE);

pub const CAMERA_START_POS: glam::Vec3 = glam::Vec3::new(0.0, 0.0, 150.0);
pub const CAMERA_NEAR_PLANE: f32 = 0.5;
pub const CAMERA_FAR_PLANE: f32 = 1e6;

pub const CAMERA_SPEED_SLOW: f32 = 1e2;
pub const CAMERA_SPEED: f32 = 1e3;
pub const CAMERA_SPEED_FAST: f32 = 1e5;

pub const SIMULATION_SPEED: f32 = 1e5;
pub const PHYSICS_SUBSTEPS: u32 = 300;

pub const FOCUS_DISTANCE: f32 = 10.0;
