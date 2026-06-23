mod camera;
mod egui_renderer;
mod model;
mod resources;
mod texture;
use std::sync::Arc;

use eframe::{egui, egui_wgpu};
use model::Vertex;
use wgpu::util::DeviceExt;
use winit::{
    event::{MouseButton, MouseScrollDelta},
    event_loop::ActiveEventLoop,
    keyboard::KeyCode,
    window::Window,
};

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    view_proj: [[f32; 4]; 4],
}

impl CameraUniform {
    fn new() -> Self {
        Self {
            view_proj: nalgebra::Matrix4::identity().into(),
        }
    }

    fn update_view_proj(&mut self, camera: &camera::Camera, projection: &camera::Projection) {
        self.view_proj = (projection.calc_matrix() * camera.calc_matrix()).into();
    }
}

struct Instance {
    position: nalgebra::Vector3<f32>,
    rotation: nalgebra::UnitQuaternion<f32>,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct InstanceRaw {
    model: [[f32; 4]; 4],
}

impl Instance {
    fn to_raw(&self) -> InstanceRaw {
        InstanceRaw {
            model: (nalgebra::Matrix4::new_translation(&self.position)
                * self.rotation.to_homogeneous())
            .into(),
        }
    }
}

impl InstanceRaw {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<InstanceRaw>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 7,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 12]>() as wgpu::BufferAddress,
                    shader_location: 8,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

const NUM_INSTANCES_PER_ROW: u32 = 200;
const NUM_PIPELINE_STATISTICS_QUERIES: u64 = 3;

pub struct Engine {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    query_set: wgpu::QuerySet,
    stats_buffer: wgpu::Buffer,
    stats_staging_buffer: wgpu::Buffer,
    stats_data: [u64; NUM_PIPELINE_STATISTICS_QUERIES as usize],
    stat_num_buf: num_format::Buffer,
    is_surface_configured: bool,
    window: Arc<Window>,
    render_pipeline: wgpu::RenderPipeline,
    camera: camera::Camera,
    projection: camera::Projection,
    pub camera_controller: camera::CameraController,
    pub mouse_pressed: bool,
    camera_uniform: CameraUniform,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    instances: Vec<Instance>,
    instance_buffer: wgpu::Buffer,
    depth_texture: texture::Texture,
    gltf_model: model::Model,
    egui_renderer: egui_renderer::EguiRenderer,
}

impl Engine {
    pub async fn new(window: Arc<Window>) -> color_eyre::Result<Self> {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            flags: Default::default(),
            memory_budget_thresholds: Default::default(),
            backend_options: Default::default(),
            display: None,
        });

        let surface = instance.create_surface(window.clone())?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::PIPELINE_STATISTICS_QUERY,
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
            })
            .await?;

        let surface_caps = surface.get_capabilities(&adapter);

        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Immediate,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        let query_set = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("Pipeline Statistics"),
            ty: wgpu::QueryType::PipelineStatistics(
                wgpu::PipelineStatisticsTypes::VERTEX_SHADER_INVOCATIONS
                    | wgpu::PipelineStatisticsTypes::CLIPPER_INVOCATIONS
                    | wgpu::PipelineStatisticsTypes::CLIPPER_PRIMITIVES_OUT,
            ),
            count: 1,
        });

        let stats_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Pipeline Statistics Readback"),
            size: NUM_PIPELINE_STATISTICS_QUERIES * 8,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let stats_staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("PSR Staging Buffer"),
            size: NUM_PIPELINE_STATISTICS_QUERIES * 8,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
                label: Some("texture_bind_group_layout"),
            });

        let egui_renderer =
            egui_renderer::EguiRenderer::new(&device, surface_format, None, 1, &window);

        let camera = camera::Camera::new(nalgebra::Point3::new(0.0, 5.0, 10.0), -90f32, -20f32);
        let projection = camera::Projection::new(config.width, config.height, 45f32, 0.1, 1000.0);
        let camera_controller = camera::CameraController::new(4.0, 2.0);

        let mut camera_uniform = CameraUniform::new();
        camera_uniform.update_view_proj(&camera, &projection);

        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
                label: Some("camera_bind_group_layout"),
            });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
            label: Some("camera_bind_group"),
        });

        let shader =
            device.create_shader_module(wgpu::include_spirv!("../../res/shaders/shader.spv"));

        let depth_texture =
            texture::Texture::create_depth_texture(&device, &config, "depth_texture");

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[
                    Some(&texture_bind_group_layout),
                    Some(&camera_bind_group_layout),
                ],
                immediate_size: 0,
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[model::ModelVertex::desc(), InstanceRaw::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
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
                format: texture::Texture::DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });

        let gltf_model = resources::load_model(
            "stoned-cube.glb",
            &device,
            &queue,
            &texture_bind_group_layout,
        )
        .await
        .unwrap();

        const SPACE_BETWEEN: f32 = 3.0;
        let instances = (0..NUM_INSTANCES_PER_ROW)
            .flat_map(|z| {
                (0..NUM_INSTANCES_PER_ROW).map(move |x| {
                    let x = SPACE_BETWEEN * (x as f32 - NUM_INSTANCES_PER_ROW as f32 / 2.0);
                    let z = SPACE_BETWEEN * (z as f32 - NUM_INSTANCES_PER_ROW as f32 / 2.0);

                    let position = nalgebra::Vector3::new(x, 0.0, z);

                    let rotation = if position.magnitude() == 0.0 {
                        nalgebra::UnitQuaternion::from_axis_angle(&nalgebra::Vector3::z_axis(), 0.0)
                    } else {
                        nalgebra::UnitQuaternion::from_axis_angle(
                            &nalgebra::Unit::new_normalize(position),
                            std::f32::consts::FRAC_PI_4,
                        )
                    };

                    Instance { position, rotation }
                })
            })
            .collect::<Vec<_>>();

        let instance_data = instances.iter().map(Instance::to_raw).collect::<Vec<_>>();
        let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Instance Buffer"),
            contents: bytemuck::cast_slice(&instance_data),
            usage: wgpu::BufferUsages::VERTEX,
        });

        Ok(Self {
            surface,
            device,
            queue,
            config,
            query_set,
            stats_buffer,
            stats_staging_buffer,
            stats_data: [0, 0, 0],
            stat_num_buf: num_format::Buffer::default(),
            is_surface_configured: false,
            window,
            render_pipeline,
            camera,
            projection,
            camera_controller,
            camera_uniform,
            mouse_pressed: false,
            camera_buffer,
            camera_bind_group,
            instances,
            instance_buffer,
            depth_texture,
            gltf_model,
            egui_renderer,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
            self.is_surface_configured = true;
            self.projection.resize(width, height);
            self.depth_texture =
                texture::Texture::create_depth_texture(&self.device, &self.config, "depth_texture");
        }
    }

    pub fn update(&mut self, dt: f32) {
        self.camera_controller.update_camera(&mut self.camera, dt);
        self.camera_uniform
            .update_view_proj(&self.camera, &self.projection);
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[self.camera_uniform]),
        );
    }

    pub fn render(&mut self, frametime: f32, ft_1pl: f32) -> color_eyre::Result<()> {
        self.window.request_redraw();

        if !self.is_surface_configured {
            return Ok(());
        }

        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [self.config.width, self.config.height],
            pixels_per_point: self.window.scale_factor() as f32,
        };

        let output = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(surface_texture) => surface_texture,
            wgpu::CurrentSurfaceTexture::Suboptimal(surface_texture) => {
                self.surface.configure(&self.device, &self.config);
                surface_texture
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                color_eyre::eyre::bail!("Lost device");
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                let error_scope = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
                let error_future = error_scope.pop();
                let error = pollster::block_on(error_future).unwrap();
                return Err(color_eyre::Report::new(error));
            }
        };

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });

            render_pass.begin_pipeline_statistics_query(&self.query_set, 0);

            render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(1, &self.camera_bind_group, &[]);

            use model::DrawModel;

            render_pass.draw_model_instanced(&self.gltf_model, 0..self.instances.len() as u32);

            render_pass.end_pipeline_statistics_query();
        }

        encoder.resolve_query_set(&self.query_set, 0..1, &self.stats_buffer, 0);
        encoder.copy_buffer_to_buffer(
            &self.stats_buffer,
            0,
            &self.stats_staging_buffer,
            0,
            NUM_PIPELINE_STATISTICS_QUERIES * 8,
        );

        {
            self.egui_renderer.begin_frame(&self.window);

            egui::Window::new("Statistics").resizable(true).show(
                self.egui_renderer.context(),
                |ui| {
                    ui.label(format!("Frame Time: {frametime:.2}ms"));
                    ui.label(format!("Frame Rate: {:.2}FPS", 1000.0 / frametime));
                    ui.label(format!("1% Lows: {:.2}FPS\n", 1.0 / ft_1pl));

                    self.stat_num_buf
                        .write_formatted(&self.stats_data[0], &num_format::Locale::en);
                    ui.label(format!(
                        "Vertex Shader Calls: {}",
                        self.stat_num_buf.as_str()
                    ));

                    self.stat_num_buf
                        .write_formatted(&self.stats_data[1], &num_format::Locale::en);
                    ui.label(format!(
                        "Triangles Sent:      {}",
                        self.stat_num_buf.as_str()
                    ));

                    self.stat_num_buf
                        .write_formatted(&self.stats_data[2], &num_format::Locale::en);
                    ui.label(format!(
                        "Triangles Rendered:  {}",
                        self.stat_num_buf.as_str()
                    ));
                },
            );

            self.egui_renderer.end_frame_and_draw(
                &self.device,
                &self.queue,
                &mut encoder,
                &self.window,
                &view,
                screen_descriptor,
            );
        }

        self.queue.submit([encoder.finish()]);
        output.present();

        let stats_slice = self.stats_staging_buffer.slice(..);
        stats_slice.map_async(wgpu::MapMode::Read, |_| {});
        self.device.poll(wgpu::PollType::wait_indefinitely())?;
        let stats_view = stats_slice.get_mapped_range();
        self.stats_data = bytemuck::cast_slice(&stats_view).try_into()?;

        drop(stats_view);
        self.stats_staging_buffer.unmap();

        self.window.request_redraw();

        Ok(())
    }

    pub fn handle_egui_input(&mut self, event: &winit::event::WindowEvent) {
        self.egui_renderer.handle_input(&self.window, event);
    }

    pub fn handle_key(&mut self, event_loop: &ActiveEventLoop, key: KeyCode, is_pressed: bool) {
        if !self.camera_controller.process_keyboard(key, is_pressed)
            && let (KeyCode::Escape, true) = (key, is_pressed)
        {
            event_loop.exit()
        }
    }

    pub fn handle_mouse_button(&mut self, button: MouseButton, pressed: bool) {
        if button == MouseButton::Right {
            self.mouse_pressed = pressed
        }
    }

    pub fn handle_mouse_scroll(&mut self, delta: &MouseScrollDelta) {
        self.camera_controller.handle_scroll(delta);
    }
}
