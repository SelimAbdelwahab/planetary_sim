use crate::{MouseData, components::{Entity, World}, renderer::Renderer};

pub trait GameObject {
    fn entity(&self) -> Entity;
    fn gpu_init(&mut self, renderer: &mut Renderer, world: &World);
    fn update_render<'a>(&'a mut self, renderer: &mut Renderer, world: &World, pass: &mut wgpu::RenderPass<'a>, mouse_data: MouseData);
}
