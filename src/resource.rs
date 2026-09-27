use std::{collections::HashMap, sync::Arc};
use crate::renderable::Vertex;
use wgpu::util::DeviceExt;

pub struct Mesh {
    pub vbuf: wgpu::Buffer,
    pub ibuf: wgpu::Buffer,
    pub index_count: u32,
}

#[derive(Eq, Hash, PartialEq, Clone)]
pub enum MeshKind {
    Sphere { slices: u32, stacks: u32 },
}

pub struct ResourceManager {
    meshes: HashMap<MeshKind, Arc<Mesh>>,
    obj_bgl: Option<wgpu::BindGroupLayout>,
}

impl ResourceManager {
    pub fn new() -> Self { Self { meshes: HashMap::new(), obj_bgl: None } }

    pub fn get_mesh(&mut self, device: &wgpu::Device, kind: MeshKind) -> Arc<Mesh> {
        self.meshes.entry(kind.clone()).or_insert_with(||
        {
            let (verts, inds) = match kind {
                MeshKind::Sphere { slices, stacks } => generate_uv_sphere(1.0, slices, stacks),
            };
            let vbuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("mesh.vbuf"),
                contents: bytemuck::cast_slice(&verts),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let ibuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("mesh.ibuf"),
                contents: bytemuck::cast_slice(&inds),
                usage: wgpu::BufferUsages::INDEX,
            });
            Arc::new(Mesh { vbuf, ibuf, index_count: inds.len() as u32 })
        }).clone()
    }

    pub fn get_object_bgl(&mut self, device: &wgpu::Device) -> &wgpu::BindGroupLayout {
        if self.obj_bgl.is_none() {
            let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Object BGL"),
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
            self.obj_bgl = Some(layout);
        }
        self.obj_bgl.as_ref().unwrap()
    }

}

fn generate_uv_sphere(radius: f32, slices: u32, stacks: u32) -> (Vec<Vertex>, Vec<u32>)
{
    let mut verts= Vec::new();
    let mut inds= Vec::new();

    for y in 0..=stacks 
    {
        let v = y as f32 / stacks as f32;
        let phi = v * std::f32::consts::PI;
        let sy = phi.cos();
        let r = phi.sin();

        for x in 0..=slices
        {
            let u = x as f32 / slices as f32;
            let theta = u * std::f32::consts::TAU;
            let sx = theta.cos();
            let cx = theta.sin();

            verts.push(Vertex {
                pos: [radius * r * sx, radius * sy, radius * r * cx],
            });
        }
    }

    let stride = slices + 1;
    for y in 0..stacks
    {
        for x in 0..slices
        {
            let i0 = y * stride + x;
            let i1 = i0 + 1;
            let i2 = i0 + stride;
            let i3 = i2 + 1;

            inds.extend_from_slice(&[
                i0, i2, i1,
                i1, i2, i3,
            ]);
        }
    }

    (verts, inds)
}
