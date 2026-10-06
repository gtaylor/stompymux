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

    /// The RGBA bytes at the center of a hex.
    fn center_pixel(pixels: &[u8], size: Size<u32>, x: i32, y: i32) -> [u8; 4] {
        let stagger = if x % 2 == 0 { 1.0 } else { 0.5 };
        let pixel_x = OFFSET[0] + RADIUS * (1.0 + 1.5 * x as f32);
        let pixel_y = OFFSET[1] + RADIUS * 3.0_f32.sqrt() * (y as f32 + stagger);
        let index = ((pixel_y as u32 * size.width + pixel_x as u32) * 4) as usize;
        pixels[index..index + 4].try_into().unwrap()
    }

    /// Assert that a hex's center is drawn in exactly `expected`.
    fn assert_color(pixels: &[u8], size: Size<u32>, x: i32, y: i32, expected: Color) {
        let actual = center_pixel(pixels, size, x, y);
        for (channel, value) in [expected.r, expected.g, expected.b].into_iter().enumerate() {
            let value = (value * 255.0).round() as i32;
            assert!(
                (i32::from(actual[channel]) - value).abs() <= 1,
                "{x},{y} channel {channel}: {actual:?} vs {value}"
            );
        }
        assert_eq!(actual[3], 255, "{x},{y}");
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
            (
                "PLANTED_FIELDS",
                position(&Foliage::ALL, Foliage::PlantedFields),
            ),
            ("RAIL", position(&Route::ALL, Route::Rail)),
            ("ICE", position(&Condition::ALL, Condition::Ice)),
            ("THIN_SNOW", position(&Condition::ALL, Condition::ThinSnow)),
            ("DEEP_SNOW", position(&Condition::ALL, Condition::DeepSnow)),
            ("MUD", position(&Condition::ALL, Condition::Mud)),
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
    /// ground is lighter, flowing water is streaked, off-map pixels stay transparent, and an
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
        put(&mut document, 1, 0, Hex::new(Terrain::Road, 0));
        put(&mut document, 2, 1, Hex::new(Terrain::HeavyWoods, 0));
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
        put(
            &mut document,
            1,
            1,
            Hex::at_level(0).with_ground(Ground::Magma),
        );
        let pixels = render(
            &device,
            &queue,
            &mut pipeline,
            &frame(&document, size),
            size,
        );
        assert_color(&pixels, size, 0, 0, water_color());
        assert_color(&pixels, size, 1, 0, route_color(Route::PavedRoad));
        assert_color(&pixels, size, 2, 1, foliage_color(Foliage::HeavyWoods));
        assert_color(
            &pixels,
            size,
            1,
            2,
            structure_base_color(StructureKind::Building),
        );
        assert_color(
            &pixels,
            size,
            2,
            0,
            crate::map_view::structure_color(StructureKind::Wall, ConstructionClass::Hardened),
        );
        assert_color(&pixels, size, 0, 1, ground_color(Ground::Clear));
        assert_color(&pixels, size, 1, 1, ground_color(Ground::Magma));
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
        let still = {
            let water = water_color();
            let shade = 1.0 - 0.08 * 2.0;
            Color::from_rgb(water.r * shade, water.g * shade, water.b * shade)
        };
        let streaked = center_pixel(&pixels, size, 0, 1);
        assert!(
            (0..3).all(|channel| f32::from(streaked[channel])
                > [still.r, still.g, still.b][channel] * 255.0 + 20.0),
            "torrent streak missing: {streaked:?}"
        );
        let snow = condition_color(Condition::DeepSnow);
        let clear = ground_color(Ground::Clear);
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
        assert_color(&pixels, size, 1, 0, route_color(Route::PavedRoad));
    }

    /// Hex radius and frame size for the label tests, large enough for the small row digits.
    const LABEL_RADIUS: f32 = 80.0;
    const LABEL_FRAME: Size<u32> = Size::new(256, 256);

    /// Whether labels change the pixel at `(dx, dy)` from the center of a lone `hex`, in hex
    /// radii, compared with the same hex drawn without them. Returns a lookup for any offset.
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
            plain[index..index + 3] != labelled[index..index + 3]
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
}
