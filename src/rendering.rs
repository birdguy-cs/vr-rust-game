use std::{borrow::Cow, num::NonZero, sync::Arc};

use anyhow::Context;
use glam::{Affine3A, Mat4, Quat, Vec2, Vec3};
use openxr::sys::SessionActionSetsAttachInfo;
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, Buffer, BufferBindingType, BufferDescriptor, BufferSize, BufferUsages, Color, CommandBuffer, CommandEncoderDescriptor, Device, FragmentState, Instance, LoadOp, Operations, PipelineLayoutDescriptor, PrimitiveState, Queue, RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline, RenderPipelineDescriptor, Sampler, SamplerBindingType, ShaderModuleDescriptor, ShaderSource, ShaderStages, StoreOp, TextureFormat, TextureSampleType, TextureView, TextureViewDimension, VertexState,
};

use crate::{
    VIEW_TYPE, actions::{self, ActionSetBundle}, math, models::{Assets, Model}, session::SessionBundle, world::World,
};

#[cfg(not(target_os = "android"))]
use crate::window::Window;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct UniformData {
    matrices: [Mat4; 2],
    model: Mat4,
    uv_scale: [f32; 4],
    colour_shift: [f32; 4],
}

pub struct RenderContext {
    pub instance: Instance,
    pub device: Device,
    pub queue: Queue,

    pub uniform_layout: BindGroupLayout,
    pub render_pipeline: RenderPipeline,

    pub depth_buffer: Option<DepthBuffer>,

    // Textures
    pub material_layout: BindGroupLayout,
    pub default_material: BindGroup,
    pub sampler: Sampler,

    // Window shit
    pub resolved_texture: wgpu::Texture,
    pub resolved_view: wgpu::TextureView,

    pub window_staging_buffer: wgpu::Buffer,
    pub padded_bytes_per_row: u32,
    pub xr_render_height: u32,
    pub xr_render_width: u32,
}

impl RenderContext {
    pub fn new(xr_instance: &openxr::Instance, xr_system: openxr::SystemId) -> Self {
        let instance = indite::create_instance(xr_instance, xr_system).unwrap();
        let (device, queue) = indite::create_device(xr_instance, xr_system, &instance).unwrap();

        // Create WPGU render pipeline
        let uniform_layout = create_uniform_layout(&device);
        let material_layout = create_material_layout(&device);
        let render_pipeline = create_render_pipeline(&device, &uniform_layout, &material_layout);

        let depth_buffer = None;

        // Set up textures
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        
        let white_view = create_texture_rgba8(&device, &queue, 1, 1, &[255, 255, 255, 255]);
        let default_material = create_material_bind_group(&device, &material_layout, &white_view, &sampler);

        // Set up rendering to the window
        let views = xr_instance
            .enumerate_view_configuration_views(
                xr_system,
                openxr::ViewConfigurationType::PRIMARY_STEREO,
            )
            .unwrap();

        let width = views[0].recommended_image_rect_width;
        let height = views[0].recommended_image_rect_height;

        let bytes_per_pixel = 4; // Assuming Rgba8Unorm or Bgra8Unorm
        let unpadded_bytes_per_row = width * bytes_per_pixel;

        // Calculate 256-byte alignment padding
        let align = 256;
        let padding = (align - (unpadded_bytes_per_row % align)) % align;
        let padded_bytes_per_row = unpadded_bytes_per_row + padding;

        let buffer_size = (padded_bytes_per_row * height) as wgpu::BufferAddress;

        // Create the staging buffer on the VR device
        let window_staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("VR to Window Staging Buffer"),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let resolved_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("VR Resolved Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 2,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let resolved_view = resolved_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("VR Resolved View"),
            format: Some(TextureFormat::Rgba8UnormSrgb),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            mip_level_count: Some(1),
            base_array_layer: 0,
            array_layer_count: Some(2),
            usage: None,
        });

        Self {
            instance,
            device,
            queue,

            uniform_layout,
            render_pipeline,

            depth_buffer,

            material_layout,
            default_material,
            sampler,

            resolved_texture,
            resolved_view,

            window_staging_buffer,
            padded_bytes_per_row,
            xr_render_height: height,
            xr_render_width: width,
        }
    }

    pub fn load_material(&self, path: &str) -> Arc<wgpu::BindGroup> {
        let asset = Assets::get(path).with_context(|| format!("Embedded texture not found: {path}")).unwrap();

        let img = image::load_from_memory(&asset.data).expect("Failed to open texture").to_rgba8();
        let (w, h) = img.dimensions();

        let view = create_texture_rgba8(&self.device, &self.queue, w, h, &img);
        Arc::new(create_material_bind_group(
            &self.device,
            &self.material_layout,
            &view,
            &self.sampler,
        ))
    }
}

#[derive(Clone)]
pub struct DepthBuffer {
    pub _depth_texture: wgpu::Texture,
    pub depth_view: TextureView,
}

pub fn create_depth_buffer(device: &Device, width: u32, height: u32,) -> DepthBuffer {
    let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("VR Depth Texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 2,
        },
        mip_level_count: 1,
        sample_count: 4,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });

    let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("VR Depth View"),
        format: Some(wgpu::TextureFormat::Depth32Float),
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        aspect: wgpu::TextureAspect::DepthOnly,
        base_mip_level: 0,
        mip_level_count: Some(1),
        base_array_layer: 0,
        array_layer_count: Some(2),
        usage: None,
    });

    DepthBuffer { _depth_texture: depth_texture, depth_view }
}

fn create_uniform_layout(device: &Device) -> BindGroupLayout {
    let size = std::mem::size_of::<UniformData>();
    let transforms = BindGroupLayoutEntry {
        binding: 0,
        visibility: ShaderStages::VERTEX,
        ty: BindingType::Buffer {
            ty: BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: BufferSize::new(size as u64),
        },
        count: None,
    };
    let desc = BindGroupLayoutDescriptor {
        label: Some("uniform-layout"),
        entries: &[transforms],
    };
    device.create_bind_group_layout(&desc)
}

fn create_uniform_bind_group(device: &Device, layout: &BindGroupLayout) -> (Buffer, BindGroup) {
    let desc = BufferDescriptor {
        label: Some("uniform-buffer"),
        size: std::mem::size_of::<UniformData>() as u64,
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        mapped_at_creation: true,
    };
    let buffer = device.create_buffer(&desc);

    let entry = BindGroupEntry {
        binding: 0,
        resource: buffer.as_entire_binding(),
    };

    let desc = BindGroupDescriptor {
        label: Some("uniform-bind-group"),
        layout,
        entries: &[entry],
    };
    let bind_group = device.create_bind_group(&desc);

    (buffer, bind_group)
}

pub fn create_texture_rgba8(
    device: &Device, queue: &Queue, width: u32, height: u32, pixels: &[u8],
) -> TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("texture"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        pixels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * width),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
    );
    texture.create_view(&Default::default())
}

pub fn create_material_bind_group(
    device: &Device, layout: &BindGroupLayout, view: &TextureView, sampler: &wgpu::Sampler,
) -> BindGroup {
    device.create_bind_group(&BindGroupDescriptor {
        label: Some("material-bind-group"),
        layout,
        entries: &[
            BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(view) },
            BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
        ],
    })
}

pub fn create_material_layout(device: &Device) -> BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("material-layout"),
        entries: &[
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: true },
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Sampler(SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

fn create_render_pipeline(
    wgpu_device: &Device,
    uniform_layout: &BindGroupLayout,
    material_layout: &BindGroupLayout,
) -> RenderPipeline {
    let shader = wgpu_device.create_shader_module(ShaderModuleDescriptor {
        label: None,
        source: ShaderSource::Wgsl(Cow::Borrowed(include_str!("../assets/world.wgsl"))),
    });

    let pipeline_layout = wgpu_device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[uniform_layout, material_layout],
        immediate_size: 0,
    });

    wgpu_device.create_render_pipeline(&RenderPipelineDescriptor {
        label: None,
        layout: Some(&pipeline_layout),
        vertex: VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[crate::models::Vertex::desc()],
            compilation_options: Default::default(),
        },
        fragment: Some(FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(TextureFormat::Rgba8UnormSrgb.into())],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: 4,
            ..Default::default()
        },
        multiview_mask: NonZero::new(0b11),
        cache: None,
    })
}

fn create_model_uniform(
    device: &Device,
    layout: &BindGroupLayout,
) -> (Buffer, BindGroup) {
    create_uniform_bind_group(device, layout)
}

fn to_world(rig: &Affine3A, rig_rot: Quat, pos: Vec3, rot: Quat) -> (Vec3, Quat) {
    (rig.transform_point3(pos), rig_rot * rot)
}

#[allow(clippy::too_many_arguments)]
pub fn render_frame(
    environment_blend_mode: openxr::EnvironmentBlendMode,
    render_context: &RenderContext,
    session_bundle: &mut SessionBundle,
    action_set_bundle: &ActionSetBundle,
    world: &mut World,
    #[cfg(not(target_os = "android"))]
    window: &Option<Window>,
) {
    // Block until the previous frame is finished displaying, and is ready for another one.
    // Also returns a prediction of when the next frame will be displayed, for use with
    // predicting locations of controllers, viewpoints, etc.
    let xr_frame_state = session_bundle.frame_wait.wait().unwrap();

    // Must be called before any rendering is done!
    session_bundle.frame_stream.begin().unwrap();

    // Is the headset on?
    world.headset_on = xr_frame_state.should_render;
    let head = session_bundle.view_space.locate(&session_bundle.stage, xr_frame_state.predicted_display_time).unwrap();
    
    if head.location_flags.contains(
        openxr::SpaceLocationFlags::POSITION_VALID | openxr::SpaceLocationFlags::ORIENTATION_VALID,
    ) {
        let p = head.pose.position;
        let o = head.pose.orientation;
        let head_pos = glam::Vec3::new(p.x, p.y, p.z);
        let head_rot = glam::Quat::from_xyzw(o.x, o.y, o.z, o.w);

        world.player.head_rotation = head_rot;
        world.player.head_position = head_pos;
    }

    // We need to ask which swapchain image to use for rendering! Which one will we get?
    // Who knows! It's up to the runtime to decide.
    let image_index = session_bundle
        .swapchain_handle
        .lock()
        .unwrap()
        .acquire_image()
        .unwrap();

    let hand_pos = actions::get_hands(
        &session_bundle.session,
        action_set_bundle,
        &session_bundle.stage,
        &xr_frame_state,
    );

    world.hands = hand_pos;

    let models = world.get_world();

    // Get the texture
    let (source_texture, _) = &session_bundle.swapchain_textures[image_index as usize];

    // Set up transforms
    let rig = Affine3A::from_rotation_translation(world.player.rotation, world.player.position);

    let (left_pos, left_rot)   = to_world(&rig, world.player.rotation, world.hands.left_position,  world.hands.left_rotation);
    let (right_pos, right_rot) = to_world(&rig, world.player.rotation, world.hands.right_position, world.hands.right_rotation);
    let (head_pos, head_rot)   = to_world(&rig, world.player.rotation, world.player.head_position,   world.player.head_rotation);

    // Fetch the view transforms. To minimize latency, we intentionally do this *after*
    // recording commands to render the scene, i.e. at the last possible moment before
    // rendering begins in earnest on the GPU. Uniforms dependent on this data can be sent
    // to the GPU just-in-time by writing them to per-frame host-visible memory which the
    // GPU will only read once the command buffer is submitted.
    let (_, xr_views) = session_bundle
        .session
        .locate_views(
            VIEW_TYPE,
            xr_frame_state.predicted_display_time,
            &session_bundle.stage,
        )
        .unwrap();

    let world_views: Vec<openxr::View> = xr_views
        .iter()
        .map(|v| {
            let p = v.pose.position;
            let o = v.pose.orientation;
            let (wp, wr) = to_world(
                &rig,
                world.player.rotation,
                Vec3::new(p.x, p.y, p.z),
                Quat::from_xyzw(o.x, o.y, o.z, o.w),
            );
            let mut nv = *v;
            nv.pose = openxr::Posef {
                orientation: openxr::Quaternionf { x: wr.x, y: wr.y, z: wr.z, w: wr.w },
                position: openxr::Vector3f { x: wp.x, y: wp.y, z: wp.z },
            };
            nv
        })
        .collect();

    let mut model_uniforms = Vec::new();

    for model in &models {
        let (buffer, bind_group) =
            create_model_uniform(
                &render_context.device,
                &render_context.uniform_layout,
            );

        let model_matrix = Mat4::from_scale_rotation_translation(
            model.scale,
            model.rotation,
            model.position,
        );

        write_uniform_buffer(
            &buffer,
            &world_views,
            model_matrix,
            model.uv_scale,
            model.colour_shift,
        );

        model_uniforms.push((buffer, bind_group));
    }

    let mut depth_view = None;

    if let Some(depth_buffer) = render_context.depth_buffer.clone() {
        depth_view = Some(depth_buffer.depth_view);
    }

    let command_buffer = record_command_buffer(
        &render_context.device,
        &render_context.render_pipeline,
        &session_bundle.multisampled_framebuffer,
        &render_context.resolved_view,
        &depth_view.expect("Couldn't get the depth view"),
        &model_uniforms,
        models.clone(),
        &render_context.resolved_texture,
        source_texture,
        &render_context.material_layout,
        &render_context.default_material,
        &render_context.window_staging_buffer,
        render_context.padded_bytes_per_row,
        render_context.xr_render_width,
        render_context.xr_render_height,
    );

    // Wait until the image is available to render to before beginning work on the GPU. The
    // compositor could still be reading from it.
    let mut xr_swapchain = session_bundle.swapchain_handle.lock().unwrap();
    xr_swapchain.wait_image(openxr::Duration::INFINITE).unwrap();

    // Submit the previously prepared command buffer
    render_context.queue.submit(Some(command_buffer));

    #[cfg(not(target_os = "android"))]
    {
        // Give frame to the window
        // Ask the staging buffer to map its bytes asynchronously
        let buffer_slice = render_context.window_staging_buffer.slice(..);
        let (sender, receiver) = futures_intrusive::channel::shared::oneshot_channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            sender.send(result).unwrap();
        });

        // Poll xr device context to flush and execute the mapping callback
        let _ = render_context.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });

        // Block until the mapped range is completely downloaded to CPU memory
        pollster::block_on(receiver.receive()).unwrap().unwrap();

        // Extract the bytes into a clean standard Vector
        let raw_bytes = {
            let data_view = buffer_slice.get_mapped_range();
            data_view.to_vec()
        }; // data_view borrow safely drops here

        // Unmap immediately so the GPU is free to reuse it on the next frame loop
        render_context.window_staging_buffer.unmap();

        // Deliver the bytes safely down into your window thread's event loop
        #[cfg(not(target_os = "android"))]
        if let Some(w) = window {
            w.render(raw_bytes, render_context.xr_render_width, render_context.xr_render_height, render_context.padded_bytes_per_row);
        }

    }

    // Finish rest of the shit
    xr_swapchain.release_image().unwrap();
    end_frame(
        environment_blend_mode,
        &mut session_bundle.frame_stream,
        &session_bundle.swapchain_desc,
        &xr_swapchain,
        &session_bundle.stage,
        &xr_views,
        &xr_frame_state,
    );
}

// Render pass
fn record_command_buffer(
    device: &Device,
    render_pipeline: &RenderPipeline,
    multisampled_framebuffer: &TextureView,
    resolved_view: &TextureView,
    depth_view: &TextureView,
    model_uniforms: &[(Buffer, BindGroup)],
    models: Vec<&Model>,

    // Intermediate resolved texture
    resolved_texture: &wgpu::Texture,

    // OpenXR swapchain texture
    xr_texture_destination: &wgpu::Texture,

    // Materials
    material_layout: &BindGroupLayout,
    default_material: &BindGroup,

    // Desktop window
    window_staging_buffer_destination: &wgpu::Buffer,
    padded_bytes_per_row: u32,
    width: u32,
    height: u32,
) -> CommandBuffer {
    let mut encoder = device.create_command_encoder(
        &CommandEncoderDescriptor {
            label: Some("VR Frame Encoder"),
        },
    );

    // Render the MSAA framebuffer and resolve into our own texture.
    {
        let attachment = RenderPassColorAttachment {
            view: multisampled_framebuffer,
            depth_slice: None,

            // Don't resolve directly into OpenXR.
            resolve_target: Some(resolved_view),

            ops: Operations { // Sky colour
                load: LoadOp::Clear(Color::BLACK),
                store: StoreOp::Discard,
            },
        };

        let mut render_pass = encoder.begin_render_pass(
            &RenderPassDescriptor {
                label: Some("VR Render Pass"),
                color_attachments: &[Some(attachment)],

                depth_stencil_attachment: Some(
                    wgpu::RenderPassDepthStencilAttachment {
                        view: depth_view,

                        depth_ops: Some(Operations {
                            load: LoadOp::Clear(1.0),
                            store: StoreOp::Discard,
                        }),

                        stencil_ops: None,
                    },
                ),

                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: NonZero::new(0b11),
            },
        );

        render_pass.set_pipeline(render_pipeline);

        for (model_index, model) in models.iter().enumerate() {
            let (_, bind_group) = &model_uniforms[model_index];

            render_pass.set_bind_group(
                0,
                bind_group,
                &[],
            );

            for mesh in &model.meshes {
                let material = mesh.material.as_deref().unwrap_or(default_material);
                render_pass.set_bind_group(1, material, &[]);

                render_pass.set_vertex_buffer(
                    0,
                    mesh.vertex_buffer.slice(..),
                );

                render_pass.set_index_buffer(
                    mesh.index_buffer.slice(..),
                    wgpu::IndexFormat::Uint32,
                );

                render_pass.draw_indexed(
                    0..mesh.index_count,
                    0,
                    0..1,
                );
            }
        }
    }

    // Copy resolved stereo texture to OpenXR swapchain
    encoder.copy_texture_to_texture(
        wgpu::TexelCopyTextureInfo {
            texture: resolved_texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyTextureInfo {
            texture: xr_texture_destination,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 2,
        },
    );

    // Copy left eye to cpu staging buffer
    #[cfg(not(target_os = "android"))]
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: resolved_texture,
            mip_level: 0,

            // z = 0 means left eye
            origin: wgpu::Origin3d {
                x: 0,
                y: 0,
                z: 0,
            },

            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: window_staging_buffer_destination,

            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,

            // Only copy ONE layer because we're staging
            // buffer only has room for one eye.
            depth_or_array_layers: 1,
        },
    );

    encoder.finish()
}

fn write_uniform_buffer(
    buffer: &Buffer,
    xr_views: &[openxr::View],
    model: Mat4,
    uv_scale: Vec2,
    colour_shift: Vec3,
) {
    let data = UniformData {
        matrices: [
            math::matrix_from_view(&xr_views[0]),
            math::matrix_from_view(&xr_views[1]),
        ],
        model,
        uv_scale: [uv_scale.x, uv_scale.y, 0.0, 0.0],
        colour_shift: [colour_shift.x, colour_shift.y, colour_shift.z, 0.0],
    };

    let contents = bytemuck::bytes_of(&data);

    buffer
        .slice(..)
        .get_mapped_range_mut()[..contents.len()]
        .copy_from_slice(contents);

    buffer.unmap();
}

fn end_frame(
    environment_blend_mode: openxr::EnvironmentBlendMode,
    frame_stream: &mut openxr::FrameStream<openxr::Vulkan>,
    swapchain_desc: &indite::SwapchainDescriptor,
    xr_swapchain: &openxr::Swapchain<openxr::Vulkan>,
    xr_stage: &openxr::Space,
    xr_views: &[openxr::View],
    xr_frame_state: &openxr::FrameState,
) {
    // Tell OpenXR what to present for this frame
    let rect = openxr::Rect2Di {
        offset: openxr::Offset2Di { x: 0, y: 0 },
        extent: openxr::Extent2Di {
            width: swapchain_desc.width as _,
            height: swapchain_desc.height as _,
        },
    };
    let views = [
        openxr::CompositionLayerProjectionView::new()
            .pose(xr_views[0].pose)
            .fov(xr_views[0].fov)
            .sub_image(
                openxr::SwapchainSubImage::new()
                    .swapchain(xr_swapchain)
                    .image_array_index(0)
                    .image_rect(rect),
            ),
        openxr::CompositionLayerProjectionView::new()
            .pose(xr_views[1].pose)
            .fov(xr_views[1].fov)
            .sub_image(
                openxr::SwapchainSubImage::new()
                    .swapchain(xr_swapchain)
                    .image_array_index(1)
                    .image_rect(rect),
            ),
    ];
    let layer = openxr::CompositionLayerProjection::new()
        .space(xr_stage)
        .views(&views);
    frame_stream
        .end(
            xr_frame_state.predicted_display_time,
            environment_blend_mode,
            &[&layer],
        )
        .unwrap();
}
