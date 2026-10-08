//! GPU drawing of the hex map with a single fragment shader pass (`map.wgsl`).
//!
//! Each hex becomes one `Rgba16Uint` texel holding every one of its layers (see
//! [`hex_texel`]), and the shader composes its look from them and a palette of layer colors
//! (see [`palette_colors`]). The texture follows the document's [`HexFeed`], so an edit
//! uploads only the rows it touched. The camera, brush and grid settings travel as a small
//! uniform block every frame, so panning and zooming cost the same however large the map is.

use iced::{
    Color, Rectangle, wgpu,
    widget::shader::{self, Viewport},
};
use stompymux_map::{
    Condition, ConstructionClass, DecorationKind, Flow, Foliage, Ground, Hex, Route, StructureKind,
};

use crate::{
    document::HexFeed,
    map_view::{
        condition_color, foliage_color, ground_color, overlay_color, route_color,
        structure_base_color, water_color,
    },
};

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
    /// Nonzero to write each hex's layer heights on it; see [`LABEL_LEGEND`].
    pub labels: f32,
}

/// What the numbers written on each hex mean. Ground level is the one elevation; every other
/// layer is an offset from it, signed to say which way: water depth as a negative number below
/// the surface (-2), which sits at ground level, and a bridge deck, building or wall as a
/// positive one above it (+2). Each has a fixed row, so a lone number still says which layer it
/// belongs to; ground level is left off bare level-zero hexes.
pub const LABEL_LEGEND: &str = "top: ground level · middle: +deck, building or wall height · \
                                bottom: -depth below the water";

impl Uniforms {
    /// The uniform block's bytes in WGSL layout: four `vec2<f32>` then four `f32`.
    fn to_bytes(self) -> Vec<u8> {
        [self.size, self.offset, self.map_size, self.hover]
            .into_iter()
            .flatten()
            .chain([self.radius, self.grid_gap, self.brush, self.labels])
            .flat_map(f32::to_ne_bytes)
            .collect()
    }
}

/// Where each layer's colors start in the palette, which `map.wgsl` names with the same
/// constants (`FOLIAGE`, `ROUTE` and so on). Each block lists its layer's values in that
/// layer's `ALL` order: [`Ground::ALL`], [`Foliage::ALL`], [`Route::ALL`], the one water color,
/// [`Condition::ALL`], [`StructureKind::ALL`] (light construction; the shader darkens stronger
/// classes), then fire and smoke. The label ink threshold follows in the red channel of
/// [`PALETTE_INK`].
pub const PALETTE_GROUND: usize = 0;
pub const PALETTE_FOLIAGE: usize = PALETTE_GROUND + Ground::ALL.len();
pub const PALETTE_ROUTE: usize = PALETTE_FOLIAGE + Foliage::ALL.len();
pub const PALETTE_WATER: usize = PALETTE_ROUTE + Route::ALL.len();
pub const PALETTE_CONDITION: usize = PALETTE_WATER + 1;
pub const PALETTE_STRUCTURE: usize = PALETTE_CONDITION + Condition::ALL.len();
pub const PALETTE_OVERLAY: usize = PALETTE_STRUCTURE + StructureKind::ALL.len();
pub const PALETTE_INK: usize = PALETTE_OVERLAY + 2;
/// Palette entries, the ink threshold included.
pub const PALETTE_LEN: usize = PALETTE_INK + 1;

/// Every layer color, in palette order; see [`PALETTE_GROUND`].
pub fn palette_colors() -> [Color; PALETTE_INK] {
    let mut colors = [Color::BLACK; PALETTE_INK];
    let mut fill = |start: usize, layer: &mut dyn Iterator<Item = Color>| {
        for (offset, color) in layer.enumerate() {
            colors[start + offset] = color;
        }
    };
    fill(
        PALETTE_GROUND,
        &mut Ground::ALL.into_iter().map(ground_color),
    );
    fill(
        PALETTE_FOLIAGE,
        &mut Foliage::ALL.into_iter().map(foliage_color),
    );
    fill(PALETTE_ROUTE, &mut Route::ALL.into_iter().map(route_color));
    fill(PALETTE_WATER, &mut [water_color()].into_iter());
    fill(
        PALETTE_CONDITION,
        &mut Condition::ALL.into_iter().map(condition_color),
    );
    fill(
        PALETTE_STRUCTURE,
        &mut StructureKind::ALL.into_iter().map(structure_base_color),
    );
    fill(
        PALETTE_OVERLAY,
        &mut [DecorationKind::Fire, DecorationKind::Smoke]
            .into_iter()
            .map(overlay_color),
    );
    colors
}

/// Position of `value` in a layer's `ALL` list.
fn position<T: PartialEq>(all: &[T], value: T) -> u16 {
    all.iter()
        .position(|candidate| *candidate == value)
        .expect("every layer value is in its ALL list") as u16
}

/// One plus the position of `value` in `all`, or zero for none.
fn optional<T: PartialEq>(all: &[T], value: Option<T>) -> u16 {
    value.map_or(0, |value| position(all, value) + 1)
}

/// Bytes in one hex texel: four `u16` channels.
const TEXEL_BYTES: u32 = 8;

/// A hex's layers packed for `map.wgsl`, one `u16` per channel:
///
/// - red: ground position in [`Ground::ALL`] (bits 0-3), then one plus the position in its
///   `ALL` list, or zero for none, of foliage (bits 4-6), route (bits 7-9) and condition
///   (bits 10-12), and the overlay (bits 13-14: none, fire, smoke);
/// - green: ground level (bits 0-7), water depth plus one or zero for none (bits 8-11) and
///   its flow (bits 12-13: still, rapids, torrent);
/// - blue: one plus the structure kind or zero for none (bits 0-1), its construction class
///   (bits 2-3) and its height or deck (bits 8-13);
/// - alpha: the structure's construction factor.
pub fn hex_texel(hex: Hex) -> [u16; 4] {
    let ground = position(&Ground::ALL, hex.ground());
    let foliage = optional(&Foliage::ALL, hex.foliage());
    let route = optional(&Route::ALL, hex.route());
    let condition = optional(&Condition::ALL, hex.condition());
    let overlay = optional(
        &[DecorationKind::Fire, DecorationKind::Smoke],
        hex.overlay(),
    );
    let water = hex.water().map_or(0, |water| {
        (u16::from(water.depth.min(14)) + 1) | position(&Flow::ALL, water.flow) << 4
    });
    let structure = hex.structure().map_or(0, |structure| {
        (position(&StructureKind::ALL, structure.kind) + 1)
            | position(&ConstructionClass::ALL, structure.class) << 2
            | u16::from(structure.height.min(63)) << 8
    });
    let cf = hex.structure().map_or(0, |structure| structure.cf);
    [
        ground | foliage << 4 | route << 7 | condition << 10 | overlay << 13,
        u16::from(hex.level()) | water << 8,
        structure,
        cf,
    ]
}

/// Palette bytes: each color of [`palette_colors`], then the luminance above which labels are
/// drawn in black rather than white. Colors and the threshold are converted to linear when the
/// target applies sRGB encoding itself, so they match the swatches iced draws.
fn palette_bytes(srgb_target: bool) -> Vec<u8> {
    let channels = |color: Color| {
        if srgb_target {
            color.into_linear()
        } else {
            [color.r, color.g, color.b, color.a]
        }
    };
    let threshold = channels(Color::from_rgb(0.55, 0.55, 0.55))[0];
    let mut bytes = Vec::with_capacity(PALETTE_LEN * 16);
    let entries = palette_colors()
        .into_iter()
        .map(channels)
        .chain([[threshold, 0.0, 0.0, 0.0]]);
    bytes.extend(entries.flatten().flat_map(f32::to_ne_bytes));
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
            labels: 0.0,
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
            format: wgpu::TextureFormat::Rgba16Uint,
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
        let texels: Vec<u8> = hexes[rows.start * width..rows.end * width]
            .iter()
            .copied()
            .flat_map(hex_texel)
            .flat_map(u16::to_ne_bytes)
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
            &texels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width as u32 * TEXEL_BYTES),
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
    use crate::document::Document;

    use iced::{
        Size,
        widget::shader::{Pipeline, Primitive},
    };
    use std::{
        future::Future,
        pin::pin,
        task::{Context, Poll, Waker},
    };
    use stompymux_map::{HexCoordinate, Structure, Terrain, Water};

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

    /// A frame of `document` at the test camera, with a grid and no labels or brush.
    fn frame(document: &Document, size: Size<u32>) -> MapPrimitive {
        labelled_frame(document, size, RADIUS, false)
    }

    /// A frame of `document` with hexes of `radius` pixels, with or without labels.
    fn labelled_frame(
        document: &Document,
        size: Size<u32>,
        radius: f32,
        labels: bool,
    ) -> MapPrimitive {
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
                radius,
                grid_gap: 1.0,
                brush: -1.0,
                labels: if labels { 1.0 } else { 0.0 },
            },
        }
    }

    /// The RGBA bytes `(dx, dy)` hex radii from the center of a hex, with hexes of `radius`
    /// pixels.
    fn hex_pixel(
        pixels: &[u8],
        size: Size<u32>,
        radius: f32,
        (x, y): (i32, i32),
        (dx, dy): (f32, f32),
    ) -> [u8; 4] {
        let stagger = if x % 2 == 0 { 1.0 } else { 0.5 };
        let pixel_x = OFFSET[0] + radius * (1.0 + 1.5 * x as f32 + dx);
        let pixel_y = OFFSET[1] + radius * (3.0_f32.sqrt() * (y as f32 + stagger) + dy);
        let index = ((pixel_y as u32 * size.width + pixel_x as u32) * 4) as usize;
        pixels[index..index + 4].try_into().unwrap()
    }

    /// The RGBA bytes at the center of a hex, at the test camera.
    fn center_pixel(pixels: &[u8], size: Size<u32>, x: i32, y: i32) -> [u8; 4] {
        hex_pixel(pixels, size, RADIUS, (x, y), (0.0, 0.0))
    }

    /// Assert that an opaque pixel is drawn in exactly `expected`.
    fn assert_pixel(actual: [u8; 4], expected: Color, place: &str) {
        for (channel, value) in [expected.r, expected.g, expected.b].into_iter().enumerate() {
            let value = (value * 255.0).round() as i32;
            assert!(
                (i32::from(actual[channel]) - value).abs() <= 1,
                "{place} channel {channel}: {actual:?} vs {value}"
            );
        }
        assert_eq!(actual[3], 255, "{place}");
    }

    /// The color of a rendered pixel.
    fn pixel_color(pixel: [u8; 4]) -> Color {
        Color::from_rgb8(pixel[0], pixel[1], pixel[2])
    }

    /// An opaque pixel of `color`.
    fn pixel_bytes(color: Color) -> [u8; 4] {
        let bytes = color_bytes(color);
        [bytes[0] as u8, bytes[1] as u8, bytes[2] as u8, 255]
    }

    /// Assert that a pixel shows water: far bluer than it is red.
    fn assert_blue(pixel: [u8; 4], place: &str) {
        assert!(
            i32::from(pixel[2]) > i32::from(pixel[0]) + 60,
            "{place} is not water: {pixel:?}"
        );
    }

    /// Assert that a hex's center is drawn in exactly `expected`.
    fn assert_color(pixels: &[u8], size: Size<u32>, x: i32, y: i32, expected: Color) {
        assert_pixel(
            center_pixel(pixels, size, x, y),
            expected,
            &format!("{x},{y}"),
        );
    }

    /// Texels pack each layer where `map.wgsl` reads it.
    #[test]
    fn texels_pack_every_layer() {
        let hex = Hex::at_level(30)
            .with_ground(Ground::Rough)
            .with_foliage(Some(Foliage::HeavyJungle))
            .with_route(Some(Route::DirtRoad))
            .with_condition(Some(Condition::Mud))
            .with_overlay(Some(DecorationKind::Smoke));
        assert_eq!(
            hex_texel(hex),
            [2 | 5 << 4 | 3 << 7 | 4 << 10 | 2 << 13, 30, 0, 0]
        );
        let torrent = Hex::at_level(4)
            .with_water(Some(Water {
                depth: 9,
                flow: Flow::Torrent,
            }))
            .with_condition(Some(Condition::Ice))
            .with_structure(Some(Structure::new(
                StructureKind::Bridge,
                3,
                ConstructionClass::Hardened,
            )));
        assert_eq!(
            hex_texel(torrent),
            [1 << 10, 4 | (10 | 2 << 4) << 8, 3 | 3 << 2 | 3 << 8, 150]
        );
        let wall = Hex::new(Terrain::Wall, 35);
        assert_eq!(hex_texel(wall)[2], 2 | 1 << 2 | 35 << 8);
        assert_eq!(hex_texel(wall)[3], 40);
        let fire = Hex::at_level(0)
            .with_ground(Ground::HeavyIndustrial)
            .with_overlay(Some(DecorationKind::Fire));
        assert_eq!(hex_texel(fire)[0], 11 | 1 << 13);
    }

    /// The palette holds one color per layer value, the shader's palette constants match
    /// render.rs, and no two layer colors are alike.
    #[test]
    fn palette_matches_the_shader() {
        let shader = include_str!("map.wgsl");
        for (name, value) in [
            ("GROUND", PALETTE_GROUND),
            ("FOLIAGE", PALETTE_FOLIAGE),
            ("ROUTE", PALETTE_ROUTE),
            ("WATER", PALETTE_WATER),
            ("CONDITION", PALETTE_CONDITION),
            ("STRUCTURE", PALETTE_STRUCTURE),
            ("OVERLAY", PALETTE_OVERLAY),
            ("INK_THRESHOLD", PALETTE_INK),
        ] {
            let declaration = format!("const {name}: u32 = {value}u;");
            assert!(
                shader.contains(&declaration),
                "map.wgsl lacks {declaration}"
            );
        }
        for (name, value) in [
            ("ROUGH", position(&Ground::ALL, Ground::Rough)),
            ("ULTRA_ROUGH", position(&Ground::ALL, Ground::UltraRough)),
            ("RUBBLE", position(&Ground::ALL, Ground::Rubble)),
            ("ULTRA_RUBBLE", position(&Ground::ALL, Ground::UltraRubble)),
            ("SAND", position(&Ground::ALL, Ground::Sand)),
            ("CLEAR", position(&Ground::ALL, Ground::Clear)),
            ("PAVEMENT", position(&Ground::ALL, Ground::Pavement)),
            ("TUNDRA", position(&Ground::ALL, Ground::Tundra)),
            ("SWAMP", position(&Ground::ALL, Ground::Swamp)),
            ("MAGMA_CRUST", position(&Ground::ALL, Ground::MagmaCrust)),
            ("MAGMA", position(&Ground::ALL, Ground::Magma)),
            (
                "HEAVY_INDUSTRIAL",
                position(&Ground::ALL, Ground::HeavyIndustrial),
            ),
            (
                "LIGHT_JUNGLE",
                position(&Foliage::ALL, Foliage::LightJungle),
            ),
            (
                "PLANTED_FIELDS",
                position(&Foliage::ALL, Foliage::PlantedFields),
            ),
            ("PAVED_ROAD", position(&Route::ALL, Route::PavedRoad)),
            ("GRAVEL_ROAD", position(&Route::ALL, Route::GravelRoad)),
            ("DIRT_ROAD", position(&Route::ALL, Route::DirtRoad)),
            ("RAIL", position(&Route::ALL, Route::Rail)),
            ("ICE", position(&Condition::ALL, Condition::Ice)),
            ("THIN_SNOW", position(&Condition::ALL, Condition::ThinSnow)),
            ("DEEP_SNOW", position(&Condition::ALL, Condition::DeepSnow)),
            ("MUD", position(&Condition::ALL, Condition::Mud)),
            (
                "BUILDING",
                position(&StructureKind::ALL, StructureKind::Building),
            ),
            (
                "BRIDGE",
                position(&StructureKind::ALL, StructureKind::Bridge),
            ),
            ("RAPIDS", position(&Flow::ALL, Flow::Rapids)),
            ("TORRENT", position(&Flow::ALL, Flow::Torrent)),
        ] {
            let declaration = format!("const {name}: u32 = {value}u;");
            assert!(
                shader.contains(&declaration),
                "map.wgsl lacks {declaration}"
            );
        }
        assert!(shader.contains(&format!("array<vec4<f32>, {PALETTE_LEN}>")));
        assert!(shader.contains(&format!(
            "const CLASS_SHADE: f32 = {};",
            crate::map_view::CLASS_SHADE
        )));
        let colors = palette_colors();
        assert_eq!(colors.len() + 1, PALETTE_LEN);
        assert_eq!(palette_bytes(false).len(), PALETTE_LEN * 16);
        for (index, a) in colors.iter().enumerate() {
            for b in &colors[index + 1..] {
                let distance = (a.r - b.r).abs() + (a.g - b.g).abs() + (a.b - b.b).abs();
                assert!(distance > 0.08, "{a:?} and {b:?} look alike");
            }
        }
    }

    /// Paint the hex at a coordinate, as one stroke.
    fn put(document: &mut Document, x: i32, y: i32, hex: Hex) {
        document.paint_with(HexCoordinate { x, y }, 0, |_| hex);
        document.end_stroke();
    }

    /// Hexes render in their layers' palette colors, stronger structures are darker, higher
    /// ground is lighter, water is blue, off-map pixels stay transparent, and an
    /// edit after the first upload reaches the screen through a partial upload. Skipped on
    /// machines without a graphics adapter.
    #[test]
    fn shader_draws_layers_and_follows_edits() {
        let Some((device, queue)) = device() else {
            eprintln!("no graphics adapter; skipping");
            return;
        };
        let size = Size::new(128, 128);
        let mut pipeline = MapPipeline::new(&device, &queue, FORMAT);
        let mut document = Document::new(3, 3).unwrap();
        let light_building = Structure::new(StructureKind::Building, 4, ConstructionClass::Light);
        let hardened_wall = Structure::new(StructureKind::Wall, 2, ConstructionClass::Hardened);
        put(&mut document, 0, 0, Hex::new(Terrain::Water, 0));
        put(
            &mut document,
            1,
            0,
            Hex::at_level(0).with_route(Some(Route::DirtRoad)),
        );
        put(
            &mut document,
            1,
            2,
            Hex::at_level(0).with_structure(Some(light_building)),
        );
        put(&mut document, 2, 2, Hex::at_level(20));
        put(
            &mut document,
            2,
            0,
            Hex::at_level(0).with_structure(Some(hardened_wall)),
        );
        let pixels = render(
            &device,
            &queue,
            &mut pipeline,
            &frame(&document, size),
            size,
        );
        assert_blue(center_pixel(&pixels, size, 0, 0), "water");
        assert_color(&pixels, size, 1, 0, route_color(Route::DirtRoad));
        // A lone wall crosses its hex east to west with a pillar at the middle; between pillars
        // its top is the wall color.
        assert_pixel(
            hex_pixel(&pixels, size, RADIUS, (2, 0), (-0.2, 0.0)),
            crate::map_view::structure_color(StructureKind::Wall, ConstructionClass::Hardened),
            "wall",
        );
        // Clear ground rolls gently, so it is compared with the same map left bare.
        let bare_document = Document::new(3, 3).unwrap();
        let bare_frame = frame(&bare_document, size);
        let mut bare_pipeline = MapPipeline::new(&device, &queue, FORMAT);
        let bare = render(&device, &queue, &mut bare_pipeline, &bare_frame, size);
        let bare_at = |x, y| pixel_color(center_pixel(&bare, size, x, y));
        assert_color(&pixels, size, 0, 1, bare_at(0, 1));
        // A building's skyline stands on its ground, which shows below it.
        let below = |pixels: &[u8]| hex_pixel(pixels, size, RADIUS, (1, 2), (0.0, 0.65));
        assert_pixel(
            below(&pixels),
            pixel_color(below(&bare)),
            "ground below a building",
        );
        let low = center_pixel(&pixels, size, 0, 1);
        let high = center_pixel(&pixels, size, 2, 2);
        assert!(
            (0..3).all(|channel| high[channel] > low[channel]),
            "{high:?} vs {low:?}"
        );
        assert_eq!(pixels[..4], [0, 0, 0, 0]);
        assert_eq!(pixels[pixels.len() - 4..], [0, 0, 0, 0]);

        let applied = pipeline.hexes.as_ref().unwrap().applied;
        let torrent = Hex::at_level(0).with_water(Some(Water {
            depth: 2,
            flow: Flow::Torrent,
        }));
        put(&mut document, 0, 1, torrent);
        put(&mut document, 0, 0, Hex::new(Terrain::DeepSnow, 0));
        let pixels = render(
            &device,
            &queue,
            &mut pipeline,
            &frame(&document, size),
            size,
        );
        assert_eq!(pipeline.hexes.as_ref().unwrap().applied, applied + 2);
        assert_blue(center_pixel(&pixels, size, 0, 1), "torrent");
        let snow = condition_color(Condition::DeepSnow);
        let clear = bare_at(0, 0);
        let mix = |a: f32, b: f32| a + (b - a) * 0.85;
        assert_color(
            &pixels,
            size,
            0,
            0,
            Color::from_rgb(
                mix(clear.r, snow.r),
                mix(clear.g, snow.g),
                mix(clear.b, snow.b),
            ),
        );
        assert_color(&pixels, size, 1, 0, route_color(Route::DirtRoad));
    }

    /// Hex radius and frame size for the label tests, large enough for the small row digits.
    const LABEL_RADIUS: f32 = 80.0;
    const LABEL_FRAME: Size<u32> = Size::new(256, 256);

    /// Whether labels put ink (pure black or white, not their halo) at `(dx, dy)` from the
    /// center of a lone `hex`, in hex radii, where the same hex drawn without them has none.
    /// Returns a lookup for any offset.
    fn label_ink(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        hex: Hex,
    ) -> impl Fn(f32, f32) -> bool {
        let (size, radius) = (LABEL_FRAME, LABEL_RADIUS);
        let mut document = Document::new(1, 1).unwrap();
        put(&mut document, 0, 0, hex);
        let mut pipeline = MapPipeline::new(device, queue, FORMAT);
        let plain = labelled_frame(&document, size, radius, false);
        let plain = render(device, queue, &mut pipeline, &plain, size);
        let labelled = labelled_frame(&document, size, radius, true);
        let labelled = render(device, queue, &mut pipeline, &labelled, size);
        let center_x = OFFSET[0] + radius;
        let center_y = OFFSET[1] + radius * 3.0_f32.sqrt();
        move |dx, dy| {
            let x = (center_x + dx * radius) as u32;
            let y = (center_y + dy * radius) as u32;
            let index = ((y * size.width + x) * 4) as usize;
            // Ink is pure black or white; its halo only shades the backdrop toward the other.
            let pixel = &labelled[index..index + 3];
            let pure = pixel.iter().all(|&v| v <= 24) || pixel.iter().all(|&v| v >= 231);
            pure && plain[index..index + 3] != *pixel
        }
    }

    /// Which of the three label rows (top, middle, bottom) are written on a lone `hex`.
    fn inked_rows(device: &wgpu::Device, queue: &wgpu::Queue, hex: Hex) -> [bool; 3] {
        let inked = label_ink(device, queue, hex);
        let steps =
            |from: f32, to: f32| (0..=20).map(move |step| from + (to - from) * step as f32 / 20.0);
        [-0.5, 0.0, 0.5].map(|row: f32| {
            steps(row - 0.2, row + 0.2).any(|dy| steps(-0.5, 0.5).any(|dx| inked(dx, dy)))
        })
    }

    /// Each layer is written in its own row, and bare level-zero ground is left blank.
    #[test]
    fn labels_use_fixed_rows() {
        let Some((device, queue)) = device() else {
            eprintln!("no graphics adapter; skipping");
            return;
        };
        let rows = |hex| inked_rows(&device, &queue, hex);
        let bridge = Hex::new(Terrain::Bridge, 2).with_level(4);
        assert_eq!(rows(bridge), [true, true, true]);
        let water = Hex::new(Terrain::Water, 3).with_level(2);
        assert_eq!(rows(water), [true, false, true]);
        let building = Hex::new(Terrain::Building, 3);
        assert_eq!(rows(building), [true, true, false]);
        assert_eq!(rows(Hex::at_level(5)), [true, false, false]);
        assert_eq!(rows(Hex::at_level(0)), [false, false, false]);
    }

    /// Depth is written with a minus sign and a bridge deck, building or wall height with a plus
    /// sign, each to the left of its digit, and neither sign appears on a zero.
    #[test]
    fn depth_and_height_labels_are_signed() {
        let Some((device, queue)) = device() else {
            eprintln!("no graphics adapter; skipping");
            return;
        };
        // In a row, a sign and one digit put the sign about 0.12 radius left of center, its
        // bar reaching past 0.16, where a lone digit, anti-aliased edge included, never does. A
        // plus also has a vertical stroke just above the bar's middle, which a minus lacks.
        let (bar, stem) = ((-0.16, 0.0), (-0.115, -0.045));
        let inked =
            |hex, row: f32, (dx, dy): (f32, f32)| label_ink(&device, &queue, hex)(dx, row + dy);
        let (middle, bottom) = (0.0, 0.5);
        let deep = Hex::new(Terrain::Water, 3);
        assert!(inked(deep, bottom, bar));
        assert!(!inked(deep, bottom, stem));
        assert!(!inked(Hex::new(Terrain::Water, 0), bottom, bar));
        let bridge = Hex::new(Terrain::Bridge, 3).with_level(4);
        assert!(inked(bridge, middle, bar));
        assert!(inked(bridge, middle, stem));
        assert!(!inked(Hex::new(Terrain::Bridge, 0), middle, bar));
        let building = Hex::new(Terrain::Building, 3).with_level(5);
        assert!(inked(building, middle, stem));
    }

    /// A building carries a skyline icon over the ground it stands on: blocks of the building
    /// color with lit windows, which labels fade so their numbers stay legible. Away from the
    /// icon, the hex shows the same ground it would without the building.
    #[test]
    fn buildings_draw_a_skyline_over_their_ground() {
        let Some((device, queue)) = device() else {
            eprintln!("no graphics adapter; skipping");
            return;
        };
        let stone = structure_base_color(StructureKind::Building);
        let block = Color::from_rgb(stone.r * 0.72, stone.g * 0.72, stone.b * 0.72);
        // In the left block: a window and the wall between window rows; and below the skyline,
        // clear of its shadow.
        let (window, wall, below) = ((-0.39, 0.15), (-0.34, 0.21), (0.0, 0.65));
        let building = Structure::new(StructureKind::Building, 3, ConstructionClass::Light);
        for ground in [Ground::Clear, Ground::HeavyIndustrial] {
            let bare = Hex::at_level(0).with_ground(ground);
            let built = bare.with_structure(Some(building));
            let probe = |hex, offset| middle_pixel(&device, &queue, &[((1, 1), hex)], offset);
            let window_color = Color::from_rgb(0.95, 0.9, 0.62);
            assert_pixel(
                probe(built, window),
                window_color,
                &format!("{ground:?} window"),
            );
            assert_pixel(probe(built, wall), block, &format!("{ground:?} wall"));
            let ground_there = pixel_color(probe(bare, below));
            assert_pixel(
                probe(built, below),
                ground_there,
                &format!("{ground:?} ground"),
            );
        }
        // With labels on, the icon fades toward the ground beneath it.
        let pixel = |hex, labels| {
            let (size, radius) = (LABEL_FRAME, LABEL_RADIUS);
            let mut document = Document::new(1, 1).unwrap();
            put(&mut document, 0, 0, hex);
            let mut pipeline = MapPipeline::new(&device, &queue, FORMAT);
            let frame = labelled_frame(&document, size, radius, labels);
            let pixels = render(&device, &queue, &mut pipeline, &frame, size);
            pixel_color(hex_pixel(&pixels, size, radius, (0, 0), wall))
        };
        let ground = pixel(Hex::at_level(0), false);
        let faded = |g: f32, b: f32| g + (b - g) * 0.3;
        let expected = Color::from_rgb(
            faded(ground.r, block.r),
            faded(ground.g, block.g),
            faded(ground.b, block.b),
        );
        let built = Hex::at_level(0).with_structure(Some(building));
        assert_pixel(pixel_bytes(pixel(built, true)), expected, "labelled wall");
    }

    /// Woods and jungle grow as stands of separate plants, green over the ground, covering more
    /// of it the denser the stand, with light stands leaving bare ground between plants. At a
    /// small zoom a stand takes one flat color.
    #[test]
    fn woods_and_jungle_grow_as_stands() {
        let Some((device, queue)) = device() else {
            eprintln!("no graphics adapter; skipping");
            return;
        };
        let clear = || Plain::Hex(Hex::at_level(0));
        let stands = [
            [
                Foliage::LightWoods,
                Foliage::HeavyWoods,
                Foliage::UltraHeavyWoods,
            ],
            [
                Foliage::LightJungle,
                Foliage::HeavyJungle,
                Foliage::UltraHeavyJungle,
            ],
        ];
        for stand in stands {
            let mut last_cover = 0;
            for foliage in stand {
                let hex = Hex::at_level(0).with_foliage(Some(foliage));
                let (marks, bare) = hex_texture(&device, &queue, hex, clear(), 40.0);
                // Leaves, lit or shaded, are clearly greener than they are red.
                let cover = marks.iter().filter(|m| m[1] > m[0] + 25).count();
                assert!(
                    cover > last_cover,
                    "{foliage:?} covers {cover}, not over {last_cover}"
                );
                if foliage == stand[0] {
                    assert!(bare > 50, "{foliage:?} leaves {bare} bare");
                }
                last_cover = cover;
                let (marks, _) = hex_texture(&device, &queue, hex, clear(), 8.0);
                let flat = marks
                    .first()
                    .copied()
                    .expect("a stand changes the ground's color");
                assert!(
                    marks
                        .iter()
                        .all(|m| (0..3).all(|c| (m[c] - flat[c]).abs() <= 1)),
                    "{foliage:?} is not flat at a small zoom"
                );
            }
        }
    }

    /// Planted fields grow in plots edged by dark hedgerows, sown in rows of plants lit from
    /// the upper left, with the hedgerows and plants fading out at a small zoom.
    #[test]
    fn planted_fields_grow_in_plots() {
        let Some((device, queue)) = device() else {
            eprintln!("no graphics adapter; skipping");
            return;
        };
        let field = Hex::at_level(0).with_foliage(Some(Foliage::PlantedFields));
        let count = |radius| {
            let bare = Plain::Hex(Hex::at_level(0));
            let (marks, _) = hex_texture(&device, &queue, field, bare, radius);
            let hedges = marks.iter().filter(|m| m.iter().all(|&v| v < 120)).count();
            let lit_plants = marks.iter().filter(|m| m[1] > 190).count();
            (hedges, lit_plants)
        };
        let (hedges, lit_plants) = count(40.0);
        assert!(
            hedges > 5 && lit_plants > 3,
            "{hedges} hedge, {lit_plants} lit plant"
        );
        assert_eq!(count(8.0), (0, 0), "hedges or plants at a small zoom");
    }

    /// The pixel `offset` hex radii from the center of the middle hex of a 3 by 3 map of clear
    /// ground with `hexes` painted on it.
    fn middle_pixel(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        hexes: &[((i32, i32), Hex)],
        offset: (f32, f32),
    ) -> [u8; 4] {
        let (size, radius) = (LABEL_FRAME, 40.0);
        let mut document = Document::new(3, 3).unwrap();
        for &((x, y), hex) in hexes {
            put(&mut document, x, y, hex);
        }
        let mut pipeline = MapPipeline::new(device, queue, FORMAT);
        let frame = labelled_frame(&document, size, radius, false);
        let pixels = render(device, queue, &mut pipeline, &frame, size);
        hex_pixel(&pixels, size, radius, (1, 1), offset)
    }

    /// Roads, rail and bridges run from the hex center toward each neighbor they join: straight
    /// through, around elbows, to a dead end in the middle of the hex, or straight across when
    /// joining nothing. Rail does not join roads, and a bridge joining one neighbor spans its
    /// whole hex.
    #[test]
    fn ways_join_their_neighbors() {
        let Some((device, queue)) = device() else {
            eprintln!("no graphics adapter; skipping");
            return;
        };
        // Dirt, whose middle runs plain between its ruts, so nothing covers the probes along
        // each line's middle.
        let road = Hex::at_level(0).with_route(Some(Route::DirtRoad));
        let rail = Hex::at_level(0).with_route(Some(Route::Rail));
        let bridge = Hex::at_level(0)
            .with_water(Some(Water {
                depth: 1,
                flow: Flow::Still,
            }))
            .with_structure(Some(Structure::new(
                StructureKind::Bridge,
                1,
                ConstructionClass::Light,
            )));
        let dirt = route_color(Route::DirtRoad);
        // Clear ground rolls gently, so each probe of it is compared with the bare map.
        let clear = |offset| pixel_color(middle_pixel(&device, &queue, &[], offset));
        let deck = structure_base_color(StructureKind::Bridge);
        // Points 0.6 hex radii from the middle hex's center toward its north, southeast, south
        // and west, the last between two edges.
        let (north, southeast, south, west) = ((0.0, -0.6), (0.52, 0.3), (0.0, 0.6), (-0.6, 0.0));
        let probe =
            |hexes: &[((i32, i32), Hex)], offset| middle_pixel(&device, &queue, hexes, offset);
        let check = |hexes: &[((i32, i32), Hex)], expected: [(_, Color, &str); 4]| {
            for (offset, color, place) in expected {
                assert_pixel(probe(hexes, offset), color, place);
            }
        };
        // The middle hex (1, 1) sits in an odd column: (1, 0) is north of it, (2, 1) southeast
        // and (1, 2) south.
        let vertical = [((1, 0), road), ((1, 1), road), ((1, 2), road)];
        check(
            &vertical,
            [
                (north, dirt, "straight north"),
                (south, dirt, "straight south"),
                (west, clear(west), "straight west"),
                (southeast, clear(southeast), "straight southeast"),
            ],
        );
        let elbow = [((1, 0), road), ((1, 1), road), ((2, 1), road)];
        check(
            &elbow,
            [
                (north, dirt, "elbow north"),
                (southeast, dirt, "elbow southeast"),
                (south, clear(south), "elbow south"),
                (west, clear(west), "elbow west"),
            ],
        );
        // The elbow curves through the hex rather than turning at its middle, which stays bare.
        let middle = (0.0, 0.0);
        assert_pixel(probe(&elbow, middle), clear(middle), "elbow middle");
        // A sharp elbow, north to northeast, curves around the corner between them: through the
        // point half a radius in from that corner, leaving the middle bare.
        let sharp = [((1, 0), road), ((1, 1), road), ((2, 0), road)];
        assert_pixel(probe(&sharp, (0.25, -0.433)), dirt, "sharp elbow bend");
        assert_pixel(probe(&sharp, middle), clear(middle), "sharp elbow middle");
        let dead_end = [((1, 0), road), ((1, 1), road)];
        check(
            &dead_end,
            [
                (north, dirt, "dead end north"),
                ((0.0, 0.0), dirt, "dead end middle"),
                (south, clear(south), "dead end south"),
                (west, clear(west), "dead end west"),
            ],
        );
        let rail_by_road = [((1, 0), road), ((1, 1), rail), ((1, 2), road)];
        assert_pixel(probe(&rail_by_road, north), clear(north), "rail north");
        assert_pixel(probe(&rail_by_road, south), clear(south), "rail south");
        let bridge_end = [((1, 0), road), ((1, 1), bridge)];
        for (offset, place) in [
            (north, "bridge north"),
            ((0.0, 0.0), "bridge middle"),
            (south, "bridge south"),
        ] {
            assert_pixel(probe(&bridge_end, offset), deck, place);
        }
        let beside = probe(&bridge_end, west);
        let deck_bytes = color_bytes(deck);
        assert!(
            (0..3).any(|c| (i32::from(beside[c]) - deck_bytes[c]).abs() > 30),
            "the deck spreads west: {beside:?}"
        );
    }

    /// Walls run from the hex center toward each neighboring wall, like roads, leaving the
    /// ground beside them bare, and join only other walls.
    #[test]
    fn walls_join_their_neighbors() {
        let Some((device, queue)) = device() else {
            eprintln!("no graphics adapter; skipping");
            return;
        };
        let class = ConstructionClass::Medium;
        let wall =
            Hex::at_level(0).with_structure(Some(Structure::new(StructureKind::Wall, 2, class)));
        let road = Hex::at_level(0).with_route(Some(Route::DirtRoad));
        let stone = crate::map_view::structure_color(StructureKind::Wall, class);
        // Probes along the wall sit in the middle of a block, clear of the joints between
        // blocks, which fall every 0.12 hex radii from the hex edge.
        let (north, southeast) = ((0.0, -0.566), (0.49, 0.283));
        let (south, west) = ((0.0, 0.6), (-0.6, 0.0));
        let probe =
            |hexes: &[((i32, i32), Hex)], offset| middle_pixel(&device, &queue, hexes, offset);
        let bare = |offset| pixel_color(middle_pixel(&device, &queue, &[], offset));
        // The middle hex (1, 1) sits in an odd column: (1, 0) is north of it, (2, 1) southeast
        // and (1, 2) south.
        let elbow = [((1, 0), wall), ((1, 1), wall), ((2, 1), wall)];
        assert_pixel(probe(&elbow, north), stone, "elbow north");
        assert_pixel(probe(&elbow, southeast), stone, "elbow southeast");
        assert_pixel(probe(&elbow, south), bare(south), "elbow south");
        assert_pixel(probe(&elbow, west), bare(west), "elbow west");
        // A pillar stands where the arms meet, its top lighter than the wall.
        let lift = |v: f32| v + (1.0 - v) * 0.14;
        let pillar_top = Color::from_rgb(lift(stone.r), lift(stone.g), lift(stone.b));
        assert_pixel(probe(&elbow, (0.0, 0.0)), pillar_top, "elbow pillar");
        // A road beside a wall does not join it: the lone wall crosses its hex east to west.
        let beside_road = [((1, 0), road), ((1, 1), wall)];
        assert_pixel(probe(&beside_road, north), bare(north), "wall toward road");
        assert_pixel(probe(&beside_road, (0.514, 0.0)), stone, "lone wall east");
    }

    /// Renders a lone `hex` with hexes `LABEL_RADIUS` pixels across and returns a lookup of the
    /// RGB bytes `(dx, dy)` hex radii from its center. A way in a lone hex joins nothing, so it
    /// crosses the hex east to west.
    fn lone_hex(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        hex: Hex,
    ) -> impl Fn(f32, f32) -> [i32; 3] {
        let (size, radius) = (LABEL_FRAME, LABEL_RADIUS);
        let mut document = Document::new(1, 1).unwrap();
        put(&mut document, 0, 0, hex);
        let mut pipeline = MapPipeline::new(device, queue, FORMAT);
        let frame = labelled_frame(&document, size, radius, false);
        let pixels = render(device, queue, &mut pipeline, &frame, size);
        move |dx, dy| {
            let pixel = hex_pixel(&pixels, size, radius, (0, 0), (dx, dy));
            [0, 1, 2].map(|c| i32::from(pixel[c]))
        }
    }

    /// Offsets every 0.01 hex radii from -0.5 to 0.5 along a way crossing a lone hex, `across`
    /// from its middle.
    fn along_way(across: f32) -> impl Iterator<Item = (f32, f32)> {
        (-50..=50).map(move |step| (step as f32 * 0.01, across))
    }

    /// Dirt roads have two darker wheel ruts either side of a plain middle, and ragged edges
    /// that wander in and out.
    #[test]
    fn dirt_roads_have_ruts_and_ragged_edges() {
        let Some((device, queue)) = device() else {
            eprintln!("no graphics adapter; skipping");
            return;
        };
        let road = Hex::at_level(0).with_route(Some(Route::DirtRoad));
        let pixel = lone_hex(&device, &queue, road);
        let dirt = color_bytes(route_color(Route::DirtRoad));
        let rut = dirt.map(|v| (v as f32 * 0.72).round() as i32);
        let bare = lone_hex(&device, &queue, Hex::at_level(0));
        let near =
            |bytes: [i32; 3], color: [i32; 3]| (0..3).all(|c| (bytes[c] - color[c]).abs() <= 1);
        assert!(near(pixel(0.0, 0.0), dirt), "plain middle");
        assert!(near(pixel(0.0, 0.07), rut), "rut");
        // Near the edge, the road reaches past some points and falls short of others.
        let edge: Vec<_> = along_way(0.15).map(|(dx, dy)| pixel(dx, dy)).collect();
        assert!(
            edge.iter().any(|&p| near(p, dirt)),
            "the edge never reaches out"
        );
        assert!(
            along_way(0.15).any(|(dx, dy)| near(pixel(dx, dy), bare(dx, dy))),
            "the edge never pulls in"
        );
    }

    /// Gravel roads have a bed speckled with darker stones between pale shoulders.
    #[test]
    fn gravel_roads_have_specks_and_shoulders() {
        let Some((device, queue)) = device() else {
            eprintln!("no graphics adapter; skipping");
            return;
        };
        let road = Hex::at_level(0).with_route(Some(Route::GravelRoad));
        let pixel = lone_hex(&device, &queue, road);
        let gravel = color_bytes(route_color(Route::GravelRoad));
        let speck = gravel.map(|v| (v as f32 * 0.75).round() as i32);
        let near =
            |bytes: [i32; 3], color: [i32; 3]| (0..3).all(|c| (bytes[c] - color[c]).abs() <= 1);
        let bed: Vec<_> = along_way(0.0).map(|(dx, dy)| pixel(dx, dy)).collect();
        assert!(bed.iter().any(|&p| near(p, gravel)), "no plain gravel");
        assert!(bed.iter().any(|&p| near(p, speck)), "no darker stones");
        let shoulder = route_color(Route::GravelRoad);
        let lift = |v: f32| v + (1.0 - v) * 0.35;
        let shoulder = color_bytes(Color::from_rgb(
            lift(shoulder.r),
            lift(shoulder.g),
            lift(shoulder.b),
        ));
        for (dx, dy) in along_way(0.14) {
            assert!(near(pixel(dx, dy), shoulder), "no shoulder at {dx}");
        }
    }

    /// Rail track is two dark steel rails on wooden ties over a ballast bed, with the ground
    /// showing beyond the bed.
    #[test]
    fn rail_is_track_on_ties() {
        let Some((device, queue)) = device() else {
            eprintln!("no graphics adapter; skipping");
            return;
        };
        let pixel = lone_hex(
            &device,
            &queue,
            Hex::at_level(0).with_route(Some(Route::Rail)),
        );
        // The rails, thin at this zoom, still come out far darker than anything around them.
        for (dx, dy) in along_way(0.055) {
            assert!(pixel(dx, dy)[0] < 80, "no rail at {dx}");
        }
        // Between the rails, the warm brown ties alternate with the grayer ballast.
        let middle: Vec<_> = along_way(0.0).map(|(dx, dy)| pixel(dx, dy)).collect();
        let tie = |p: &&[i32; 3]| p[0] > p[2] + 40;
        let ballast = |p: &&[i32; 3]| p[0] < p[2] + 20 && p[0] > 100;
        assert!(middle.iter().any(|p| tie(&p)), "no ties");
        assert!(middle.iter().any(|p| ballast(&p)), "no ballast");
        let bare = lone_hex(&device, &queue, Hex::at_level(0));
        assert_eq!(pixel(0.0, 0.25), bare(0.0, 0.25), "the bed spreads too far");
    }

    /// Bridge decks are concrete crossed by darker expansion joints, with dark railings along
    /// both edges.
    #[test]
    fn bridges_have_joints_and_railings() {
        let Some((device, queue)) = device() else {
            eprintln!("no graphics adapter; skipping");
            return;
        };
        let bridge = Hex::at_level(0)
            .with_water(Some(Water {
                depth: 1,
                flow: Flow::Still,
            }))
            .with_structure(Some(Structure::new(
                StructureKind::Bridge,
                1,
                ConstructionClass::Light,
            )));
        let pixel = lone_hex(&device, &queue, bridge);
        let deck = color_bytes(structure_base_color(StructureKind::Bridge));
        assert_eq!(pixel(0.0, 0.0), deck, "plain deck");
        // A deck that joins nothing enters at the hex's western vertex, so joints fall a third
        // of the way in from either edge's middle.
        let joint = pixel(-0.289, 0.0);
        assert!(
            (0..3).all(|c| joint[c] < deck[c] - 10),
            "no joint: {joint:?}"
        );
        for (dx, dy) in along_way(0.148) {
            let railing = pixel(dx, dy);
            assert!(
                (0..3).all(|c| railing[c] < deck[c] - 40),
                "no railing at {dx}"
            );
        }
    }

    /// Paved roads carry a dashed white line down their middle, with a dash centered on each
    /// hex of a straight road and gaps across its edges.
    #[test]
    fn paved_roads_have_lane_markings() {
        let Some((device, queue)) = device() else {
            eprintln!("no graphics adapter; skipping");
            return;
        };
        let road = Hex::at_level(0).with_route(Some(Route::PavedRoad));
        let vertical = [((1, 0), road), ((1, 1), road), ((1, 2), road)];
        let paved = route_color(Route::PavedRoad);
        let lane = Color::from_rgb(0.95, 0.95, 0.9);
        // Dashes are two thirds of the way from a hex edge to its center apart, so along the
        // middle of the road a dash sits at the center and a third of the way in from an edge,
        // with a gap halfway between them.
        for (offset, expected, place) in [
            ((0.0, 0.0), lane, "dash at the center"),
            ((0.0, -0.577), lane, "dash near the edge"),
            ((0.0, -0.289), paved, "gap"),
            ((0.1, 0.0), paved, "beside the dash"),
        ] {
            assert_pixel(
                middle_pixel(&device, &queue, &vertical, offset),
                expected,
                place,
            );
        }
    }

    /// Water runs from the middle of its hex toward each neighboring water hex, leaving banks
    /// beside it: straight through a river, around an elbow, into a round pond when it has no
    /// water beside it, and across the whole hex in a lake.
    #[test]
    fn water_runs_toward_neighboring_water() {
        let Some((device, queue)) = device() else {
            eprintln!("no graphics adapter; skipping");
            return;
        };
        let lake = Hex::at_level(0).with_water(Some(Water {
            depth: 1,
            flow: Flow::Still,
        }));
        let wet = |pixel: [u8; 4]| i32::from(pixel[2]) > i32::from(pixel[0]) + 60;
        let (north, southeast, south, west) = ((0.0, -0.6), (0.52, 0.3), (0.0, 0.6), (-0.6, 0.0));
        // The middle hex (1, 1) sits in an odd column: (1, 0) is north of it, (2, 1) southeast
        // and (1, 2) south.
        let river = [((1, 0), lake), ((1, 1), lake), ((1, 2), lake)];
        let elbow = [((1, 0), lake), ((1, 1), lake), ((2, 1), lake)];
        let pond = [((1, 1), lake)];
        let all: Vec<_> = (0..3)
            .flat_map(|x| (0..3).map(move |y| ((x, y), lake)))
            .collect();
        for (hexes, offset, water, place) in [
            (&river[..], north, true, "river north"),
            (&river[..], south, true, "river south"),
            (&river[..], west, false, "river west"),
            (&elbow[..], north, true, "elbow north"),
            (&elbow[..], southeast, true, "elbow southeast"),
            (&elbow[..], south, false, "elbow south"),
            (&pond[..], (0.0, 0.0), true, "pond middle"),
            (&pond[..], (0.75, 0.0), false, "pond bank"),
            (&all[..], (0.8, 0.0), true, "lake east"),
            (&all[..], (0.0, -0.8), true, "lake north"),
        ] {
            let pixel = middle_pixel(&device, &queue, hexes, offset);
            assert_eq!(wet(pixel), water, "{place}: {pixel:?}");
        }
    }

    /// Ice covers the water in a hex as a pale sheet and leaves its banks bare.
    #[test]
    fn ice_covers_only_the_water() {
        let Some((device, queue)) = device() else {
            eprintln!("no graphics adapter; skipping");
            return;
        };
        let river = Hex::at_level(0).with_water(Some(Water {
            depth: 1,
            flow: Flow::Still,
        }));
        let frozen = river.with_condition(Some(Condition::Ice));
        let hexes = [((1, 0), river), ((1, 1), frozen), ((1, 2), river)];
        let ice = middle_pixel(&device, &queue, &hexes, (0.0, -0.6));
        assert!(ice[0] > 120 && ice[2] > 200, "no ice on the water: {ice:?}");
        let bank = middle_pixel(&device, &queue, &hexes, (-0.7, 0.0));
        let bare = middle_pixel(&device, &queue, &[], (-0.7, 0.0));
        assert_pixel(bank, pixel_color(bare), "bank");
    }

    /// Still water ripples, rapids streak, and torrents streak hardest, with their marks fading
    /// out when the hexes are too small to show them.
    #[test]
    fn faster_water_streaks_more() {
        let Some((device, queue)) = device() else {
            eprintln!("no graphics adapter; skipping");
            return;
        };
        let water = water_color();
        let plain = Color::from_rgb(water.r * 0.92, water.g * 0.92, water.b * 0.92);
        let plain_red = color_bytes(plain)[0];
        let marks = |flow, radius| {
            let hex = Hex::at_level(0).with_water(Some(Water { depth: 1, flow }));
            let (marks, _) = hex_texture(&device, &queue, hex, Plain::Color(plain), radius);
            // Ripples and streaks lighten the water far more than a torrent's froth does.
            marks.iter().filter(|m| m[0] > plain_red + 60).count()
        };
        let (still, rapids, torrent) = (
            marks(Flow::Still, 40.0),
            marks(Flow::Rapids, 40.0),
            marks(Flow::Torrent, 40.0),
        );
        assert!(
            0 < still && still < rapids && rapids < torrent,
            "{still} still, {rapids} rapids, {torrent} torrent"
        );
        assert_eq!(marks(Flow::Torrent, 8.0), 0, "streaks at a small zoom");
    }

    /// How far, on average over `marks` and `bare` unmarked pixels, a texture moves the pixels
    /// of `ground` from its color, and how many of the marks are lighter and darker than it.
    fn relief_spread(marks: &[[i32; 3]], bare: usize, ground: Ground) -> (f32, usize, usize) {
        let bytes = color_bytes(ground_color(ground));
        let shift = |m: &[i32; 3]| (0..3).map(|c| m[c] - bytes[c]).sum::<i32>();
        let total: i32 = marks.iter().map(|m| shift(m).abs()).sum();
        let lighter = marks.iter().filter(|m| shift(m) > 6).count();
        let darker = marks.iter().filter(|m| shift(m) < -6).count();
        (total as f32 / (marks.len() + bare) as f32, lighter, darker)
    }

    /// Rough ground is shaded into hummocks, lit on some slopes and shadowed on others, and ultra
    /// rough ground into rougher ones; both fade to plain ground at a small zoom.
    #[test]
    fn rough_ground_has_hummocks() {
        let Some((device, queue)) = device() else {
            eprintln!("no graphics adapter; skipping");
            return;
        };
        let mut last_spread = 0.0;
        for ground in [Ground::Rough, Ground::UltraRough] {
            let (marks, bare) = ground_texture(&device, &queue, ground, 40.0);
            let (spread, lighter, darker) = relief_spread(&marks, bare, ground);
            assert!(
                lighter > 20 && darker > 20,
                "{ground:?}: {lighter} lit, {darker} shadowed"
            );
            assert!(
                spread > last_spread,
                "{ground:?} spreads {spread}, not over {last_spread}"
            );
            last_spread = spread;
            let (marks, _) = ground_texture(&device, &queue, ground, 8.0);
            assert!(marks.is_empty(), "{ground:?} shaded at a small zoom");
        }
    }

    /// Rubble carries ruined walls and debris, dark walls and block edges with lighter tops and
    /// faces, and ultra rubble more of them; both fade to plain ground at a small zoom.
    #[test]
    fn rubble_has_ruins_and_debris() {
        let Some((device, queue)) = device() else {
            eprintln!("no graphics adapter; skipping");
            return;
        };
        let mut last_marks = 0;
        for ground in [Ground::Rubble, Ground::UltraRubble] {
            let (marks, bare) = ground_texture(&device, &queue, ground, 40.0);
            let (_, lighter, darker) = relief_spread(&marks, bare, ground);
            assert!(
                lighter > 0 && darker > 20,
                "{ground:?}: {lighter} light, {darker} dark"
            );
            assert!(
                marks.len() > last_marks,
                "{ground:?} marks {}, not over {last_marks}",
                marks.len()
            );
            last_marks = marks.len();
            let (marks, _) = ground_texture(&device, &queue, ground, 8.0);
            assert!(marks.is_empty(), "{ground:?} marked at a small zoom");
        }
    }

    /// Samples the middle of the middle hex of a 3 by 3 map of bare `ground` drawn with hexes
    /// `radius` pixels across, returning the colors of the pixels its texture marks and the
    /// count of pixels of bare ground.
    fn ground_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        ground: Ground,
        radius: f32,
    ) -> (Vec<[i32; 3]>, usize) {
        let hex = Hex::at_level(0).with_ground(ground);
        hex_texture(
            device,
            queue,
            hex,
            Plain::Color(ground_color(ground)),
            radius,
        )
    }

    /// What a sampled pixel counts as unmarked against: one flat color, or the same pixel of
    /// the same map filled with a bare hex.
    enum Plain {
        Color(Color),
        Hex(Hex),
    }

    /// Renders a 3 by 3 map filled with `hex`, with hexes `radius` pixels across.
    fn filled_map(device: &wgpu::Device, queue: &wgpu::Queue, hex: Hex, radius: f32) -> Vec<u8> {
        let mut document = Document::new(3, 3).unwrap();
        for x in 0..3 {
            for y in 0..3 {
                put(&mut document, x, y, hex);
            }
        }
        let mut pipeline = MapPipeline::new(device, queue, FORMAT);
        let frame = labelled_frame(&document, LABEL_FRAME, radius, false);
        render(device, queue, &mut pipeline, &frame, LABEL_FRAME)
    }

    /// Samples the middle of the middle hex of a 3 by 3 map of `hex` drawn with hexes `radius`
    /// pixels across, returning the colors of the pixels that differ from `plain` and the count
    /// of pixels that match it.
    fn hex_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        hex: Hex,
        plain: Plain,
        radius: f32,
    ) -> (Vec<[i32; 3]>, usize) {
        let size = LABEL_FRAME;
        let pixels = filled_map(device, queue, hex, radius);
        let reference = match plain {
            Plain::Hex(bare) => Some(filled_map(device, queue, bare, radius)),
            Plain::Color(_) => None,
        };
        let flat = match plain {
            Plain::Color(color) => color_bytes(color),
            Plain::Hex(_) => [0; 3],
        };
        let (mut marks, mut bare) = (Vec::new(), 0);
        for step_y in -10..=10 {
            for step_x in -10..=10 {
                let offset = (step_x as f32 * 0.05, step_y as f32 * 0.05);
                let pixel = hex_pixel(&pixels, size, radius, (1, 1), offset);
                // Skip the hex's antialiased border, which a small zoom brings in range.
                if pixel[3] != 255 {
                    continue;
                }
                let channels = [0, 1, 2].map(|c| i32::from(pixel[c]));
                let bytes = reference.as_ref().map_or(flat, |reference| {
                    let there = hex_pixel(reference, size, radius, (1, 1), offset);
                    [0, 1, 2].map(|c| i32::from(there[c]))
                });
                if (0..3).all(|c| (channels[c] - bytes[c]).abs() <= 1) {
                    bare += 1;
                } else {
                    marks.push(channels);
                }
            }
        }
        (marks, bare)
    }

    /// A color's channels as bytes.
    fn color_bytes(color: Color) -> [i32; 3] {
        [color.r, color.g, color.b].map(|v| (v * 255.0).round() as i32)
    }

    /// Sand is shaded into dunes, lit on their long windward slopes and shadowed on their lee
    /// faces, with wind ripples over them, all fading to plain sand at a small zoom.
    #[test]
    fn sand_has_dunes() {
        let Some((device, queue)) = device() else {
            eprintln!("no graphics adapter; skipping");
            return;
        };
        let (marks, bare) = ground_texture(&device, &queue, Ground::Sand, 40.0);
        let (_, lighter, darker) = relief_spread(&marks, bare, Ground::Sand);
        assert!(
            lighter > 20 && darker > 20,
            "{lighter} lit, {darker} shadowed"
        );
        let (marks, _) = ground_texture(&device, &queue, Ground::Sand, 8.0);
        assert!(marks.is_empty(), "dunes at a small zoom");
    }

    /// Tundra carries patches of pale lichen and dark moss over lit tussocks, all fading to
    /// plain tundra at a small zoom.
    #[test]
    fn tundra_has_lichen_and_moss() {
        let Some((device, queue)) = device() else {
            eprintln!("no graphics adapter; skipping");
            return;
        };
        let tundra = color_bytes(ground_color(Ground::Tundra));
        let (marks, _) = ground_texture(&device, &queue, Ground::Tundra, 40.0);
        let lichen = marks
            .iter()
            .filter(|m| (0..3).all(|c| m[c] > tundra[c] + 10))
            .count();
        let moss = marks
            .iter()
            .filter(|m| (0..3).all(|c| m[c] < tundra[c] - 20))
            .count();
        assert!(lichen > 5 && moss > 5, "{lichen} lichen, {moss} moss");
        let (marks, _) = ground_texture(&device, &queue, Ground::Tundra, 8.0);
        assert!(marks.is_empty(), "tundra textured at a small zoom");
    }

    /// Pavement is laid as a concrete lot: slabs of slightly different shades, some lighter
    /// and some darker than the pavement's color, between darker joints, all fading to plain
    /// pavement at a small zoom.
    #[test]
    fn pavement_is_a_concrete_lot() {
        let Some((device, queue)) = device() else {
            eprintln!("no graphics adapter; skipping");
            return;
        };
        let (marks, bare) = ground_texture(&device, &queue, Ground::Pavement, 40.0);
        let (_, lighter, darker) = relief_spread(&marks, bare, Ground::Pavement);
        let pavement = color_bytes(ground_color(Ground::Pavement));
        let joints = marks
            .iter()
            .filter(|m| (0..3).all(|c| m[c] < pavement[c] - 20))
            .count();
        assert!(
            lighter > 20 && darker > 20 && joints > 5,
            "{lighter} lighter, {darker} darker, {joints} joint"
        );
        let (marks, _) = ground_texture(&device, &queue, Ground::Pavement, 8.0);
        assert!(marks.is_empty(), "pavement textured at a small zoom");
    }

    /// Swamp is a bog of murky pools, bluer than the swamp, among darker mud and reeds, all
    /// fading to plain swamp at a small zoom.
    #[test]
    fn swamp_is_a_bog_with_pools() {
        let Some((device, queue)) = device() else {
            eprintln!("no graphics adapter; skipping");
            return;
        };
        let swamp = color_bytes(ground_color(Ground::Swamp));
        let (marks, _) = ground_texture(&device, &queue, Ground::Swamp, 40.0);
        let pools = marks.iter().filter(|m| m[2] > swamp[2] + 10).count();
        let dark = marks
            .iter()
            .filter(|m| (0..3).all(|c| m[c] < swamp[c] - 15))
            .count();
        assert!(pools > 5 && dark > 5, "{pools} pool, {dark} mud or reed");
        let (marks, _) = ground_texture(&device, &queue, Ground::Swamp, 8.0);
        assert!(marks.is_empty(), "swamp textured at a small zoom");
    }

    /// Molten magma glows yellow toward the middle of its lava cells and darkens along the
    /// seams between them; magma crust is split by bright orange cracks. Both fade to their
    /// plain colors when the hexes are too small to show them.
    #[test]
    fn magma_glows() {
        let Some((device, queue)) = device() else {
            eprintln!("no graphics adapter; skipping");
            return;
        };
        let magma = color_bytes(ground_color(Ground::Magma));
        let (marks, _) = ground_texture(&device, &queue, Ground::Magma, 40.0);
        let hot = marks.iter().filter(|m| m[1] > magma[1] + 60).count();
        let cool = marks
            .iter()
            .filter(|m| (0..3).all(|c| m[c] < magma[c]))
            .count();
        assert!(hot > 10 && cool > 10, "{hot} hot, {cool} cool");
        let crust = color_bytes(ground_color(Ground::MagmaCrust));
        let (marks, _) = ground_texture(&device, &queue, Ground::MagmaCrust, 40.0);
        let cracks = marks.iter().filter(|m| m[0] > crust[0] + 80).count();
        assert!(cracks > 10, "{cracks} cracks");
        for ground in [Ground::Magma, Ground::MagmaCrust] {
            let (marks, _) = ground_texture(&device, &queue, ground, 8.0);
            assert!(marks.is_empty(), "{ground:?} textured at a small zoom");
        }
    }

    /// Heavy industrial ground is built up with structures lit from the upper left and casting
    /// shadows over a concrete yard, all fading to the plain ground color at a small zoom.
    #[test]
    fn heavy_industry_is_built_up() {
        let Some((device, queue)) = device() else {
            eprintln!("no graphics adapter; skipping");
            return;
        };
        let ground = Ground::HeavyIndustrial;
        let (marks, bare) = ground_texture(&device, &queue, ground, 40.0);
        let (_, lighter, darker) = relief_spread(&marks, bare, ground);
        assert!(
            lighter > 20 && darker > 20,
            "{lighter} lit, {darker} shadowed"
        );
        let (marks, _) = ground_texture(&device, &queue, ground, 8.0);
        assert!(marks.is_empty(), "industry drawn at a small zoom");
    }

    /// Fire draws as a flame icon, outlined in deep red and filled with the fire color, over
    /// scorched ground darker than the same ground unburned.
    #[test]
    fn fire_draws_a_flame_over_scorched_ground() {
        let Some((device, queue)) = device() else {
            eprintln!("no graphics adapter; skipping");
            return;
        };
        let fire = Hex::at_level(0).with_overlay(Some(DecorationKind::Fire));
        let pixel = lone_hex(&device, &queue, fire);
        let bare = lone_hex(&device, &queue, Hex::at_level(0));
        let ink = color_bytes(overlay_color(DecorationKind::Fire));
        let near =
            |bytes: [i32; 3], color: [i32; 3]| (0..3).all(|c| (bytes[c] - color[c]).abs() <= 2);
        let samples: Vec<_> = (-25..=25)
            .flat_map(|y| (-25..=25).map(move |x| (x as f32 * 0.02, y as f32 * 0.02)))
            .collect();
        let inked = samples
            .iter()
            .filter(|&&(dx, dy)| near(pixel(dx, dy), ink))
            .count();
        let outline = samples
            .iter()
            .filter(|&&(dx, dy)| {
                let p = pixel(dx, dy);
                p[0] > 120 && p[1] < 50 && p[2] < 30
            })
            .count();
        assert!(
            inked > 10 && outline > 5,
            "{inked} fire-colored, {outline} outline"
        );
        // Well clear of the flames, the ground is scorched darker than it would be unburned.
        for offset in [(-0.6, 0.0), (0.6, 0.0), (0.0, -0.6)] {
            let (burned, unburned) = (pixel(offset.0, offset.1), bare(offset.0, offset.1));
            assert!(
                (0..3).all(|c| burned[c] < unburned[c] - 8),
                "{offset:?}: {burned:?} is not scorched from {unburned:?}"
            );
        }
    }

    /// Smoke draws as bands streaming across the hex: lighter and darker than the smoke color,
    /// outlined in dark gray, with the terrain beyond them untouched.
    #[test]
    fn smoke_draws_streaming_bands() {
        let Some((device, queue)) = device() else {
            eprintln!("no graphics adapter; skipping");
            return;
        };
        let smoke = Hex::at_level(0).with_overlay(Some(DecorationKind::Smoke));
        let pixel = lone_hex(&device, &queue, smoke);
        let bare = lone_hex(&device, &queue, Hex::at_level(0));
        let ink = color_bytes(overlay_color(DecorationKind::Smoke));
        let (mut lit, mut shaded, mut outline) = (0, 0, 0);
        for y in -25..=25 {
            for x in -25..=25 {
                let p = pixel(x as f32 * 0.02, y as f32 * 0.02);
                let gray = (p[0] - p[2]).abs() < 20;
                lit += usize::from(gray && (0..3).all(|c| p[c] > ink[c] + 10));
                shaded += usize::from(gray && (0..3).all(|c| p[c] < ink[c] - 10 && p[c] > 100));
                outline += usize::from(p.iter().all(|&v| v < 90));
            }
        }
        assert!(
            lit > 10 && shaded > 10 && outline > 5,
            "{lit} lit, {shaded} shaded, {outline} outline"
        );
        for offset in [(-0.75, 0.0), (0.0, 0.7)] {
            assert_eq!(
                pixel(offset.0, offset.1),
                bare(offset.0, offset.1),
                "{offset:?}"
            );
        }
    }
}
