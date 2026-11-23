mod audio;

use std::{sync::{Arc, mpsc::Receiver}, thread::{self, JoinHandle}, time::Duration};

use wgpu::{BindGroup, Buffer, CurrentSurfaceTexture, Device, Queue, Surface, SurfaceConfiguration, SurfaceTexture};
use winit::{application::ApplicationHandler, dpi::PhysicalSize, event::{self, WindowEvent}, event_loop::{ActiveEventLoop, ControlFlow, EventLoop}, window::Window};

use crate::audio::AudioManager;


#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    size: [u32; 2],
    volume: u32,
    time: f32,
    _pad: f32,
}
struct State {
    surface: Surface<'static>,
    device: Device,
    queue: Queue,
    config: SurfaceConfiguration,
    render_pipeline: wgpu::RenderPipeline,
    uniform_buffer: Buffer,
    bind_group: BindGroup,
    uniform: Uniforms,
    audio_manager: Arc<AudioManager>,
}

impl State {
    async fn new(window: Arc<Window>, audio_manager: Arc<AudioManager>) -> State {
        let uniform = Uniforms {
            size: [window.inner_size().width, window.inner_size().height],
            volume: 0,
            time: 0.0,
            _pad: 0.0,
        };
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            #[cfg(not(target_arch = "wasm32"))]
            backends: wgpu::Backends::PRIMARY,
            #[cfg(target_arch = "wasm32")]
            backends: wgpu::Backends::GL,
            flags: Default::default(),
            memory_budget_thresholds: Default::default(),
            backend_options: Default::default(),
            display: None,
        });

        let surface = instance.create_surface(window).unwrap();
        let adapter = instance.request_adapter(
            &wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
                apply_limit_buckets: false,
            }
        ).await.expect("No suitable GPU :( MR. Altman is happy");

        let (device, queue) = adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("Main Device"),
                required_features: wgpu::Features::default(),
                required_limits: wgpu::Limits::defaults(),
                memory_hints: Default::default(),
                experimental_features: Default::default(),
                trace: wgpu::Trace::Off,
            }
        ).await.unwrap();

        let surface_caps = surface.get_capabilities(&adapter);

        let surface_format = surface_caps.formats.iter()
        .find(|f| f.is_srgb())
        .copied()
        .unwrap_or(surface_caps.formats[0]);

        let alpha_mode = if 
            surface_caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::PreMultiplied) |
            surface_caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::PostMultiplied) 
        {
            wgpu::CompositeAlphaMode::PreMultiplied
        } else {
            surface_caps.alpha_modes[0]
        };

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width:  uniform.size[0],
            height: uniform.size[1],
            color_space: Default::default(),
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: alpha_mode,
            view_formats: vec![],
        };

        surface.configure(&device, &config);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let uniform_clone = [uniform.clone()];
        let bytes: &[u8] = bytemuck::cast_slice(&uniform_clone);

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Screen Size Buffer"),
            size: bytes.len() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Screen Bind Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Screen Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Pipeline Layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu:: VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Cw,
                cull_mode: None, //Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });

        State {
            surface: surface,
            device: device,
            queue: queue,
            config: config,
            render_pipeline: render_pipeline,
            uniform_buffer: uniform_buffer,
            bind_group: bind_group,
            uniform: uniform,
            audio_manager
        }
    }

    fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.uniform.size = [new_size.width, new_size.height];
            self.config.width = new_size.width;
            self.config.height = new_size.height;

            self.reconfigure_surface();
            self.queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[self.uniform]));
        }
    }

    fn reconfigure_surface(&self) {
        self.surface.configure(&self.device, &self.config);
    }

    fn render(&mut self) {
        let output = self.surface.get_current_texture();

        match output {
            CurrentSurfaceTexture::Success(output_texture) => {
                self.render_texture(output_texture);
            },
            CurrentSurfaceTexture::Suboptimal(output_texture) => {
                self.render_texture(output_texture);
                self.reconfigure_surface();
            },
            CurrentSurfaceTexture::Lost | CurrentSurfaceTexture::Outdated => {
                self.reconfigure_surface();
            }
            _ => {}
        };
    }

    fn render_texture(&mut self, output: SurfaceTexture) {
        self.uniform.time += 0.01;
        let v = self.audio_manager.get_volumes().expect("Failed to get the volume");
        println!("{}", v[1] as u32);
        self.uniform.volume = v[1] as u32;
        self.queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[self.uniform]));
        let descriptor = wgpu::TextureViewDescriptor::default();
        let view = output.texture.create_view(&descriptor);

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor{
                label: Some("render Encoder")
            });
                            
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Clear pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 0.0
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            
            render_pass.set_bind_group(0, &self.bind_group, &[]);
            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.draw(0..6, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        self.queue.present(output);
    }
}

#[derive(Default)]
struct App {
    window: Option<Arc<Window>>,
    state: Option<State>,
    audio_manager: Option<Arc<AudioManager>>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.audio_manager = Some(Arc::new(AudioManager::new().expect("Failed to create an audio manager")));
        let window_attributes = Window::default_attributes()
            .with_title("soundKat")
            .with_transparent(true)
            .with_blur(false)
            .with_decorations(false)
            .with_inner_size(PhysicalSize { width: 200, height: 200 })
            // .with_resize_increments(PhysicalSize { width: 100, height: 100})
            // .with_fullscreen(Some(winit::window::Fullscreen::Borderless(None)))buttons
        ;
        self.window = Some(Arc::new(event_loop.create_window(window_attributes).unwrap()));
        self.state = Some(pollster::block_on(
            State::new(self.window.clone().unwrap(), self.audio_manager.clone().unwrap())
        ));
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: event::WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            },
            WindowEvent::Resized(new_size) => {
                if self.state.is_some() {
                    self.state.as_mut().unwrap().resize(new_size);
                }
            }
            WindowEvent::RedrawRequested => {
                if self.window.is_some() && self.state.is_some() {
                    let state_ref = self.state.as_mut().unwrap();
                    let window_ref = self.window.as_ref().unwrap();

                    window_ref.request_redraw();

                    state_ref.render();
                }
            },
            _ => {}
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = App::default();
    event_loop.run_app(&mut app)?;
    
    Ok(())
}