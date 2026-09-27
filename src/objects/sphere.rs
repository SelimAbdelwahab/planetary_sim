use std::sync::Arc;

use glam::Mat4;

use crate::{
    MouseData, components::{Entity, World, Transform}, game_object::GameObject, renderer::Renderer, resource::{Mesh, MeshKind}
};

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct ObjUBO {
    model: [[f32; 4]; 4],
    color: [f32; 4],
}

pub struct Sphere {
    entity: Entity,
    color: [f32; 3],
    mesh: Option<Arc<Mesh>>,
    bind_group: Option<wgpu::BindGroup>,
    ubo: Option<wgpu::Buffer>,
}

impl Sphere {
    pub fn new(entity: Entity, color: [f32; 3]) -> Self {
        Self {
            entity,
            color,
            mesh: None,
            bind_group: None,
            ubo: None,
        }
    }

    fn write_model(&self, renderer: &mut Renderer, transform: Mat4, color: [f32; 4]) {
        if let Some(ubo) = &self.ubo {
            let ubo_data = ObjUBO {
                model: transform.to_cols_array_2d(),
                color,
            };
            renderer.queue.write_buffer(ubo, 0, bytemuck::bytes_of(&ubo_data));
        }
    }

    fn project_to_screen(renderer: &Renderer, world_pos: glam::Vec3) -> Option<glam::Vec2> {
        let vp = renderer.view_proj_matrix();
        let clip = vp * glam::Vec4::new(world_pos.x, world_pos.y, world_pos.z, 1.0);
        if clip.w.abs() < f32::EPSILON || clip.w <= 0.0 { return None; }
        let ndc = clip.truncate() / clip.w;
        if ndc.z < -1.0 || ndc.z > 1.0 { return None; }
        let (width, height) = renderer.surface_dimensions();
        let screen_x = (ndc.x * 0.5 + 0.5) * width as f32;
        let screen_y = (-ndc.y * 0.5 + 0.5) * height as f32;
        Some(glam::Vec2::new(screen_x, screen_y))
    }

    fn is_mouse_over(&self, renderer: &Renderer, transform: &Transform, mouse_data: MouseData) -> bool {
        let Some(center) = Self::project_to_screen(renderer, transform.position) else {
            return false;
        };
        let radius_world = transform.scale.max_element();
        if radius_world <= 0.0 { return false; }
        let offset_pos = transform.position + glam::Vec3::new(radius_world, 0.0, 0.0);
        let Some(edge) = Self::project_to_screen(renderer, offset_pos) else {
            return false;
        };
        let radius_pixels = center.distance(edge).abs().max(2.0);
        let cursor = mouse_data.pos;
        center.distance(cursor) <= radius_pixels * 1.1
    }
}

impl GameObject for Sphere {
    fn entity(&self) -> Entity { self.entity }

    fn gpu_init(&mut self, renderer: &mut Renderer, world: &World) {
        use wgpu::util::DeviceExt;

        self.mesh = Some(renderer.resources.get_mesh(&renderer.device, MeshKind::Sphere { slices: 32, stacks: 16 }));

        if let Some(transform) = world.transform(self.entity) {
            let model = Mat4::from_translation(transform.position)
                * Mat4::from_quat(transform.rotation)
                * Mat4::from_scale(transform.scale);
            let ubo_data = ObjUBO {
                model: model.to_cols_array_2d(),
                color: [self.color[0], self.color[1], self.color[2], 1.0],
            };
            let ubo = renderer.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Sphere UBO"),
                contents: bytemuck::bytes_of(&ubo_data),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
            self.ubo = Some(ubo);

            let layout = renderer.resources.get_object_bgl(&renderer.device);
            self.bind_group = Some(renderer.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Sphere BindGroup"),
                layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.ubo.as_ref().unwrap().as_entire_binding(),
                }],
            }));
        }
    }

    fn update_render<'a>(&'a mut self, renderer: &mut Renderer, world: &World, pass: &mut wgpu::RenderPass<'a>, mouse_data: MouseData) {
        if self.mesh.is_none() {
            self.gpu_init(renderer, world);
        }

        if let Some(transform) = world.transform(self.entity) {
            let model = Mat4::from_translation(transform.position)
                * Mat4::from_quat(transform.rotation)
                * Mat4::from_scale(transform.scale);

            if let Some(mesh) = &self.mesh {
                let hovered = self.is_mouse_over(renderer, transform, mouse_data);

                if hovered {
                    let scale = transform.scale.max_element() * 1.12;
                    let highlight_model = Mat4::from_translation(transform.position)
                        * Mat4::from_quat(transform.rotation)
                        * Mat4::from_scale(glam::Vec3::splat(scale));
                    self.write_model(renderer, highlight_model, [1.0, 1.0, 1.0, 1.0]);

                    pass.set_pipeline(&renderer.outline_pipeline);
                    pass.set_bind_group(0, &renderer.camera_bind_group, &[]);
                    pass.set_bind_group(1, self.bind_group.as_ref().unwrap(), &[]);
                    pass.set_vertex_buffer(0, mesh.vbuf.slice(..));
                    pass.set_index_buffer(mesh.ibuf.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..mesh.index_count, 0, 0..1);

                    pass.set_pipeline(&renderer.render_pipeline);
                    pass.set_bind_group(0, &renderer.camera_bind_group, &[]);
                }

                self.write_model(renderer, model, [self.color[0], self.color[1], self.color[2], 1.0]);
                pass.set_bind_group(1, self.bind_group.as_ref().unwrap(), &[]);
                pass.set_vertex_buffer(0, mesh.vbuf.slice(..));
                pass.set_index_buffer(mesh.ibuf.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..mesh.index_count, 0, 0..1);
            }
        }
    }
}
