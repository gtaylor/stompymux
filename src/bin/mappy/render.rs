//! GPU drawing of the hex map with a single fragment shader pass (`map.wgsl`).
//!
//! Each hex becomes one byte, its terrain index times ten plus its elevation, in an integer
//! texture. The texture follows the document's [`HexFeed`], so an edit uploads only the rows it
//! touched. The camera, brush and grid settings travel as a small uniform block every frame, so
//! panning and zooming cost the same however large the map is.

use iced::{
    Color, Rectangle, wgpu,
    widget::shader::{self, Viewport},
};
use stompymux_rs::{BattleHex, Terrain};

use crate::{
    document::HexFeed,
    map_view::{contrast, hex_color},
};

/// Elevation digits per terrain in the palette and in hex codes.
const ELEVATIONS: usize = 10;

/// Number of palette entries, one per terrain and elevation digit.
const PALETTE_LEN: usize = Terrain::ALL.len() * ELEVATIONS;

/// Values the shader reads every frame; see `Uniforms` in `map.wgsl`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Uniforms {
    pub size: [f32; 2],
    pub offset: [f32; 2],
    pub map_size: [f32; 2],
    pub hover: [f32; 2],
    pub radius: f32,
    pub grid_gap: f32,
    pub brush: f32,
    pub digits: f32,
}

impl Uniforms {
    /// The uniform block's bytes in WGSL layout: four `vec2<f32>` then four `f32`.
    fn to_bytes(self) -> Vec<u8> {
        [self.size, self.offset, self.map_size, self.hover]
            .into_iter()
            .flatten()
            .chain([self.radius, self.grid_gap, self.brush, self.digits])
            .flat_map(f32::to_ne_bytes)
            .collect()
    }
}

/// The byte the shader looks up a hex's color with.
fn hex_code(hex: BattleHex) -> u8 {
    let terrain = Terrain::ALL
        .iter()
        .position(|terrain| *terrain == hex.terrain())
        .unwrap_or(0);
    (terrain * ELEVATIONS) as u8 + hex.elevation().min(9)
}

/// Palette bytes: each entry's fill color, with its label ink (0 black, 1 white) as alpha.
/// Colors are converted to linear when the target applies sRGB encoding itself.
fn palette_bytes(srgb_target: bool) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(PALETTE_LEN * 16);
    for terrain in Terrain::ALL {
        for elevation in 0..ELEVATIONS as u8 {
            let fill = hex_color(terrain, elevation);
            let [red, green, blue, _] = if srgb_target {
                fill.into_linear()
            } else {
                [fill.r, fill.g, fill.b, fill.a]
            };
            let ink = if contrast(fill) == Color::WHITE {
                1.0
            } else {
                0.0
            };
            for channel in [red, green, blue, ink] {
                bytes.extend(channel.to_ne_bytes());
            }
        }
    }
    bytes
}

/// One frame of the map: where to read the hexes and their changes, and the uniforms.
#[derive(Debug)]
pub struct MapPrimitive {
    pub feed: HexFeed,
    pub width: u16,
    pub height: u16,
    pub uniforms: Uniforms,
}

/// GPU state shared by every frame: the pipeline, buffers and the uploaded hex texture.
pub struct MapPipeline {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    uniforms: wgpu::Buffer,
    palette: wgpu::Buffer,
    hexes: Option<HexTexture>,
}

/// The uploaded hexes, the bind group using them, and how far into which feed they are current.
struct HexTexture {
    texture: wgpu::Texture,
    size: (u16, u16),
    bind_group: wgpu::BindGroup,
    /// [`HexFeed::id`] of the hexes in the texture, or `None` before the first upload.
    feed_id: Option<u64>,
    /// How many entries of that feed's change log the texture includes.
    applied: usize,
}

impl shader::Pipeline for MapPipeline {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let uniform_entry = |binding, size: usize| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: wgpu::BufferSize::new(size as u64),
            },
            count: None,
        };
        let uniform_size = Uniforms {
            size: [0.0; 2],
            offset: [0.0; 2],
            map_size: [0.0; 2],
            hover: [0.0; 2],
            radius: 0.0,
            grid_gap: 0.0,
            brush: 0.0,
            digits: 0.0,
        }
        .to_bytes()
        .len();
        let palette = palette_bytes(format.is_srgb());
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mappy hex map layout"),
            entries: &[
                uniform_entry(0, uniform_size),
                uniform_entry(1, palette.len()),
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mappy hex map shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("map.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mappy hex map pipeline layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mappy hex map pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        let buffer = |label, size: usize| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: size as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let uniforms = buffer("mappy hex map uniforms", uniform_size);
        let palette_buffer = buffer("mappy hex map palette", palette.len());
        queue.write_buffer(&palette_buffer, 0, &palette);
        Self {
            pipeline,
            layout,
            uniforms,
            palette: palette_buffer,
            hexes: None,
        }
    }
}

impl MapPipeline {
    /// The hex texture for a map of this size, recreated when the size changes.
    fn texture(&mut self, device: &wgpu::Device, size: (u16, u16)) -> &mut HexTexture {
        if self.hexes.as_ref().is_some_and(|hexes| hexes.size == size) {
            return self.hexes.as_mut().expect("checked above");
        }
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("mappy hexes"),
            size: wgpu::Extent3d {
                width: u32::from(size.0),
                height: u32::from(size.1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Uint,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mappy hex map bind group"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniforms.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.palette.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
            ],
        });
        self.hexes.insert(HexTexture {
            texture,
            size,
            bind_group,
            feed_id: None,
            applied: 0,
        })
    }
}

impl shader::Primitive for MapPrimitive {
    type Pipeline = MapPipeline;

    fn prepare(
        &self,
        pipeline: &mut MapPipeline,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _bounds: &Rectangle,
        _viewport: &Viewport,
    ) {
        queue.write_buffer(&pipeline.uniforms, 0, &self.uniforms.to_bytes());
        // Frames are prepared before the next update can replace the document, so these upgrade;
        // if one ever does not, the texture is left as it is and the next frame catches up.
        let (Some(hexes), Some(changes)) = (self.feed.hexes.upgrade(), self.feed.changes.upgrade())
        else {
            return;
        };
        let texture = pipeline.texture(device, (self.width, self.height));
        let width = usize::from(self.width);
        let pending = (texture.feed_id == Some(self.feed.id))
            .then(|| changes.get(texture.applied..))
            .flatten();
        let rows = match pending {
            None => 0..usize::from(self.height),
            Some(pending) => {
                let (Some(first), Some(last)) = (pending.iter().min(), pending.iter().max()) else {
                    return;
                };
                *first as usize / width..*last as usize / width + 1
            }
        };
        let codes: Vec<u8> = hexes[rows.start * width..rows.end * width]
            .iter()
            .copied()
            .map(hex_code)
            .collect();
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture.texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: rows.start as u32,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &codes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width as u32),
                rows_per_image: Some(rows.len() as u32),
            },
            wgpu::Extent3d {
                width: width as u32,
                height: rows.len() as u32,
                depth_or_array_layers: 1,
            },
        );
        texture.feed_id = Some(self.feed.id);
        texture.applied = changes.len();
    }

    fn draw(&self, pipeline: &MapPipeline, render_pass: &mut wgpu::RenderPass<'_>) -> bool {
        let Some(hexes) = &pipeline.hexes else {
            return true;
        };
        render_pass.set_pipeline(&pipeline.pipeline);
        render_pass.set_bind_group(0, &hexes.bind_group, &[]);
        render_pass.draw(0..3, 0..1);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Brush, Document};
    use iced::{
        Size,
        widget::shader::{Pipeline, Primitive},
    };
    use std::{
        future::Future,
        pin::pin,
        task::{Context, Poll, Waker},
    };
    use stompymux_rs::BattleHexCoordinate;

    /// Drive a wgpu future to completion; native wgpu resolves them without a reactor.
    fn block_on<T>(future: impl Future<Output = T>) -> T {
        let mut future = pin!(future);
        let mut context = Context::from_waker(Waker::noop());
        loop {
            if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
                return value;
            }
            std::thread::yield_now();
        }
    }

    /// A device on any available adapter, software ones included, or `None` without one.
    fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            force_fallback_adapter: false,
            ..Default::default()
        }))
        .ok()?;
        block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()
    }

    /// Render one frame through the real pipeline and shader into RGBA bytes.
    fn render(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pipeline: &mut MapPipeline,
        primitive: &MapPrimitive,
        size: Size<u32>,
    ) -> Vec<u8> {
        let bounds = Rectangle::new(
            iced::Point::ORIGIN,
            Size::new(size.width as f32, size.height as f32),
        );
        primitive.prepare(
            pipeline,
            device,
            queue,
            &bounds,
            &Viewport::with_physical_size(size, 1.0),
        );
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: size.width,
                height: size.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = target.create_view(&wgpu::TextureViewDescriptor::default());
        let row_bytes = size.width * 4;
        assert_eq!(row_bytes % wgpu::COPY_BYTES_PER_ROW_ALIGNMENT, 0);
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: u64::from(row_bytes * size.height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            assert!(primitive.draw(pipeline, &mut pass));
        }
        encoder.copy_texture_to_buffer(
            target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row_bytes),
                    rows_per_image: Some(size.height),
                },
            },
            target.size(),
        );
        queue.submit([encoder.finish()]);
        readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, |result| result.unwrap());
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        readback.slice(..).get_mapped_range().to_vec()
    }

    /// Render target format for tests; not sRGB, so palette values are written unchanged.
    const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

    /// Hex vertex radius and map origin used by the render tests, in pixels.
    const RADIUS: f32 = 20.0;
    const OFFSET: [f32; 2] = [4.0, 4.0];

    /// A frame of `document` at the test camera, with no grid, digits or brush.
    fn frame(document: &Document, size: Size<u32>) -> MapPrimitive {
        let map = &document.map;
        MapPrimitive {
            feed: document.hex_feed(),
            width: map.width,
            height: map.height,
            uniforms: Uniforms {
                size: [size.width as f32, size.height as f32],
                offset: OFFSET,
                map_size: [f32::from(map.width), f32::from(map.height)],
                hover: [0.0, 0.0],
                radius: RADIUS,
                grid_gap: 1.0,
                brush: -1.0,
                digits: 0.0,
            },
        }
    }

    /// Assert that every hex in `document` renders in its palette color.
    fn assert_hex_colors(document: &Document, pixels: &[u8], size: Size<u32>) {
        let sqrt_3 = 3.0_f32.sqrt();
        for y in 0..i32::from(document.map.height) {
            for x in 0..i32::from(document.map.width) {
                let hex = document.hex(BattleHexCoordinate { x, y }).unwrap();
                let stagger = if x % 2 == 0 { 1.0 } else { 0.5 };
                let pixel_x = OFFSET[0] + RADIUS * (1.0 + 1.5 * x as f32);
                let pixel_y = OFFSET[1] + RADIUS * sqrt_3 * (y as f32 + stagger);
                let index = ((pixel_y as u32 * size.width + pixel_x as u32) * 4) as usize;
                let actual = &pixels[index..index + 4];
                let expected = hex_color(hex.terrain(), hex.elevation());
                for (channel, value) in [expected.r, expected.g, expected.b].into_iter().enumerate()
                {
                    let value = (value * 255.0).round() as i32;
                    assert!(
                        (i32::from(actual[channel]) - value).abs() <= 1,
                        "{x},{y} {hex:?} channel {channel}: {actual:?} vs {value}"
                    );
                }
                assert_eq!(actual[3], 255, "{x},{y}");
            }
        }
    }

    /// The hex codes index the palette by terrain order and elevation.
    #[test]
    fn hex_codes_follow_palette_order() {
        assert_eq!(hex_code(BattleHex::new(Terrain::Grassland, 0)), 0);
        assert_eq!(hex_code(BattleHex::new(Terrain::Road, 3)), 13);
        assert_eq!(
            usize::from(hex_code(BattleHex::new(Terrain::Sand, 9))),
            PALETTE_LEN - 1
        );
        assert_eq!(palette_bytes(false).len(), PALETTE_LEN * 16);
    }

    /// Hexes render in their palette colors, off-map pixels stay transparent, and an edit
    /// after the first upload reaches the screen through a partial upload. Skipped on
    /// machines without any graphics adapter.
    #[test]
    fn shader_draws_hexes_and_follows_edits() {
        let Some((device, queue)) = device() else {
            eprintln!("no graphics adapter; skipping");
            return;
        };
        let size = Size::new(128, 128);
        let mut pipeline = MapPipeline::new(&device, &queue, FORMAT);
        let mut document = Document::new(3, 3).unwrap();
        for (x, y, terrain, elevation) in [
            (0, 0, Terrain::Water, 2),
            (1, 0, Terrain::Road, 0),
            (2, 1, Terrain::HeavyForest, 0),
            (1, 2, Terrain::Sand, 1),
        ] {
            let brush = Brush {
                terrain: Some(terrain),
                elevation: Some(elevation),
                radius: 0,
            };
            document.paint(BattleHexCoordinate { x, y }, brush);
        }
        document.end_stroke();
        let pixels = render(
            &device,
            &queue,
            &mut pipeline,
            &frame(&document, size),
            size,
        );
        assert_hex_colors(&document, &pixels, size);
        assert_eq!(pixels[..4], [0, 0, 0, 0]);
        assert_eq!(pixels[pixels.len() - 4..], [0, 0, 0, 0]);

        let brush = Brush {
            terrain: Some(Terrain::Snow),
            elevation: Some(3),
            radius: 0,
        };
        document.paint(BattleHexCoordinate { x: 0, y: 1 }, brush);
        let pixels = render(
            &device,
            &queue,
            &mut pipeline,
            &frame(&document, size),
            size,
        );
        assert_eq!(pipeline.hexes.as_ref().unwrap().applied, 5);
        assert_hex_colors(&document, &pixels, size);
    }
}
