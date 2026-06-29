use super::{model, texture};
use nalgebra as na;
use std::path::PathBuf;
use wgpu::util::DeviceExt;

pub async fn load_path(file_name: &str) -> PathBuf {
    std::path::Path::new(env!("OUT_DIR"))
        .join("res")
        .join(file_name)
}

#[allow(unused)]
pub async fn load_binary(file_name: &str) -> color_eyre::Result<Vec<u8>> {
    let path = std::path::Path::new(env!("OUT_DIR"))
        .join("res")
        .join(file_name);
    Ok(std::fs::read(path)?)
}

#[allow(unused)]
pub async fn load_texture(
    file_name: &str,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> color_eyre::Result<texture::Texture> {
    let data = load_binary(file_name).await?;
    texture::Texture::from_bytes(device, queue, &data, file_name)
}

pub async fn load_model(
    file_name: &str,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
) -> color_eyre::Result<model::Model> {
    let (doc, bufs, imgs) = gltf::import(load_path(file_name).await)?;

    let mut materials = Vec::new();
    for mat in doc.materials() {
        let diffuse_texture = match mat.pbr_metallic_roughness().base_color_texture() {
            Some(info) => texture::Texture::from_gltf_data(
                device,
                queue,
                &imgs[info.texture().source().index()],
                mat.name(),
            )?,
            None => {
                let img = image::DynamicImage::ImageRgba8(image::ImageBuffer::from_pixel(
                    1,
                    1,
                    image::Rgba([255, 0, 255, 255]),
                ));
                texture::Texture::from_image(device, queue, &img, Some("MISSING_TEXTURE"))?
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

        super::mipmapper::generate_mipmaps(device, queue, &diffuse_texture);

        materials.push(model::Material {
            name: mat.name().unwrap_or("UNNAMED_MATERIAL").to_string(),
            diffuse_texture,
            bind_group,
        });
    }

    let meshes = doc
        .meshes()
        .map(|mesh| {
            let primitives = mesh
                .primitives()
                .map(|prim| {
                    let reader = prim.reader(|buf| Some(&bufs[buf.index()]));

                    let positions: Vec<[f32; 3]> = reader
                        .read_positions()
                        .ok_or_else(|| color_eyre::eyre::anyhow!("failed to read position(s)"))
                        .unwrap()
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
                            position: na::Point3::from(positions[i]),
                            tex_coord: na::Point2::from([tex_coords[i][0], 1.0 - tex_coords[i][1]]),
                            normal: na::Point3::from(normals[i]),
                        })
                        .collect();

                    let indices: Vec<u32> = reader
                        .read_indices()
                        .ok_or_else(|| color_eyre::eyre::anyhow!("primitive missing indices"))
                        .unwrap()
                        .into_u32()
                        .collect();

                    let vertex_buffer =
                        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some(&format!("{:?} Vertex Buffer", file_name)),
                            contents: bytemuck::cast_slice(&vertices),
                            usage: wgpu::BufferUsages::VERTEX,
                        });
                    let index_buffer =
                        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some(&format!("{:?} Index Buffer", file_name)),
                            contents: bytemuck::cast_slice(&indices),
                            usage: wgpu::BufferUsages::INDEX,
                        });

                    model::Primitive {
                        vertex_buffer,
                        index_buffer,
                        num_elements: indices.len() as u32,
                        material_id: prim.material().index().unwrap_or(0),
                    }
                })
                .collect::<Vec<_>>();

            model::Mesh {
                name: mesh.name().unwrap_or(file_name).to_string(),
                primitives,
            }
        })
        .collect::<Vec<_>>();

    Ok(model::Model { meshes, materials })
}
