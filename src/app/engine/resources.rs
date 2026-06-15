use image::{DynamicImage, ImageBuffer};
use wgpu::util::DeviceExt;

use super::{model, texture};

pub async fn load_string(file_name: &str) -> color_eyre::Result<String> {
    let path = std::path::Path::new(env!("OUT_DIR"))
        .join("res")
        .join(file_name);
    Ok(std::fs::read_to_string(path)?)
}

pub async fn load_binary(file_name: &str) -> color_eyre::Result<Vec<u8>> {
    let path = std::path::Path::new(env!("OUT_DIR"))
        .join("res")
        .join(file_name);
    Ok(std::fs::read(path)?)
}

pub async fn load_texture(
    file_name: &str,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> color_eyre::Result<texture::Texture> {
    let data = load_binary(file_name).await?;
    texture::Texture::from_bytes(device, queue, &data, file_name)
}

fn gltf_to_dynamic_image(data: &gltf::image::Data) -> color_eyre::Result<DynamicImage> {
    let (w, h) = (data.width, data.height);
    let p = data.pixels.clone();
    Ok(match data.format {
        gltf::image::Format::R8 => {
            DynamicImage::ImageLuma8(ImageBuffer::from_raw(w, h, p).unwrap())
        }
        gltf::image::Format::R8G8 => {
            DynamicImage::ImageLumaA8(ImageBuffer::from_raw(w, h, p).unwrap())
        }
        gltf::image::Format::R8G8B8 => {
            DynamicImage::ImageRgb8(ImageBuffer::from_raw(w, h, p).unwrap())
        }
        gltf::image::Format::R8G8B8A8 => {
            DynamicImage::ImageRgba8(ImageBuffer::from_raw(w, h, p).unwrap())
        }
        fmt => color_eyre::eyre::bail!("unsupported gltf image format: {:?}", fmt),
    })
}

pub async fn load_model(
    file_name: &str,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
) -> color_eyre::Result<model::Model> {
    let path = std::path::Path::new(env!("OUT_DIR"))
        .join("res")
        .join(file_name);
    let (doc, bufs, imgs) = gltf::import(path)?;

    let mut materials = Vec::new();
    for mat in doc.materials() {
        let diffuse_texture = match mat.pbr_metallic_roughness().base_color_texture() {
            Some(info) => {
                let img = gltf_to_dynamic_image(&imgs[info.texture().source().index()])?;
                texture::Texture::from_image(device, queue, &img, mat.name())?
            }
            None => {
                // 1×1 opaque white fallback
                let img = DynamicImage::ImageRgba8(ImageBuffer::from_pixel(
                    1,
                    1,
                    image::Rgba([255, 0, 255, 255]),
                ));
                texture::Texture::from_image(device, queue, &img, Some("white"))?
            }
        };

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
                },
            ],
            label: None,
        });

        materials.push(model::Material {
            name: mat.name().unwrap_or("").to_string(),
            diffuse_texture,
            bind_group,
        });
    }

    let mut meshes = Vec::new();
    for mesh in doc.meshes() {
        for primitive in mesh.primitives() {
            let reader = primitive.reader(|buf| Some(&bufs[buf.index()]));

            let positions: Vec<[f32; 3]> = reader
                .read_positions()
                .ok_or_else(|| color_eyre::eyre::anyhow!("primitive missing POSITION"))?
                .collect();

            let tex_coords: Vec<[f32; 2]> = reader
                .read_tex_coords(0)
                .map(|t| t.into_f32().collect())
                .unwrap_or_else(|| vec![[0.0, 0.0]; positions.len()]);

            let normals: Vec<[f32; 3]> = reader
                .read_normals()
                .map(|n| n.collect())
                .unwrap_or_else(|| vec![[0.0, 0.0, 0.0]; positions.len()]);

            let vertices: Vec<model::ModelVertex> = (0..positions.len())
                .map(|i| model::ModelVertex {
                    position: positions[i],
                    tex_coord: [tex_coords[i][0], 1.0 - tex_coords[i][1]], // flip V
                    normal: normals[i],
                })
                .collect();

            let indices: Vec<u32> = reader
                .read_indices()
                .ok_or_else(|| color_eyre::eyre::anyhow!("primitive missing indices"))?
                .into_u32()
                .collect();

            let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{:?} Vertex Buffer", file_name)),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{:?} Index Buffer", file_name)),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            });

            meshes.push(model::Mesh {
                name: mesh.name().unwrap_or(file_name).to_string(),
                vertex_buffer,
                index_buffer,
                num_elements: indices.len() as u32,
                material: primitive.material().index().unwrap_or(0),
            });
        }
    }

    Ok(model::Model { meshes, materials })
}
