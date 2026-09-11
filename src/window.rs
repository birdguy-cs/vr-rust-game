use std::{
    borrow::Cow, sync::{Arc, atomic::{AtomicBool, Ordering}, mpsc}, thread,
};

use std::sync::mpsc::{Receiver, Sender};

use wgpu::ShaderSource;
use winit::{
    application::ApplicationHandler, event::WindowEvent, event_loop::{ActiveEventLoop, ControlFlow, EventLoop}, window::{Window as WinitWindow, WindowId},
};

#[cfg(target_os = "linux")]
use winit::platform::x11::EventLoopBuilderExtX11;
#[cfg(target_os = "windows")]
use winit::platform::windows::EventLoopBuilderExtWindows;

use crate::popup_panic;

pub struct Window {
    pub window: Arc<WinitWindow>,
    event_tx: Sender<WindowCommand>,
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    pub exit: Arc<AtomicBool>,
}

impl Window {
    pub fn render(&self, pixel_data: Vec<u8>, width: u32, height: u32, padded_bytes_per_row: u32) {
        self.event_tx
            .send(WindowCommand::PresentFrame {
                data: pixel_data,
                width,
                height,
                padded_bytes_per_row,
            })
            .expect("Window thread has exited");
    }

    pub fn close(&self) {
        self.event_tx
            .send(WindowCommand::Close)
            .ok();
    }
}

enum WindowCommand {
    PresentFrame {
        data: Vec<u8>,
        width: u32,
        height: u32,
        padded_bytes_per_row: u32,
    },
    Close,
}

struct WindowThreadState {
    _window: Arc<WinitWindow>,
    _surface: wgpu::Surface<'static>,
    _config: wgpu::SurfaceConfiguration,

    _device: wgpu::Device,
    _queue: wgpu::Queue,

    blit_pipeline: wgpu::RenderPipeline,
    blit_bind_group_layout: wgpu::BindGroupLayout,
    blit_sampler: wgpu::Sampler,
}

struct WindowInitInfo {
    window: Arc<WinitWindow>,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
}

struct WindowApp {
    state: Option<WindowThreadState>,

    event_rx: Receiver<WindowCommand>,
    ready_tx: Sender<WindowInitInfo>,

    current_texture: Option<wgpu::Texture>,
    current_texture_size: Option<(u32, u32)>,

    exit: Arc<AtomicBool>,
}

impl ApplicationHandler for WindowApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(ControlFlow::Poll);

        let window = Arc::new(
            event_loop
                .create_window(
                    WinitWindow::default_attributes()
                        .with_title("Epic cool vr game")
                        .with_inner_size(winit::dpi::LogicalSize::new(1280, 720)),
                )
                .unwrap(),
        );

        // WGPU
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());

        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = pollster::block_on(
            instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            }),
        )
        .expect("Failed to find an appropriate adapter");

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("Window Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                experimental_features: wgpu::ExperimentalFeatures::default(),
                memory_hints: wgpu::MemoryHints::default(),
                trace: wgpu::Trace::default(),
            },
        ))
        .expect("Failed to create device");

        // Surface
        let capabilities = surface.get_capabilities(&adapter);

        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(capabilities.formats[0]);

        let size = window.inner_size();

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: capabilities.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &config);

        // Fullscreen blit shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Window Blit Shader"),
            source: ShaderSource::Wgsl(Cow::Borrowed(include_str!("../assets/window.wgsl"))),
        });

        // Texture + sampler bind group
        let blit_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Window Blit Bind Group Layout"),

                entries: &[
                    // Texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,

                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float {
                                filterable: true,
                            },
                        },

                        count: None,
                    },

                    // Sampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,

                        ty: wgpu::BindingType::Sampler(
                            wgpu::SamplerBindingType::Filtering,
                        ),

                        count: None,
                    },
                ],
            });

        let pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Window Blit Pipeline Layout"),

                bind_group_layouts: &[&blit_bind_group_layout],

                immediate_size: 0,
            });

        let blit_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Window Blit Pipeline"),
                layout: Some(&pipeline_layout),

                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },

                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),

                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),

                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            });

        let blit_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Window Blit Sampler"),

            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,

            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,

            ..Default::default()
        });

        // State
        self.state = Some(WindowThreadState {
            _window: window.clone(),
            _surface: surface,
            _config: config,

            _device: device.clone(),
            _queue: queue.clone(),

            blit_pipeline,
            blit_bind_group_layout,
            blit_sampler,
        });

        let init_info = WindowInitInfo {
            window,
            device: Arc::new(device),
            queue: Arc::new(queue),
        };

        self.ready_tx.send(init_info).unwrap();
    }

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                self.exit.store(true, Ordering::Relaxed);
                _event_loop.exit();
            }

            WindowEvent::Resized(size) => {
                if let Some(state) = &mut self.state {
                    if size.width == 0 || size.height == 0 {
                        return;
                    }

                    state._config.width = size.width;
                    state._config.height = size.height;

                    state
                        ._surface
                        .configure(&state._device, &state._config);
                }
            }

            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        while let Ok(command) = self.event_rx.try_recv() {
            match command {
                WindowCommand::PresentFrame {
                    data,
                    width,
                    height,
                    padded_bytes_per_row,
                } => {
                    let Some(state) = &mut self.state else {
                        continue;
                    };

                    // Create/recreate VR texture if resolution changed
                    if self.current_texture_size != Some((width, height)) {
                        let texture = state._device.create_texture(
                            &wgpu::TextureDescriptor {
                                label: Some("Window VR Frame"),

                                size: wgpu::Extent3d {
                                    width,
                                    height,
                                    depth_or_array_layers: 1,
                                },

                                mip_level_count: 1,
                                sample_count: 1,

                                dimension: wgpu::TextureDimension::D2,

                                format: wgpu::TextureFormat::Rgba8UnormSrgb,

                                usage: wgpu::TextureUsages::COPY_DST
                                    | wgpu::TextureUsages::TEXTURE_BINDING,

                                view_formats: &[],
                            },
                        );

                        self.current_texture = Some(texture);
                        self.current_texture_size = Some((width, height));
                    }

                    let Some(current_texture) = &self.current_texture else {
                        continue;
                    };

                    // Upload cpu vr pixels to vr texture
                    state._queue.write_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: current_texture,

                            mip_level: 0,

                            origin: wgpu::Origin3d::ZERO,

                            aspect: wgpu::TextureAspect::All,
                        },

                        &data,

                        wgpu::TexelCopyBufferLayout {
                            offset: 0,

                            bytes_per_row: Some(padded_bytes_per_row),

                            rows_per_image: Some(height),
                        },

                        wgpu::Extent3d {
                            width,
                            height,
                            depth_or_array_layers: 1,
                        },
                    );

                    // Get window surface texture
                    let frame = match state._surface.get_current_texture() {
                        Ok(frame) => frame,

                        Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                            state
                                ._surface
                                .configure(&state._device, &state._config);

                            continue;
                        }

                        Err(wgpu::SurfaceError::OutOfMemory) => {
                            popup_panic!("Window surface ran out of memory");
                        }

                        Err(wgpu::SurfaceError::Timeout) => {
                            continue;
                        }

                        Err(wgpu::SurfaceError::Other) => {
                            continue;
                        }
                    };

                    let surface_view =
                        frame.texture.create_view(&Default::default());

                    let texture_view =
                        current_texture.create_view(&wgpu::TextureViewDescriptor {
                            label: Some("Window VR Frame View"),

                            format: Some(wgpu::TextureFormat::Rgba8UnormSrgb),

                            dimension: Some(wgpu::TextureViewDimension::D2),

                            aspect: wgpu::TextureAspect::All,

                            base_mip_level: 0,
                            mip_level_count: Some(1),

                            base_array_layer: 0,
                            array_layer_count: Some(1),

                            usage: Some(
                                wgpu::TextureUsages::TEXTURE_BINDING,
                            ),
                        });

                    // Bind vr texture to fullscreen shader
                    let bind_group =
                        state._device.create_bind_group(
                            &wgpu::BindGroupDescriptor {
                                label: Some("Window Blit Bind Group"),

                                layout: &state.blit_bind_group_layout,

                                entries: &[
                                    wgpu::BindGroupEntry {
                                        binding: 0,

                                        resource:
                                            wgpu::BindingResource::TextureView(
                                                &texture_view,
                                            ),
                                    },

                                    wgpu::BindGroupEntry {
                                        binding: 1,

                                        resource:
                                            wgpu::BindingResource::Sampler(
                                                &state.blit_sampler,
                                            ),
                                    },
                                ],
                            },
                        );

                    // Render vr texture to window surface
                    let mut encoder = state
                        ._device
                        .create_command_encoder(
                            &wgpu::CommandEncoderDescriptor {
                                label: Some("Window Blit Encoder"),
                            },
                        );
                    {
                        let mut render_pass =
                            encoder.begin_render_pass(
                                &wgpu::RenderPassDescriptor {
                                    label: Some("Window Blit Render Pass"),

                                    color_attachments: &[Some(
                                        wgpu::RenderPassColorAttachment {
                                            view: &surface_view,

                                            depth_slice: None,

                                            resolve_target: None,

                                            ops: wgpu::Operations {
                                                load: wgpu::LoadOp::Clear(
                                                    wgpu::Color::TRANSPARENT,
                                                ),

                                                store: wgpu::StoreOp::Store,
                                            },
                                        },
                                    )],

                                    depth_stencil_attachment: None,

                                    occlusion_query_set: None,

                                    timestamp_writes: None,

                                    multiview_mask: None,
                                },
                            );

                        render_pass.set_pipeline(
                            &state.blit_pipeline,
                        );

                        render_pass.set_bind_group(
                            0,
                            &bind_group,
                            &[],
                        );

                        // Fullscreen triangle
                        render_pass.draw(0..3, 0..1);
                    }

                    state
                        ._queue
                        .submit(Some(encoder.finish()));

                    frame.present();
                }

                WindowCommand::Close => {
                    self.exit.store(true, Ordering::Relaxed);
                    _event_loop.exit();
                }
            }
        }
    }
}

pub fn create_window() -> Window {
    let (event_tx, event_rx) = mpsc::channel();
    let (ready_tx, ready_rx) = mpsc::channel();

    let exit = Arc::new(AtomicBool::new(false));
    let exit_thread = exit.clone();

    thread::spawn(move || {
        let event_loop = EventLoop::builder()
            .with_any_thread(true)
            .build()
            .unwrap();

        let mut app = WindowApp {
            state: None,

            event_rx,
            ready_tx,

            current_texture: None,
            current_texture_size: None,

            exit: exit_thread,
        };

        event_loop.run_app(&mut app).unwrap();
    });

    let init_info = ready_rx.recv().expect("Window thread failed");

    Window {
        window: init_info.window,

        event_tx,

        device: init_info.device,
        queue: init_info.queue,

        exit,
    }
}
