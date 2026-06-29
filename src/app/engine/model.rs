use std::ops::Range;

use super::texture;
use nalgebra as na;

pub trait Vertex {
    fn desc() -> wgpu::VertexBufferLayout<'static>;
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ModelVertex {
    pub position: na::Point3<f32>,
    pub tex_coord: na::Point2<f32>,
    pub normal: na::Point3<f32>,
}

impl Vertex for ModelVertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<ModelVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 5]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

pub struct Model {
    pub meshes: Vec<Mesh>,
    pub materials: Vec<Material>,
}

#[allow(unused)]
pub struct Material {
    pub name: String,
    pub diffuse_texture: texture::Texture,
    pub bind_group: wgpu::BindGroup,
}

#[allow(unused)]
pub struct Mesh {
    pub name: String,
    pub primitives: Vec<Primitive>,
}

pub struct Primitive {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_elements: u32,
    pub material_id: usize,
}

pub trait DrawModel<'a> {
    #[allow(unused)]
    fn draw_mesh(&mut self, mesh: &'a Mesh, material: &'a Material);
    fn draw_primitive_instanced(
        &mut self,
        mesh: &'a Primitive,
        material: &'a Material,
        instances: Range<u32>,
    );
    #[allow(unused)]
    fn draw_model(&mut self, model: &'a Model);
    fn draw_model_instanced(&mut self, model: &'a Model, instances: Range<u32>);
}
impl<'a, 'b> DrawModel<'b> for wgpu::RenderPass<'a>
where
    'b: 'a,
{
    fn draw_mesh(&mut self, mesh: &'b Mesh, material: &'b Material) {
        for prim in &mesh.primitives {
            self.draw_primitive_instanced(prim, material, 0..1);
        }
    }

    fn draw_primitive_instanced(
        &mut self,
        prim: &'b Primitive,
        material: &'b Material,
        instances: Range<u32>,
    ) {
        self.set_vertex_buffer(0, prim.vertex_buffer.slice(..));
        self.set_index_buffer(prim.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        self.set_bind_group(0, &material.bind_group, &[]);
        self.draw_indexed(0..prim.num_elements, 0, instances);
    }
    fn draw_model(&mut self, model: &'b Model) {
        self.draw_model_instanced(model, 0..1);
    }

    fn draw_model_instanced(&mut self, model: &'b Model, instances: Range<u32>) {
        for mesh in &model.meshes {
            for prim in &mesh.primitives {
                let material = &model.materials[prim.material_id];
                self.draw_primitive_instanced(prim, material, instances.clone());
            }
        }
    }
}
