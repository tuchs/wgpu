use std::ops::Range;
use std::rc::Rc;

mod framework;


#[derive(Clone)]
struct Vertex {
    pos: [i8; 4],
    normal: [i8; 4],
}

fn vertex(pos: [i8; 3], nor: [i8; 3]) -> Vertex {
    Vertex {
        pos: [pos[0], pos[1], pos[2], 1],
        normal: [nor[0], nor[1], nor[2], 0],
    }
}

fn create_cube() -> (Vec<Vertex>, Vec<u16>) {
    let vertex_data = [
        // top (0, 0, 1)
        vertex([-1, -1,  1], [0, 0, 1]),
        vertex([ 1, -1,  1], [0, 0, 1]),
        vertex([ 1,  1,  1], [0, 0, 1]),
        vertex([-1,  1,  1], [0, 0, 1]),
        // bottom (0, 0, -1)
        vertex([-1,  1, -1], [0, 0, -1]),
        vertex([ 1,  1, -1], [0, 0, -1]),
        vertex([ 1, -1, -1], [0, 0, -1]),
        vertex([-1, -1, -1], [0, 0, -1]),
        // right (1, 0, 0)
        vertex([ 1, -1, -1], [1, 0, 0]),
        vertex([ 1,  1, -1], [1, 0, 0]),
        vertex([ 1,  1,  1], [1, 0, 0]),
        vertex([ 1, -1,  1], [1, 0, 0]),
        // left (-1, 0, 0)
        vertex([-1, -1,  1], [-1, 0, 0]),
        vertex([-1,  1,  1], [-1, 0, 0]),
        vertex([-1,  1, -1], [-1, 0, 0]),
        vertex([-1, -1, -1], [-1, 0, 0]),
        // front (0, 1, 0)
        vertex([ 1,  1, -1], [0, 1, 0]),
        vertex([-1,  1, -1], [0, 1, 0]),
        vertex([-1,  1,  1], [0, 1, 0]),
        vertex([ 1,  1,  1], [0, 1, 0]),
        // back (0, -1, 0)
        vertex([ 1, -1,  1], [0, -1, 0]),
        vertex([-1, -1,  1], [0, -1, 0]),
        vertex([-1, -1, -1], [0, -1, 0]),
        vertex([ 1, -1, -1], [0, -1, 0]),
    ];

    let index_data: &[u16] = &[
         0,  1,  2,  2,  3,  0, // top
         4,  5,  6,  6,  7,  4, // bottom
         8,  9, 10, 10, 11,  8, // right
        12, 13, 14, 14, 15, 12, // left
        16, 17, 18, 18, 19, 16, // front
        20, 21, 22, 22, 23, 20, // back
    ];

    (vertex_data.to_vec(), index_data.to_vec())
}

fn create_plane(size: i8) -> (Vec<Vertex>, Vec<u16>) {
    let vertex_data = [
        vertex([ size, -size,  0], [0, 0, 1]),
        vertex([ size,  size,  0], [0, 0, 1]),
        vertex([-size, -size,  0], [0, 0, 1]),
        vertex([-size,  size,  0], [0, 0, 1]),
    ];

    let index_data: &[u16] = &[
        0, 1, 2,
        2, 1, 3
    ];

    (vertex_data.to_vec(), index_data.to_vec())
}


struct Entity {
    mx_world: cgmath::Matrix4<f32>,
    vertex_buf: Rc<wgpu::Buffer>,
    index_buf: Rc<wgpu::Buffer>,
    index_count: usize,
    bind_group: wgpu::BindGroup,
    uniform_buf: wgpu::Buffer,
}

struct Light {
    pos: cgmath::Point3<f32>,
    color: wgpu::Color,
    fov: f32,
    depth: Range<f32>,
    target_view: wgpu::TextureView,
}

#[repr(C)]
struct LightRaw {
    pos: [f32; 4],
    color: [f32; 4],
    proj: [[f32; 4]; 4],
}

impl Light {
    fn to_raw(&self) -> LightRaw {
        use cgmath::{EuclideanSpace, Deg, Point3, Vector3, Matrix4, PerspectiveFov};

        let mx_view = Matrix4::look_at(
            self.pos,
            Point3::origin(),
            Vector3::unit_z(),
        );
        let projection = PerspectiveFov {
            fovy: Deg(self.fov).into(),
            aspect: 1.0,
            near: self.depth.start,
            far: self.depth.end,
        };
        let mx_view_proj = cgmath::Matrix4::from(projection.to_perspective()) * mx_view;
        LightRaw {
            proj: *mx_view_proj.as_ref(),
            pos: [self.pos.x, self.pos.y, self.pos.z, 1.0],
            color: [self.color.r, self.color.g, self.color.b, 1.0],
        }
    }
}

#[repr(C)]
struct ForwardUniforms {
    proj: [[f32; 4]; 4],
    color: [f32; 4],
    num_lights: [u32; 4],
}

#[repr(C)]
struct ShadowUniforms {
    proj: [[f32; 4]; 4],
}

struct Pass {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    uniform_buf: wgpu::Buffer,
}

struct Example {
    entities: Vec<Entity>,
    lights: Vec<Light>,
    lights_are_dirty: bool,
    shadow_pass: Pass,
    forward_pass: Pass,
    light_uniform_buf: wgpu::Buffer,
}

impl Example {
    const MAX_LIGHTS: usize = 10;
    const SHADOW_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::D32Float;
    const SHADOW_SIZE: wgpu::Extent3d = wgpu::Extent3d {
        width: 512,
        height: 512,
        depth: 1,
    };

    fn generate_matrix(aspect_ratio: f32) -> cgmath::Matrix4<f32> {
        let mx_projection = cgmath::perspective(cgmath::Deg(45f32), aspect_ratio, 1.0, 10.0);
        let mx_view = cgmath::Matrix4::look_at(
            cgmath::Point3::new(1.5f32, -5.0, 3.0),
            cgmath::Point3::new(0f32, 0.0, 0.0),
            cgmath::Vector3::unit_z(),
        );
        mx_projection * mx_view
    }
}

impl framework::Example for Example {
    fn init(device: &mut wgpu::Device, sc_desc: &wgpu::SwapChainDescriptor) -> Self {
        use std::mem;

        // Create the vertex and index buffers
        let vertex_size = mem::size_of::<Vertex>();
        let (cube_vertex_data, cube_index_data) = create_cube();
        let cube_vertex_buf = Rc::new(device.create_buffer(&wgpu::BufferDescriptor {
            size: (cube_vertex_data.len() * vertex_size) as u32,
            usage: wgpu::BufferUsageFlags::VERTEX | wgpu::BufferUsageFlags::TRANSFER_DST,
        }));
        cube_vertex_buf.set_sub_data(0, framework::cast_slice(&cube_vertex_data));
        let cube_index_buf = Rc::new(device.create_buffer(&wgpu::BufferDescriptor {
            size: (cube_index_data.len() * 2) as u32,
            usage: wgpu::BufferUsageFlags::INDEX | wgpu::BufferUsageFlags::TRANSFER_DST,
        }));
        cube_index_buf.set_sub_data(0, framework::cast_slice(&cube_index_data));

        let (plane_vertex_data, plane_index_data) = create_plane(7);
        let plane_vertex_buf = device.create_buffer(&wgpu::BufferDescriptor {
            size: (plane_vertex_data.len() * vertex_size) as u32,
            usage: wgpu::BufferUsageFlags::VERTEX | wgpu::BufferUsageFlags::TRANSFER_DST,
        });
        plane_vertex_buf.set_sub_data(0, framework::cast_slice(&plane_vertex_data));
        let plane_index_buf = device.create_buffer(&wgpu::BufferDescriptor {
            size: (plane_index_data.len() * 2) as u32,
            usage: wgpu::BufferUsageFlags::INDEX | wgpu::BufferUsageFlags::TRANSFER_DST,
        });
        plane_index_buf.set_sub_data(0, framework::cast_slice(&plane_index_data));
        let plane_uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
            size: 64,
            usage: wgpu::BufferUsageFlags::UNIFORM | wgpu::BufferUsageFlags::TRANSFER_DST,
        });

        let local_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            bindings: &[                     
                wgpu::BindGroupLayoutBinding {
                    binding: 0,
                    visibility: wgpu::ShaderStageFlags::VERTEX,
                    ty: wgpu::BindingType::UniformBuffer,
                },
            ],
        });

        let mut entities = vec![{
            use cgmath::SquareMatrix;

            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &local_bind_group_layout,
                bindings: &[
                    wgpu::Binding {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer {
                            buffer: &plane_uniform_buf,
                            range: 0 .. 64,
                        },
                    },
                ],
            });
            Entity {
                mx_world: cgmath::Matrix4::identity(),
                vertex_buf: Rc::new(plane_vertex_buf),
                index_buf: Rc::new(plane_index_buf),
                index_count: plane_index_data.len(),
                bind_group,
                uniform_buf: plane_uniform_buf,
            }
        }];

        struct CubeDesc {
            offset: cgmath::Vector3<f32>,
            angle: f32,
            scale: f32,
        }
        let cube_descs = [
            CubeDesc {
                offset: cgmath::vec3(-2.0, -2.0, 2.0),
                angle: 10.0,
                scale: 0.7,
            },
            CubeDesc {
                offset: cgmath::vec3(2.0, -2.0, 2.0),
                angle: 50.0,
                scale: 1.3,
            },
            CubeDesc {
                offset: cgmath::vec3(-2.0, 2.0, 2.0),
                angle: 140.0,
                scale: 1.1,
            },
            CubeDesc {
                offset: cgmath::vec3(2.0, 2.0, 2.0),
                angle: 210.0,
                scale: 0.9,
            },
        ];

        for cube in &cube_descs {
            use cgmath::{Deg, Decomposed, Quaternion, Rotation3, InnerSpace};

            let transform = Decomposed {
                disp: cube.offset.clone(),
                rot: Quaternion::from_axis_angle(
                    cube.offset.normalize(),
                    Deg(cube.angle),
                ),
                scale: cube.scale,
            };
            let uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
                size: 64,
                usage: wgpu::BufferUsageFlags::UNIFORM | wgpu::BufferUsageFlags::TRANSFER_DST,
            });
            entities.push(Entity {
                mx_world: cgmath::Matrix4::from(transform),
                vertex_buf: Rc::clone(&cube_vertex_buf),
                index_buf: Rc::clone(&cube_index_buf),
                index_count: cube_index_data.len(),
                bind_group: device.create_bind_group(&wgpu::BindGroupDescriptor {
                    layout: &local_bind_group_layout,
                    bindings: &[
                        wgpu::Binding {
                            binding: 0,
                            resource: wgpu::BindingResource::Buffer {
                                buffer: &uniform_buf,
                                range: 0 .. 64,
                            },
                        },
                    ],
                }),
                uniform_buf,
            });
        }

        // Create other resources
        let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            r_address_mode: wgpu::AddressMode::ClampToEdge,
            s_address_mode: wgpu::AddressMode::ClampToEdge,
            t_address_mode: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            lod_min_clamp: -100.0,
            lod_max_clamp: 100.0,
            max_anisotropy: 0,
            compare_function: wgpu::CompareFunction::LessEqual,
            border_color: wgpu::BorderColor::TransparentBlack,
        });

        let shadow_texture = device.create_texture(&wgpu::TextureDescriptor {
            size: Self::SHADOW_SIZE,
            array_size: Self::MAX_LIGHTS as u32,
            dimension: wgpu::TextureDimension::D2,
            format: Self::SHADOW_FORMAT,
            usage: wgpu::TextureUsageFlags::OUTPUT_ATTACHMENT | wgpu::TextureUsageFlags::SAMPLED,
        });
        let shadow_view = shadow_texture.create_default_view();

        let mut shadow_target_views = (0..2)
            .map(|i| Some(shadow_texture.create_view(&wgpu::TextureViewDescriptor {
                format: Self::SHADOW_FORMAT,
                dimension: wgpu::TextureViewDimension::D2,
                aspect: wgpu::TextureAspectFlags::DEPTH,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: i as u32,
                array_count: 1,
            })))
            .collect::<Vec<_>>();
        let lights = vec![
            Light {
                pos: cgmath::Point3::new(7.0, -5.0, 10.0),
                color: wgpu::Color { r: 0.5, g: 1.0, b: 0.5, a: 1.0 },
                fov: 60.0,
                depth: 1.0 .. 20.0,
                target_view: shadow_target_views[0].take().unwrap(),
            },
            Light {
                pos: cgmath::Point3::new(-5.0, 7.0, 10.0),
                color: wgpu::Color { r: 1.0, g: 0.5, b: 0.5, a: 1.0 },
                fov: 45.0,
                depth: 1.0 .. 20.0,
                target_view: shadow_target_views[1].take().unwrap(),
            },
        ];
        let light_uniform_size = (Self::MAX_LIGHTS * mem::size_of::<LightRaw>()) as u32;
        let light_uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
            size: light_uniform_size,
            usage: wgpu::BufferUsageFlags::UNIFORM | wgpu::BufferUsageFlags::TRANSFER_DST,
        });

        let vb_desc = wgpu::VertexBufferDescriptor {
            stride: vertex_size as u32,
            step_mode: wgpu::InputStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttributeDescriptor {
                    attribute_index: 0,
                    format: wgpu::VertexFormat::IntR8G8B8A8,
                    offset: 0,
                },
                wgpu::VertexAttributeDescriptor {
                    attribute_index: 1,
                    format: wgpu::VertexFormat::IntR8G8B8A8,
                    offset: 4 * 1,
                },
            ],
        };

        let shadow_pass = {
            // Create pipeline layout
            let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                bindings: &[
                    wgpu::BindGroupLayoutBinding {
                        binding: 0, // global
                        visibility: wgpu::ShaderStageFlags::VERTEX,
                        ty: wgpu::BindingType::UniformBuffer,
                    },
                ],
            });
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                bind_group_layouts: &[
                    &bind_group_layout,
                    &local_bind_group_layout,
                ],
            });

            let uniform_size = mem::size_of::<ShadowUniforms>() as u32;
            let uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
                size: uniform_size,
                usage: wgpu::BufferUsageFlags::UNIFORM | wgpu::BufferUsageFlags::TRANSFER_DST,
            });

            // Create bind group
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &bind_group_layout,
                bindings: &[
                    wgpu::Binding {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer {
                            buffer: &uniform_buf,
                            range: 0 .. uniform_size,
                        },
                    },
                ],
            });

            // Create the render pipeline
            let vs_bytes = framework::load_glsl("shadow-base.vert", framework::ShaderStage::Vertex);
            let fs_bytes = framework::load_glsl("shadow-bake.frag", framework::ShaderStage::Fragment);
            let vs_module = device.create_shader_module(&vs_bytes);
            let fs_module = device.create_shader_module(&fs_bytes);

            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                layout: &pipeline_layout,
                vertex_stage: wgpu::PipelineStageDescriptor {
                    module: &vs_module,
                    entry_point: "main",
                },
                fragment_stage: wgpu::PipelineStageDescriptor {
                    module: &fs_module,
                    entry_point: "main",
                },
                rasterization_state: wgpu::RasterizationStateDescriptor {
                    front_face: wgpu::FrontFace::Cw,
                    cull_mode: wgpu::CullMode::Back,
                    depth_bias: 0,
                    depth_bias_slope_scale: 0.0,
                    depth_bias_clamp: wgpu::MAX_DEPTH_BIAS_CLAMP,
                },
                primitive_topology: wgpu::PrimitiveTopology::TriangleList,
                color_states: &[],
                depth_stencil_state: Some(wgpu::DepthStencilStateDescriptor {
                    format: Self::SHADOW_FORMAT,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::LessEqual,
                    stencil_front: wgpu::StencilStateFaceDescriptor::IGNORE,
                    stencil_back: wgpu::StencilStateFaceDescriptor::IGNORE,
                    stencil_read_mask: 0,
                    stencil_write_mask: 0,
                }),
                index_format: wgpu::IndexFormat::Uint16,
                vertex_buffers: &[vb_desc.clone()],
                sample_count: 1,
            });

            Pass {
                pipeline,
                bind_group,
                uniform_buf,
            }
        };

        let forward_pass = {
            // Create pipeline layout
            let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                bindings: &[
                    wgpu::BindGroupLayoutBinding {
                        binding: 0, // global
                        visibility: wgpu::ShaderStageFlags::VERTEX | wgpu::ShaderStageFlags::FRAGMENT,
                        ty: wgpu::BindingType::UniformBuffer,
                    },
                    wgpu::BindGroupLayoutBinding {
                        binding: 1, // lights
                        visibility: wgpu::ShaderStageFlags::VERTEX | wgpu::ShaderStageFlags::FRAGMENT,
                        ty: wgpu::BindingType::UniformBuffer,
                    },
                    wgpu::BindGroupLayoutBinding {
                        binding: 2,
                        visibility: wgpu::ShaderStageFlags::FRAGMENT,
                        ty: wgpu::BindingType::SampledTexture,
                    },
                    wgpu::BindGroupLayoutBinding {
                        binding: 3,
                        visibility: wgpu::ShaderStageFlags::FRAGMENT,
                        ty: wgpu::BindingType::Sampler,
                    },
                ],
            });
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                bind_group_layouts: &[
                    &bind_group_layout,
                    &local_bind_group_layout,
                ],
            });

            let uniform_size = mem::size_of::<ForwardUniforms>() as u32;
            let uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
                size: uniform_size,
                usage: wgpu::BufferUsageFlags::UNIFORM | wgpu::BufferUsageFlags::TRANSFER_DST,
            });
            let mx_total = Self::generate_matrix(sc_desc.width as f32 / sc_desc.height as f32);
            let data = ForwardUniforms {
                proj: *mx_total.as_ref(),
                color: [1.0; 4],
                num_lights: [lights.len() as u32, 0, 0, 0],
            };
            uniform_buf.set_sub_data(0, framework::cast_slice(&[data]));

            // Create bind group
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &bind_group_layout,
                bindings: &[
                    wgpu::Binding {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer {
                            buffer: &uniform_buf,
                            range: 0 .. uniform_size,
                        },
                    },
                    wgpu::Binding {
                        binding: 1,
                        resource: wgpu::BindingResource::Buffer {
                            buffer: &light_uniform_buf,
                            range: 0 .. light_uniform_size,
                        },
                    },
                    wgpu::Binding {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&shadow_view),
                    },
                    wgpu::Binding {
                        binding: 3,
                        resource: wgpu::BindingResource::Sampler(&shadow_sampler),
                    },
                ],
            });

            // Create the render pipeline
            let vs_bytes = framework::load_glsl("shadow-forward.vert", framework::ShaderStage::Vertex);
            let fs_bytes = framework::load_glsl("shadow-forward.frag", framework::ShaderStage::Fragment);
            let vs_module = device.create_shader_module(&vs_bytes);
            let fs_module = device.create_shader_module(&fs_bytes);

            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                layout: &pipeline_layout,
                vertex_stage: wgpu::PipelineStageDescriptor {
                    module: &vs_module,
                    entry_point: "main",
                },
                fragment_stage: wgpu::PipelineStageDescriptor {
                    module: &fs_module,
                    entry_point: "main",
                },
                rasterization_state: wgpu::RasterizationStateDescriptor {
                    front_face: wgpu::FrontFace::Cw,
                    cull_mode: wgpu::CullMode::Back,
                    depth_bias: 0,
                    depth_bias_slope_scale: 0.0,
                    depth_bias_clamp: wgpu::MAX_DEPTH_BIAS_CLAMP,
                },
                primitive_topology: wgpu::PrimitiveTopology::TriangleList,
                color_states: &[
                    wgpu::ColorStateDescriptor {
                        format: sc_desc.format,
                        color: wgpu::BlendDescriptor::REPLACE,
                        alpha: wgpu::BlendDescriptor::REPLACE,
                        write_mask: wgpu::ColorWriteFlags::ALL,
                    },
                ],
                depth_stencil_state: None,
                index_format: wgpu::IndexFormat::Uint16,
                vertex_buffers: &[vb_desc],
                sample_count: 1,
            });

            Pass {
                pipeline,
                bind_group,
                uniform_buf,
            }
        };

        Example {
            entities,
            lights,
            lights_are_dirty: true,
            shadow_pass,
            forward_pass,
            light_uniform_buf,
        }
    }

    fn update(&mut self, event: wgpu::winit::WindowEvent) {
        if let wgpu::winit::WindowEvent::Resized(size) = event {
            let mx_total = Self::generate_matrix(size.width as f32 / size.height as f32);
            let mx_ref: &[f32; 16] = mx_total.as_ref();
            self.forward_pass.uniform_buf.set_sub_data(0, framework::cast_slice(&mx_ref[..]));
        }
    }

    fn render(&mut self, frame: &wgpu::SwapChainOutput, device: &mut wgpu::Device) {
        for entity in &self.entities {
            let raw: &[f32; 16] = entity.mx_world.as_ref();
            entity.uniform_buf.set_sub_data(0, framework::cast_slice(&raw[..]));
        }
        if self.lights_are_dirty {
            self.lights_are_dirty = false;
            let raw = self.lights
                .iter()
                .map(|light| light.to_raw())
                .collect::<Vec<_>>();
            self.light_uniform_buf.set_sub_data(0, framework::cast_slice(&raw));
        }

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { todo: 0 });

        for (_i, light) in self.lights.iter().enumerate() {
            //TODO: update light uniforms
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachmentDescriptor {
                    attachment: &light.target_view,
                    depth_load_op: wgpu::LoadOp::Clear,
                    depth_store_op: wgpu::StoreOp::Store,
                    stencil_load_op: wgpu::LoadOp::Clear,
                    stencil_store_op: wgpu::StoreOp::Store,
                    clear_depth: 1.0,
                    clear_stencil: 0,
                }),
            });
            pass.set_pipeline(&self.shadow_pass.pipeline);
            pass.set_bind_group(0, &self.shadow_pass.bind_group);

            for entity in &self.entities {
                pass.set_bind_group(1, &entity.bind_group);
                pass.set_index_buffer(&entity.index_buf, 0);
                pass.set_vertex_buffers(&[(&entity.vertex_buf, 0)]);
                pass.draw_indexed(0 .. entity.index_count as u32, 0, 0..1);
            }
        }

        // forward pass
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                    attachment: &frame.view,
                    load_op: wgpu::LoadOp::Clear,
                    store_op: wgpu::StoreOp::Store,
                    clear_color: wgpu::Color { r: 0.1, g: 0.2, b: 0.3, a: 1.0 },
                }],
                depth_stencil_attachment: None,
            });
            pass.set_pipeline(&self.shadow_pass.pipeline);
            pass.set_bind_group(0, &self.shadow_pass.bind_group);

            for entity in &self.entities {
                pass.set_bind_group(1, &entity.bind_group);
                pass.set_index_buffer(&entity.index_buf, 0);
                pass.set_vertex_buffers(&[(&entity.vertex_buf, 0)]);
                pass.draw_indexed(0 .. entity.index_count as u32, 0, 0..1);
            }
        }

        device
            .get_queue()
            .submit(&[encoder.finish()]);
    }
}

fn main() {
    framework::run::<Example>("shadow");
}
