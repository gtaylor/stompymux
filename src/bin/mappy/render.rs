//! GPU drawing of the hex map with a single fragment shader pass (`map.wgsl`).
//!
//! Each hex becomes one RGBA texel holding its layers (see [`hex_texel`]) in an integer
//! texture, and the shader composes its look from them. The texture follows the document's [`HexFeed`], so an edit uploads only the rows it
//! touched. The camera, brush and grid settings travel as a small uniform block every frame, so
//! panning and zooming cost the same however large the map is.

use iced::{
    Color, Rectangle, wgpu,
    widget::shader::{self, Viewport},
};
use stompymux_rs::{
    BattleDecorationKind, BattleGround, BattleHex, BattleStructure, BattleWoods, Terrain,
};

use crate::{
    document::{HexFeed, file_holds},
    map_view::terrain_color,
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
    /// Which value to label hexes with, a [`Label`] as a number, or negative for none.
    pub label: f32,
}

/// What is written on each hex when zoomed in. Ground level is the one elevation; every other
/// layer is an offset from it, signed to say which way: water depth as a negative number below
/// the surface (-2), which sits at ground level, and a bridge deck, building or wall as a
/// positive one above it (+2). A layer's own elevation is ground level plus its offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Label {
    /// Every layer in fixed rows: ground level on top, deck, building or wall height in the
    /// middle, water depth at the bottom. Ground level is left off bare level-zero hexes.
    #[default]
    All,
    /// Ground level, on hexes above level zero.
    Level,
    /// Water depth below the surface, on water and ice.
    Depth,
    /// Bridge deck, building or wall height above the ground or water.
    Height,
}

impl Label {
    pub const ALL: [Self; 4] = [Self::All, Self::Level, Self::Depth, Self::Height];

    /// Name shown on the label selector.
    pub fn name(self) -> &'static str {
        match self {
            Self::All => "All layers",
            Self::Level => "Level",
            Self::Depth => "Depth",
            Self::Height => "Height",
        }
    }

    /// What the numbers on each hex mean, for the toolbar.
    pub fn legend(self) -> &'static str {
        match self {
            Self::All => {
                "top: ground level · middle: +deck, building or wall height · \
                 bottom: -depth below the water"
            }
            Self::Level => "ground level",
            Self::Depth => "-depth below the water surface",
            Self::Height => "+deck, building or wall height above the ground or water",
        }
    }

    /// The smallest hex radius, in pixels, at which these labels are legible. All layers
    /// stacks three small rows, so it needs more room than one centered number.
    pub fn min_radius(self) -> f32 {
        match self {
            Self::All => 20.0,
            Self::Level | Self::Depth | Self::Height => 12.0,
        }
    }

    /// The value `map.wgsl` selects this label with.
    pub fn shader_value(self) -> f32 {
        self as u8 as f32
    }
}

impl Uniforms {
    /// The uniform block's bytes in WGSL layout: four `vec2<f32>` then four `f32`.
    fn to_bytes(self) -> Vec<u8> {
        [self.size, self.offset, self.map_size, self.hover]
            .into_iter()
            .flatten()
            .chain([self.radius, self.grid_gap, self.brush, self.label])
            .flat_map(f32::to_ne_bytes)
            .collect()
    }
}

/// Position of a terrain's color in the palette.
fn palette_index(terrain: Terrain) -> u8 {
    Terrain::ALL
        .iter()
        .position(|candidate| *candidate == terrain)
        .expect("every terrain is in Terrain::ALL") as u8
}

/// A hex's layers packed for `map.wgsl`:
///
/// - red: ground as a palette index (bits 0-3), woods (bits 4-5: none, light, heavy) and
///   overlay (bits 6-7: none, fire, smoke);
/// - green: ground level;
/// - blue: water depth plus one, or zero for none (bits 0-3), frozen (bit 4), and whether a
///   map file cannot store the hex (bit 7);
/// - alpha: structure (bits 6-7: none, building, wall, bridge) and its height or deck (bits 0-5).
fn hex_texel(hex: BattleHex, storable: bool) -> [u8; 4] {
    let ground = palette_index(match hex.ground() {
        BattleGround::Clear => Terrain::Grassland,
        BattleGround::Road => Terrain::Road,
        BattleGround::Rough => Terrain::Rough,
        BattleGround::Mountains => Terrain::Mountains,
        BattleGround::Snow => Terrain::Snow,
        BattleGround::Sand => Terrain::Sand,
    });
    let woods = match hex.woods() {
        None => 0,
        Some(BattleWoods::Light) => 1,
        Some(BattleWoods::Heavy) => 2,
    };
    let overlay = match hex.overlay() {
        None => 0,
        Some(BattleDecorationKind::Fire) => 1,
        Some(BattleDecorationKind::Smoke) => 2,
    };
    let water = hex.water().map_or(0, |water| {
        (water.depth.min(14) + 1) | if water.frozen { 1 << 4 } else { 0 }
    });
    let structure = match hex.structure() {
        None => 0,
        Some(BattleStructure::Building { height }) => (1 << 6) | height.min(63),
        Some(BattleStructure::Wall { height }) => (2 << 6) | height.min(63),
        Some(BattleStructure::Bridge { deck }) => (3 << 6) | deck.min(63),
    };
    [
        ground | woods << 4 | overlay << 6,
        hex.level(),
        water | if storable { 0 } else { 1 << 7 },
        structure,
    ]
}

/// `map.wgsl` names the palette entries it needs by their [`Terrain::ALL`] positions.
/// Palette bytes: each terrain's color, then the luminance above which labels are drawn in
/// black rather than white. Colors and the threshold are converted to linear when the target
/// applies sRGB encoding itself, so they match the swatches iced draws.
fn palette_bytes(srgb_target: bool) -> Vec<u8> {
    let channels = |color: Color| {
        if srgb_target {
            color.into_linear()
        } else {
            [color.r, color.g, color.b, color.a]
        }
    };
    let threshold = channels(Color::from_rgb(0.55, 0.55, 0.55))[0];
    Terrain::ALL
        .into_iter()
        .map(|terrain| channels(terrain_color(terrain)))
        .chain([[threshold, 0.0, 0.0, 0.0]])
        .flatten()
        .flat_map(f32::to_ne_bytes)
        .collect()
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
            label: 0.0,
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
            format: wgpu::TextureFormat::Rgba8Uint,
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
        // Neighboring hexes are usually identical, so reuse the last storability answer
        // rather than asking the shared memo for every hex.
        let mut last: Option<(BattleHex, bool)> = None;
        let texels: Vec<u8> = hexes[rows.start * width..rows.end * width]
            .iter()
            .flat_map(|&hex| {
                let storable = match last {
                    Some((previous, storable)) if previous == hex => storable,
                    _ => file_holds(hex),
                };
                last = Some((hex, storable));
                hex_texel(hex, storable)
            })
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
                bytes_per_row: Some(width as u32 * 4),
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

    /// Palette entries: one color per terrain, then the label ink threshold.
    const PALETTE_LEN: usize = Terrain::ALL.len() + 1;
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

    /// A frame of `document` at the test camera, with a grid and no labels or brush.
    fn frame(document: &Document, size: Size<u32>) -> MapPrimitive {
        labelled_frame(document, size, RADIUS, None)
    }

    /// A frame of `document` with hexes of `radius` pixels, labelled as `label` says.
    fn labelled_frame(
        document: &Document,
        size: Size<u32>,
        radius: f32,
        label: Option<Label>,
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
                label: label.map_or(-1.0, Label::shader_value),
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

    /// Paint the hex the compact notation describes at a coordinate, as one stroke.
    fn put(document: &mut Document, x: i32, y: i32, hex: BattleHex) {
        document.paint(BattleHexCoordinate { x, y }, Brush::matching(hex, 0));
        document.end_stroke();
    }

    /// Texels pack each layer where `map.wgsl` reads it, and the shader's palette positions
    /// match `Terrain::ALL`.
    #[test]
    fn texels_and_palette_match_the_shader() {
        for (terrain, position) in [
            (Terrain::LightForest, 2),
            (Terrain::HeavyForest, 3),
            (Terrain::Water, 4),
            (Terrain::Ice, 5),
            (Terrain::Bridge, 6),
            (Terrain::Fire, 9),
            (Terrain::Smoke, 10),
            (Terrain::Building, 12),
            (Terrain::Wall, 13),
        ] {
            assert_eq!(palette_index(terrain), position, "{terrain:?}");
        }
        assert_eq!(palette_bytes(false).len(), PALETTE_LEN * 16);
        let rough = palette_index(Terrain::Rough);
        let hex = BattleHex::new(Terrain::Rough, 30)
            .with_woods(Some(BattleWoods::Heavy))
            .with_overlay(Some(BattleDecorationKind::Smoke));
        assert_eq!(
            hex_texel(hex, false),
            [rough | 2 << 4 | 2 << 6, 30, 1 << 7, 0]
        );
        let ice = BattleHex::new(Terrain::Ice, 9).with_level(4);
        assert_eq!(hex_texel(ice, true), [0, 4, 10 | 1 << 4, 0]);
        let bridge = BattleHex::new(Terrain::Bridge, 3);
        assert_eq!(hex_texel(bridge, true)[3], 3 << 6 | 3);
        let wall = BattleHex::new(Terrain::Wall, 35);
        assert_eq!(hex_texel(wall, true)[3], 2 << 6 | 35);
    }

    /// Hexes render in their palette colors, higher ground is lighter, unsavable hexes are
    /// hatched, off-map pixels stay transparent, and an edit after the first upload reaches
    /// the screen through a partial upload. Skipped on machines without a graphics adapter.
    #[test]
    fn shader_draws_layers_and_follows_edits() {
        let Some((device, queue)) = device() else {
            eprintln!("no graphics adapter; skipping");
            return;
        };
        let size = Size::new(128, 128);
        let mut pipeline = MapPipeline::new(&device, &queue, FORMAT);
        let mut document = Document::new(3, 3).unwrap();
        put(&mut document, 0, 0, BattleHex::new(Terrain::Water, 0));
        put(&mut document, 1, 0, BattleHex::new(Terrain::Road, 0));
        put(&mut document, 2, 1, BattleHex::new(Terrain::HeavyForest, 0));
        put(&mut document, 1, 2, BattleHex::new(Terrain::Building, 4));
        put(&mut document, 2, 2, BattleHex::at_level(20));
        let woods_on_rough = BattleHex::new(Terrain::Rough, 0).with_woods(Some(BattleWoods::Light));
        put(&mut document, 2, 0, woods_on_rough);
        let pixels = render(
            &device,
            &queue,
            &mut pipeline,
            &frame(&document, size),
            size,
        );
        assert_color(&pixels, size, 0, 0, terrain_color(Terrain::Water));
        assert_color(&pixels, size, 1, 0, terrain_color(Terrain::Road));
        assert_color(&pixels, size, 2, 1, terrain_color(Terrain::HeavyForest));
        assert_color(&pixels, size, 1, 2, terrain_color(Terrain::Building));
        assert_color(&pixels, size, 0, 1, terrain_color(Terrain::Grassland));
        let low = center_pixel(&pixels, size, 0, 1);
        let high = center_pixel(&pixels, size, 2, 2);
        assert!(
            (0..3).all(|channel| high[channel] > low[channel]),
            "{high:?} vs {low:?}"
        );
        let hatched = (0..12).any(|step| {
            let x = OFFSET[0] + RADIUS * 4.0 + step as f32 - 6.0;
            let y = OFFSET[1] + RADIUS * 3.0_f32.sqrt();
            let index = ((y as u32 * size.width + x as u32) * 4) as usize;
            pixels[index] > 150 && pixels[index + 1] < 80
        });
        assert!(
            hatched,
            "woods on rough ground should be hatched as unsavable"
        );
        assert_eq!(pixels[..4], [0, 0, 0, 0]);
        assert_eq!(pixels[pixels.len() - 4..], [0, 0, 0, 0]);

        let applied = pipeline.hexes.as_ref().unwrap().applied;
        put(&mut document, 0, 1, BattleHex::new(Terrain::Snow, 0));
        let pixels = render(
            &device,
            &queue,
            &mut pipeline,
            &frame(&document, size),
            size,
        );
        assert_eq!(pipeline.hexes.as_ref().unwrap().applied, applied + 1);
        assert_color(&pixels, size, 0, 1, terrain_color(Terrain::Snow));
        assert_color(&pixels, size, 1, 0, terrain_color(Terrain::Road));
    }

    /// Hex radius and frame size for the label tests, large enough for small digits.
    const LABEL_RADIUS: f32 = 40.0;
    const LABEL_FRAME: Size<u32> = Size::new(128, 128);

    /// Whether `label` changes the pixel at `(dx, dy)` from the center of a lone `hex`, in hex
    /// radii, compared with the same hex drawn without labels. Returns a lookup for any offset.
    fn label_ink(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        hex: BattleHex,
        label: Label,
    ) -> impl Fn(f32, f32) -> bool {
        let (size, radius) = (LABEL_FRAME, LABEL_RADIUS);
        let mut document = Document::new(1, 1).unwrap();
        put(&mut document, 0, 0, hex);
        let mut pipeline = MapPipeline::new(device, queue, FORMAT);
        let plain = labelled_frame(&document, size, radius, None);
        let plain = render(device, queue, &mut pipeline, &plain, size);
        let labelled = labelled_frame(&document, size, radius, Some(label));
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

    /// Which of the three label rows (top, middle, bottom) `label` writes on a lone `hex`.
    fn inked_rows(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        hex: BattleHex,
        label: Label,
    ) -> [bool; 3] {
        let inked = label_ink(device, queue, hex, label);
        let steps =
            |from: f32, to: f32| (0..=20).map(move |step| from + (to - from) * step as f32 / 20.0);
        [-0.5, 0.0, 0.5].map(|row: f32| {
            steps(row - 0.2, row + 0.2).any(|dy| steps(-0.5, 0.5).any(|dx| inked(dx, dy)))
        })
    }

    /// All layers writes each layer in its own row and leaves bare level-zero ground blank;
    /// single-value modes write one larger number across the middle.
    #[test]
    fn all_layers_labels_use_fixed_rows() {
        let Some((device, queue)) = device() else {
            eprintln!("no graphics adapter; skipping");
            return;
        };
        let bridge = BattleHex::new(Terrain::Bridge, 2).with_level(4);
        let water = BattleHex::new(Terrain::Water, 3).with_level(2);
        let building = BattleHex::new(Terrain::Building, 3);
        let bare = BattleHex::at_level(0);
        let rows = |hex, label| inked_rows(&device, &queue, hex, label);
        assert_eq!(rows(bridge, Label::All), [true, true, true]);
        assert_eq!(rows(water, Label::All), [true, false, true]);
        assert_eq!(rows(building, Label::All), [true, true, false]);
        assert_eq!(rows(bare, Label::All), [false, false, false]);
        assert!(rows(bridge, Label::Height)[1]);
        assert_eq!(rows(bare, Label::Level), [false, false, false]);
    }

    /// Depth is written with a minus sign and a bridge deck, building or wall height with a plus
    /// sign, each to the left of its digit, and neither sign appears on a zero.
    #[test]
    fn depth_and_deck_labels_are_signed() {
        let Some((device, queue)) = device() else {
            eprintln!("no graphics adapter; skipping");
            return;
        };
        let inked = |hex, label, dx, dy| label_ink(&device, &queue, hex, label)(dx, dy);
        // A sign and one digit put the sign's bar about a quarter radius left of center; a
        // lone centered digit, anti-aliased edge included, stops short of 0.3. A plus also
        // has a vertical stroke just above that bar, which a minus lacks.
        let (bar, stem) = ((-0.3, 0.0), (-0.24, -0.08));
        let deep = BattleHex::new(Terrain::Water, 3);
        let still = BattleHex::new(Terrain::Water, 0);
        let bridge = BattleHex::new(Terrain::Bridge, 3).with_level(4);
        let flush = BattleHex::new(Terrain::Bridge, 0);
        let building = BattleHex::new(Terrain::Building, 3).with_level(5);
        assert!(inked(deep, Label::Depth, bar.0, bar.1));
        assert!(!inked(deep, Label::Depth, stem.0, stem.1));
        assert!(!inked(still, Label::Depth, bar.0, bar.1));
        assert!(inked(bridge, Label::Height, bar.0, bar.1));
        assert!(inked(bridge, Label::Height, stem.0, stem.1));
        assert!(!inked(flush, Label::Height, bar.0, bar.1));
        assert!(inked(building, Label::Height, stem.0, stem.1));
    }
}
