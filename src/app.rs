use pollster::block_on;
use winit::{
    application::ApplicationHandler,
    event::{WindowEvent, ElementState, MouseButton},
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowAttributes, WindowId},
};
use winit::keyboard::{KeyCode, PhysicalKey};
use std::time::Instant;
use glam::{Quat, Vec2, Vec3};

// Bring internal modules into scope for this binary crate
mod renderer;
mod renderable;
mod resource;
mod objects;
mod components;
mod game_object;
mod constants;
mod physics;
mod config;

use crate::
{
    renderer::Renderer,
    components::{Transform, PhysicsBody, World, Entity},
    game_object::GameObject,
    objects::sphere::Sphere,
    constants::*,
    physics::PhysicsWorld,
    config::Config,
};
use anyhow::Context;
use std::collections::HashMap;

#[derive(Clone, Copy)]
struct MouseData
{
    is_down: bool,
    pos: glam::Vec2
}

impl MouseData
{
    pub fn new() -> Self
    {
        Self
        {
            is_down: false,
            pos: Vec2::ZERO
        }
    }
}

pub struct Application {
    window: Option<Window>,
    surface: Option<wgpu::Surface<'static>>,
    renderer: Option<Renderer>,
    objects: Vec<Box<dyn GameObject>>, // created after renderer exists
    physics: PhysicsWorld,
    world: World,
    mouse_data: MouseData,
    move_forward: bool,
    move_backward: bool,
    move_left: bool,
    move_right: bool,
    shift_pressed: bool,
    ctrl_pressed: bool,
    last_frame: Instant,
    paused: bool
}

impl Application {
    pub fn new() -> Self {
        let mut physics = PhysicsWorld::new(G);
        physics.set_substeps(PHYSICS_SUBSTEPS);
        Self {
            window: None,
            surface: None,
            renderer: None,
            objects: Vec::new(),
            physics,
            world: World::new(),
            mouse_data: MouseData::new(),
            move_forward: false,
            move_backward: false,
            move_left: false,
            move_right: false,
            shift_pressed: false,
            ctrl_pressed: false,
            last_frame: Instant::now(),
            paused: false
        }
    }

    fn digit_index(code: KeyCode) -> Option<usize> {
        match code {
            KeyCode::Digit0 | KeyCode::Numpad0 => Some(0),
            KeyCode::Digit1 | KeyCode::Numpad1 => Some(1),
            KeyCode::Digit2 | KeyCode::Numpad2 => Some(2),
            KeyCode::Digit3 | KeyCode::Numpad3 => Some(3),
            KeyCode::Digit4 | KeyCode::Numpad4 => Some(4),
            KeyCode::Digit5 | KeyCode::Numpad5 => Some(5),
            KeyCode::Digit6 | KeyCode::Numpad6 => Some(6),
            KeyCode::Digit7 | KeyCode::Numpad7 => Some(7),
            KeyCode::Digit8 | KeyCode::Numpad8 => Some(8),
            KeyCode::Digit9 | KeyCode::Numpad9 => Some(9),
            _ => None,
        }
    }

    fn focus_object(&mut self, index: usize) {
        let Some(renderer) = self.renderer.as_mut() else { return; };
        let Some(obj) = self.objects.get(index) else { return; };

        let entity = obj.entity();
        let Some(transform) = self.world.transform(entity) else { return; };
        
        let pos = transform.position;

        if self.shift_pressed {
            renderer.point_camera_to(pos);
        } else {
            let radius = transform.scale.max_element();
            let distance = (radius * 6.0).max(FOCUS_DISTANCE);
            let desired_pos = pos + glam::Vec3::new(0.0, 0.0, distance);
            renderer.move_camera_to(desired_pos);
            renderer.point_camera_to(pos);
        }
    }
}

impl ApplicationHandler for Application 
{
    fn resumed(&mut self, event_loop: &ActiveEventLoop) 
    {
        // Create the OS window (winit 0.30 style)
        let attrs = WindowAttributes::default().with_title("Planetary Simulator");
        let window = event_loop.create_window(attrs).expect("Failed to create window");

        // Initialize Renderer now that we have a Window
        let (mut renderer, surface) = block_on(Renderer::new(&window, CAMERA_START_POS));
        // Ensure the window outlives the surface by storing it first
        self.window = Some(window);
        self.surface = Some(surface);

        let cfg = Config::load("config/bodies.json").context("failed to load bodies config").unwrap();
        let mut objects: Vec<Box<dyn GameObject>> = Vec::new();
        let mut name_to_entity = HashMap::new();

        for body in cfg.bodies.iter() {
            let entity = self.world.spawn();
            name_to_entity.insert(body.name.clone(), entity);

            let (base_pos, base_vel) = body.relative_to.as_ref()
                .and_then(|parent| name_to_entity.get(parent).copied())
                .map(|parent_entity| {
                    let pos = self.world.transform(parent_entity).map(|t| t.position).unwrap_or(Vec3::ZERO);
                    let vel = self.world.physics(parent_entity).map(|p| p.velocity).unwrap_or(Vec3::ZERO);
                    (pos, vel)
                })
                .unwrap_or((Vec3::ZERO, Vec3::ZERO));

            let position = base_pos + Vec3::new(body.distance, 0.0, 0.0);
            let velocity = base_vel + Vec3::new(0.0, 0.0, body.velocity);

            self.world.insert_transform(entity, Transform::new(position, Quat::IDENTITY, Vec3::splat(body.radius)));
            self.world.insert_physics(entity, PhysicsBody::new(velocity, body.mass));
            objects.push(Box::new(Sphere::new(entity, body.color)));
        }

        for obj in &mut objects 
        {
            obj.gpu_init(&mut renderer, &self.world);
        }
        self.objects = objects;
        self.renderer = Some(renderer);
    }

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) 
    {
        if let Some(window) = &self.window 
        {
            if window_id != window.id() { return; }
        }

        match event {
            WindowEvent::CloseRequested => {
                _event_loop.exit();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let is_down = event.state == ElementState::Pressed;
                let is_up = event.state == ElementState::Released;

                match event.physical_key {
                    PhysicalKey::Code(KeyCode::KeyW) => self.move_forward = is_down,
                    PhysicalKey::Code(KeyCode::KeyS) => self.move_backward = is_down,
                    PhysicalKey::Code(KeyCode::KeyA) => self.move_left = is_down,
                    PhysicalKey::Code(KeyCode::KeyD) => self.move_right = is_down,
                    PhysicalKey::Code(KeyCode::ArrowRight) => self.physics.increase_sim_speed(),
                    PhysicalKey::Code(KeyCode::ArrowLeft) => self.physics.decrease_sim_speed(),
                    PhysicalKey::Code(KeyCode::ShiftLeft) => self.shift_pressed = is_down,
                    PhysicalKey::Code(KeyCode::ControlLeft) => self.ctrl_pressed = is_down,
                    PhysicalKey::Code(KeyCode::Space) => 
                    {
                        if is_up
                        {
                            let is_running = self.physics.get_running();
                            self.physics.set_running(!is_running);
                        }
                    },
                    PhysicalKey::Code(code) if Self::digit_index(code).is_some() => {
                        if let Some(index) = Self::digit_index(code) {
                            self.focus_object(index);
                        }
                    }
                    _ => {}
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if button == MouseButton::Left {
                    self.mouse_data.is_down = state == ElementState::Pressed;
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let pos = glam::Vec2::new(position.x as f32, position.y as f32);
                if self.mouse_data.is_down {
                    if let Some(renderer) = self.renderer.as_mut() {
                        let delta = pos - self.mouse_data.pos;
                        let yaw_sens = 0.005;
                        let pitch_sens = 0.005;
                        renderer.rotate_camera(delta.x * yaw_sens, -delta.y * pitch_sens);
                    }
                }
                self.mouse_data.pos = pos;
            }
            WindowEvent::Resized(size) => {
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.resize(self.surface.as_ref().unwrap(), size);
                }
            }
            WindowEvent::RedrawRequested => {
                // Smooth movement: apply held keys each frame
                let now = Instant::now();
                let delta = (now - self.last_frame).as_secs_f32().clamp(0.0, 0.1);
                
                let dt = if self.paused {
                    0.0
                } else {
                    delta
                };

                self.last_frame = now;

                // Update camera movement first
                if let Some(renderer) = self.renderer.as_mut() {
                    let speed = if self.shift_pressed && !self.ctrl_pressed {
                        CAMERA_SPEED_FAST
                    } else if self.ctrl_pressed && !self.shift_pressed {
                        CAMERA_SPEED_SLOW
                    } else {
                        CAMERA_SPEED
                    };

                    let forward_axis = (self.move_forward as i32 - self.move_backward as i32) as f32;
                    let strafe_axis = (self.move_right as i32 - self.move_left as i32) as f32;
                    if forward_axis != 0.0 { renderer.dolly_camera(forward_axis * speed * dt); }
                    if strafe_axis != 0.0 { renderer.strafe_camera(strafe_axis * speed * dt); }
                    renderer.update_camera(dt);
                }

                // Advance physics before rendering
                let entity_buffer: Vec<Entity> = self.objects.iter().map(|obj| obj.entity()).collect();
                self.physics.step(&mut self.world, &entity_buffer, dt);

                if let (Some(renderer), Some(_window)) = (self.renderer.as_mut(), self.window.as_ref()) {
                    let (frame, view, mut encoder) = renderer.begin_frame(self.surface.as_ref().unwrap());
                    {
                        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("Main Render Pass"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: &view,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }),
                                    store: wgpu::StoreOp::Store,
                                },
                                depth_slice: None,
                            })],
                            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                                view: &renderer.depth_view,
                                depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                                stencil_ops: None,
                            }),
                            timestamp_writes: None,
                            occlusion_query_set: None,
                        });

                        for obj in &mut self.objects {
                            pass.set_pipeline(&renderer.render_pipeline);
                            pass.set_bind_group(0, &renderer.camera_bind_group, &[]);
                            obj.update_render(renderer, &self.world, &mut pass, self.mouse_data);
                        }
                        renderer.render_axis_gizmo(&mut pass);
                    }
                    renderer.end_frame(frame, encoder);
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) 
    {
        // Continuous redraw
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

pub fn run() 
{
    let event_loop = EventLoop::new().expect("Failed to create EventLoop");
    let mut app = Application::new();
    event_loop.run_app(&mut app).expect("run_app failed");
}

fn main() {
    run();
}
