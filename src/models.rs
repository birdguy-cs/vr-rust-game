use glam::{Quat, Vec3};
use anyhow::{Context, Result};
use wgpu::util::DeviceExt;
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "assets/"]
struct Assets;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub tex_coords: [f32; 2],
}

impl Vertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;

        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 3]>() as u64,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 6]>() as u64,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

#[derive(Clone)]
pub struct Model {
    pub meshes: Vec<Mesh>,
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

#[derive(Clone)]
pub struct Mesh {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
}

impl Model {
    pub fn load(
        device: &wgpu::Device,
        path: &str,
    ) -> Result<Self> {
        let asset = Assets::get(path)
            .with_context(|| format!("Embedded model not found: {path}"))?;
    
        let (document, buffers, _images) =
            gltf::import_slice(asset.data.as_ref())
                .context("Failed to import embedded GLB")?;

        let mut meshes = Vec::new();

        for scene in document.scenes() {
            for node in scene.nodes() {
                Self::load_node(
                    device,
                    &buffers,
                    node,
                    &mut meshes,
                )?;
            }
        }

        Ok(Self {
            meshes,
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        })
    }

    fn load_node(
        device: &wgpu::Device,
        buffers: &[gltf::buffer::Data],
        node: gltf::Node,
        meshes: &mut Vec<Mesh>,
    ) -> Result<()> {
        if let Some(gltf_mesh) = node.mesh() {
            for primitive in gltf_mesh.primitives() {
                let reader = primitive.reader(|buffer| {
                    Some(&buffers[buffer.index()])
                });

                let positions: Vec<[f32; 3]> = reader
                    .read_positions()
                    .context("Model has no positions")?
                    .collect();

                let normals: Vec<[f32; 3]> = reader
                    .read_normals()
                    .map(|n| n.collect())
                    .unwrap_or_else(|| {
                        vec![[0.0, 1.0, 0.0]; positions.len()]
                    });

                let tex_coords: Vec<[f32; 2]> = reader
                    .read_tex_coords(0)
                    .map(|uv| uv.into_f32().collect())
                    .unwrap_or_else(|| {
                        vec![[0.0, 0.0]; positions.len()]
                    });

                let vertices: Vec<Vertex> = positions
                    .iter()
                    .enumerate()
                    .map(|(i, position)| Vertex {
                        position: *position,
                        normal: normals[i],
                        tex_coords: tex_coords[i],
                    })
                    .collect();

                let indices: Vec<u32> = reader
                    .read_indices()
                    .map(|indices| indices.into_u32().collect())
                    .unwrap_or_else(|| {
                        (0..vertices.len() as u32).collect()
                    });

                let vertex_buffer =
                    device.create_buffer_init(
                        &wgpu::util::BufferInitDescriptor {
                            label: Some("Model Vertex Buffer"),
                            contents: bytemuck::cast_slice(&vertices),
                            usage: wgpu::BufferUsages::VERTEX,
                        },
                    );

                let index_buffer =
                    device.create_buffer_init(
                        &wgpu::util::BufferInitDescriptor {
                            label: Some("Model Index Buffer"),
                            contents: bytemuck::cast_slice(&indices),
                            usage: wgpu::BufferUsages::INDEX,
                        },
                    );

                meshes.push(Mesh {
                    vertex_buffer,
                    index_buffer,
                    index_count: indices.len() as u32,
                });
            }
        }

        for child in node.children() {
            Self::load_node(
                device,
                buffers,
                child,
                meshes,
            )?;
        }

        Ok(())
    }
}
