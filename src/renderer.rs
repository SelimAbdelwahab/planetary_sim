use std::f32::consts::PI;

use winit::{dpi::PhysicalSize, window::Window};
use glam::{Mat4, Quat, Vec3};
use wgpu::util::DeviceExt;
use crate::resource::ResourceManager;
use crate::renderable::Vertex;
use crate::constants::{CAMERA_NEAR_PLANE, CAMERA_FAR_PLANE};

const CAMERA_POINT_DURATION: f32 = 0.1;
const AXIS_LENGTH_SCALE: f32 = 0.15;
const AXIS_PADDING: [f32; 2] = [-0.8, -0.8];

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct AxisVertex {
    position: [f32; 3],
    color: [f32; 3],
}

impl AxisVertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<AxisVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 12,
                    shader_location: 1,
                },
            ],
        }
    }
}
const CAMERA_MOVE_DURATION: f32 = 0.5;

pub struct Renderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub render_pipeline: wgpu::RenderPipeline,
    pub outline_pipeline: wgpu::RenderPipeline,
    pub resources: ResourceManager,
    pub camera_buffer: wgpu::Buffer,
    pub camera_bind_group: wgpu::BindGroup,
    camera_controller: CameraController,
    axis_pipeline: wgpu::RenderPipeline,
    axis_vertex_buffer: wgpu::Buffer,
    depth_texture: wgpu::Texture,
    pub depth_view: wgpu::TextureView,
    depth_format: wgpu::TextureFormat,
}

impl Renderer {
    /// Create renderer and return the Surface separately to avoid lifetimes on the struct.
    pub async fn new(window: &Window, camera_pos: glam::Vec3) -> (Self, wgpu::Surface<'static>) {
        let size = window.inner_size();

        // Instance, Surface, Adapter, Device, Queue
        let instance = wgpu::Instance::default();
        // Create a surface with a 'static lifetime by asserting the window outlives the surface
        let surface = unsafe {
            #[allow(unused_unsafe)]
            instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::from_window(window).expect("surface target"))
        }.expect("Failed to create surface");
        let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }).await.expect("No suitable GPU adapters found");

        let (device, queue) = adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("Device"),
                required_features: wgpu::Features::empty(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
                trace: wgpu::Trace::Off,
            }
        ).await.expect("Failed to create device");

        // Surface configuration
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps.formats.iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        // Shared resources & common bind group layouts
        let mut resources = ResourceManager::new();
        let cam_bgl_local = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Camera BGL"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let obj_bgl = resources.get_object_bgl(&device);

        // Camera uniform (view-projection)
        #[repr(C)]
        #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
        struct CameraUBO { view_proj: [[f32; 4]; 4] }

        let camera_controller = CameraController::new(camera_pos, 60f32.to_radians());
        let vp = camera_controller.view_proj(config.width, config.height);
        let cam_ubo = CameraUBO { view_proj: vp.to_cols_array_2d() };
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera UBO"),
            contents: bytemuck::bytes_of(&cam_ubo),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera BindGroup"),
            layout: &cam_bgl_local,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: camera_buffer.as_entire_binding() }],
        });

        // Depth buffer
        let depth_format = wgpu::TextureFormat::Depth32Float;
        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size: wgpu::Extent3d { width: config.width, height: config.height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: depth_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Shader and pipeline
        let shader = device.create_shader_module(wgpu::include_wgsl!("shaders/basic.wgsl"));
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[&cam_bgl_local, obj_bgl],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        let outline_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Outline Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Front),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        let axis_shader = device.create_shader_module(wgpu::include_wgsl!("shaders/axis.wgsl"));
        let axis_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Axis Vertex Buffer"),
            size: (std::mem::size_of::<AxisVertex>() * 6) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let axis_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Axis Pipeline Layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        let axis_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Axis Pipeline"),
            layout: Some(&axis_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &axis_shader,
                entry_point: Some("vs_main"),
                buffers: &[AxisVertex::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &axis_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let mut renderer = Self {
            device,
            queue,
            config,
            render_pipeline,
            outline_pipeline,
            resources,
            camera_buffer,
            camera_bind_group,
            camera_controller,
            axis_pipeline,
            axis_vertex_buffer,
            depth_texture,
            depth_view,
            depth_format,
        };
        renderer.update_axis_gizmo_vertices();
        (renderer, surface)
    }

    pub fn begin_frame(&self, surface: &wgpu::Surface<'_>) -> (wgpu::SurfaceTexture, wgpu::TextureView, wgpu::CommandEncoder) {
        let frame = surface.get_current_texture().expect("Failed to acquire next surface texture");
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });
        (frame, view, encoder)
    }

    pub fn end_frame(&self, frame: wgpu::SurfaceTexture, encoder: wgpu::CommandEncoder) {
        self.queue.submit(Some(encoder.finish()));
        frame.present();
    }

    pub fn resize(&mut self, surface: &wgpu::Surface<'_>, new_size: PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            surface.configure(&self.device, &self.config);

            self.depth_texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Depth Texture"),
                size: wgpu::Extent3d { width: self.config.width, height: self.config.height, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.depth_format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            self.depth_view = self.depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

            self.update_camera_buffer();
        }
    }

    pub fn dolly_camera(&mut self, distance: f32) {
        self.camera_controller.dolly(distance);
        self.update_camera_buffer();
    }

    pub fn strafe_camera(&mut self, distance: f32) {
        self.camera_controller.strafe(distance);
        self.update_camera_buffer();
    }

    pub fn rotate_camera(&mut self, yaw_delta: f32, pitch_delta: f32) {
        self.camera_controller.rotate(yaw_delta, pitch_delta);
        self.update_camera_buffer();
    }

    pub fn move_camera_to(&mut self, position: Vec3)
    {
        self.camera_controller.move_to(position, CAMERA_MOVE_DURATION);
        self.update_camera_buffer();
    }

    pub fn point_camera_to(&mut self, position: Vec3)
    {
        self.camera_controller.point_to(position, CAMERA_POINT_DURATION);
        self.update_camera_buffer();
    }

    pub fn update_camera(&mut self, dt: f32) {
        if self.camera_controller.update(dt) {
            self.update_camera_buffer();
        }
    }

    pub fn view_proj_matrix(&self) -> Mat4 {
        self.camera_controller.view_proj(self.config.width, self.config.height)
    }

    pub fn surface_dimensions(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }


    fn update_camera_buffer(&mut self) {
        #[repr(C)]
        #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
        struct CameraUBO { view_proj: [[f32; 4]; 4] }

        let vp = self.camera_controller.view_proj(self.config.width, self.config.height);
        let cam_ubo = CameraUBO { view_proj: vp.to_cols_array_2d() };
        self.queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&cam_ubo));
        self.update_axis_gizmo_vertices();
    }

    fn update_axis_gizmo_vertices(&mut self) {
        let origin = Vec3::new(AXIS_PADDING[0], AXIS_PADDING[1], 0.0);
        let scale = AXIS_LENGTH_SCALE;
        let inv = self.camera_controller.orientation().conjugate();

        let axis_dirs = [
            (Vec3::X, [1.0, 0.0, 0.0]),
            (Vec3::Y, [0.0, 1.0, 0.0]),
            (Vec3::Z, [0.0, 0.0, 1.0]),
        ];

        let mut vertices = [AxisVertex { position: [0.0; 3], color: [0.0; 3] }; 6];
        for (i, (dir, color)) in axis_dirs.iter().enumerate() {
            let cam_space = inv * *dir;
            let mut projected = Vec3::new(cam_space.x, cam_space.y, 0.0);
            if projected.length_squared() < 1e-6 {
                projected = Vec3::new(0.0, cam_space.z.signum(), 0.0);
            }
            let endpoint = origin + projected.normalize() * scale;
            vertices[i * 2] = AxisVertex { position: origin.to_array(), color: *color };
            vertices[i * 2 + 1] = AxisVertex { position: endpoint.to_array(), color: *color };
        }
        self.queue.write_buffer(&self.axis_vertex_buffer, 0, bytemuck::cast_slice(&vertices));
    }

    pub fn render_axis_gizmo<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        pass.set_pipeline(&self.axis_pipeline);
        pass.set_vertex_buffer(0, self.axis_vertex_buffer.slice(..));
        pass.draw(0..6, 0..1);
    }
}

struct CameraAnimation<T> 
{
    start: T,
    end: T,
    elapsed: f32,
    duration: f32,
}

impl<T> CameraAnimation<T> {
    fn new(start: T, end: T, duration: f32) -> Self {
        Self { start, end, elapsed: 0.0, duration }
    }

    fn advance(&mut self, dt: f32) -> f32 {
        self.elapsed = (self.elapsed + dt).min(self.duration.max(f32::EPSILON));
        self.elapsed / self.duration.max(f32::EPSILON)
    }

    fn finished(&self) -> bool {
        (self.elapsed >= self.duration) || (self.duration <= f32::EPSILON)
    }
}

struct CameraController {
    camera: Camera,
    rotate_animation: Option<CameraAnimation<Quat>>,
    move_animation: Option<CameraAnimation<glam::Vec3>>
}

impl CameraController {
    fn new(position: Vec3, fov_y: f32) -> Self {
        Self {
            camera: Camera::new(position, fov_y),
            rotate_animation: None,
            move_animation: None
        }
    }

    fn orientation(&self) -> Quat { self.camera.orientation }

    fn view_proj(&self, width: u32, height: u32) -> Mat4 {
        self.camera.view_proj(width, height)
    }

    fn update(&mut self, dt: f32) -> bool {
        self.update_rotate_ani(dt) || self.update_move_ani(dt)
    }

    fn update_rotate_ani(&mut self, dt: f32) -> bool
    {
        if let Some(anim) = self.rotate_animation.as_mut()
        {
            let t = anim.advance(dt).clamp(0.0, 1.0);
            self.camera.orientation = anim.start.slerp(anim.end, t).normalize();
            if anim.finished() {
                self.camera.orientation = anim.end.normalize();
                self.camera.sync_angles_from_orientation();
                self.rotate_animation = None;
            }
            true
        } else {
            false
        }
    }

    fn update_move_ani(&mut self, dt: f32) -> bool
    {
        if let Some(anim) = self.move_animation.as_mut()
        {
            let t = anim.advance(dt).clamp(0.0, 1.0);
            self.camera.pos = anim.start.slerp(anim.end, t);
            if anim.finished() {
                self.camera.pos = anim.end;
                self.move_animation = None;
            }
            true
        } else {
            false
        }
    }

    fn dolly(&mut self, distance: f32) {
        let (forward, _, _) = self.camera.basis();
        let new_pos = self.camera.pos + forward * distance;
        self.camera.pos = new_pos;
    }

    fn strafe(&mut self, distance: f32) {
        let (_, right, _) = self.camera.basis();
        self.camera.pos += right * distance;
    }

    fn rotate(&mut self, yaw_delta: f32, pitch_delta: f32) {
        if yaw_delta == 0.0 && pitch_delta == 0.0 {
            return;
        }
        self.rotate_animation = None;
        if yaw_delta != 0.0 {
            self.camera.yaw -= yaw_delta;
        }
        if pitch_delta != 0.0 {
            self.camera.pitch = (self.camera.pitch + pitch_delta).clamp(-1.54, 1.54);
        }
        self.camera.rebuild_orientation();
    }

    fn move_to(&mut self, position: Vec3, duration: f32) {
        if duration <= 0.0 {
            self.move_animation = None;
            self.camera.pos = position;
        } else {
            // self.point_to(position, 0.1);
            self.move_animation = Some(CameraAnimation::new(self.camera.pos, position, duration));
        }
    }

    fn point_to(&mut self, position: Vec3, duration: f32) {
        if let Some(target_orientation) = self.orientation_towards(position) {
            if duration <= 0.0 {
                self.rotate_animation = None;
                self.camera.orientation = target_orientation;
                self.camera.sync_angles_from_orientation();
            } else {
                self.rotate_animation = Some(CameraAnimation::new(self.camera.orientation, target_orientation, duration));
            }
        }
    }

    fn orientation_towards(&self, position: Vec3) -> Option<Quat> {
        let to_target = position - self.camera.pos;
        if to_target.length_squared() == 0.0 {
            return None;
        }
        let desired = to_target.normalize();
        let current = self.camera.basis().0;
        let dot = current.dot(desired).clamp(-1.0, 1.0);
        if dot > 0.9999 {
            return None;
        }

        let rotation = if dot < -0.9999 {
            let axis = current.any_orthogonal_vector().normalize();
            Quat::from_axis_angle(axis, PI)
        } else {
            let axis = current.cross(desired);
            if axis.length_squared() == 0.0 {
                return None;
            }
            let angle = current.angle_between(desired);
            Quat::from_axis_angle(axis.normalize(), angle)
        };

        Some((rotation * self.camera.orientation).normalize())
    }
}

#[derive(Clone, Copy, Debug)]
struct Camera {
    pos: Vec3,
    yaw: f32,
    pitch: f32,
    orientation: Quat,
    fov_y: f32,
}

impl Camera {
    fn new(pos: Vec3, fov_y: f32) -> Self {
        let mut camera = Self { pos, yaw: 0.0, pitch: 0.0, orientation: Quat::IDENTITY, fov_y };
        camera.rebuild_orientation();
        camera
    }

    fn rebuild_orientation(&mut self) {
        let yaw_rot = Quat::from_rotation_y(self.yaw);
        let pitch_rot = Quat::from_rotation_x(self.pitch);
        self.orientation = (yaw_rot * pitch_rot).normalize();
    }

    fn sync_angles_from_orientation(&mut self) {
        let forward = (self.orientation * Vec3::new(0.0, 0.0, -1.0)).normalize();
        self.pitch = forward.y.asin().clamp(-1.54, 1.54);
        self.yaw = forward.x.atan2(-forward.z);
        self.rebuild_orientation();
    }

    fn basis(&self) -> (Vec3, Vec3, Vec3) {
        let forward = self.orientation * Vec3::new(0.0, 0.0, -1.0);
        let right = self.orientation * Vec3::X;
        let up = self.orientation * Vec3::Y;
        (forward.normalize(), right.normalize(), up.normalize())
    }

    fn view_proj(&self, width: u32, height: u32) -> Mat4 {
        let aspect = width as f32 / height as f32;
        let near = CAMERA_NEAR_PLANE;
        let far = CAMERA_FAR_PLANE;
        let proj = Mat4::perspective_rh(self.fov_y, aspect, near, far);
        let (forward, _right, up) = self.basis();
        let view = Mat4::look_to_rh(self.pos, forward, up);
        proj * view
    }
}
