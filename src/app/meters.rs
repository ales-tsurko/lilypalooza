use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use bytemuck::{Pod, Zeroable};
use iced::widget::{shader, shader as shader_widget};
use iced::{Color, Element, Length, Rectangle, Theme};

use lilypalooza_audio::mixer::StripMeterSnapshot;

const METER_WIDTH: f32 = 14.0;
const METER_GAP: f32 = 2.0;
const CLIP_HEIGHT: f32 = 5.0;
const HOLD_HEIGHT: f32 = 2.0;
const HOT_ZONE_START: f32 = 0.82;

pub(super) fn stereo_meter<'a, Message: 'a>(
    snapshot: StripMeterSnapshot,
    colors: MeterColors,
    height: f32,
) -> Element<'a, Message> {
    shader_widget(MeterProgram::new(snapshot, colors))
        .width(Length::Fixed(METER_WIDTH * 2.0 + METER_GAP))
        .height(Length::Fixed(height.max(1.0)))
        .into()
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct MeterColors {
    pub(super) rail: Color,
    pub(super) fill: Color,
    pub(super) hot: Color,
    pub(super) hold: Color,
    pub(super) clip: Color,
}

pub(super) fn meter_colors(theme: &Theme) -> MeterColors {
    let palette = theme.extended_palette();

    MeterColors {
        rail: palette.background.weak.color,
        fill: palette.primary.base.color,
        hot: palette.success.base.color,
        hold: palette.primary.strong.color,
        clip: palette.danger.base.color,
    }
}

#[derive(Debug, Clone, Copy)]
struct MeterProgram {
    snapshot: StripMeterSnapshot,
    colors: MeterColors,
}

impl MeterProgram {
    fn new(snapshot: StripMeterSnapshot, colors: MeterColors) -> Self {
        Self { snapshot, colors }
    }
}

impl<Message> shader::Program<Message> for MeterProgram {
    type State = ();
    type Primitive = MeterPrimitive;

    fn draw(
        &self,
        _state: &Self::State,
        _cursor: iced::mouse::Cursor,
        bounds: Rectangle,
    ) -> Self::Primitive {
        MeterPrimitive::new(self.snapshot, self.colors, bounds)
    }
}

#[derive(Debug, Clone)]
struct MeterPrimitive {
    key: u64,
    vertices: Box<[MeterVertex]>,
}

impl MeterPrimitive {
    fn new(snapshot: StripMeterSnapshot, colors: MeterColors, bounds: Rectangle) -> Self {
        let mut vertices = Vec::new();
        let channel_width = (bounds.width - METER_GAP) * 0.5;
        let left_x = 0.0;
        let right_x = channel_width + METER_GAP;

        push_channel_meter(
            &mut vertices,
            left_x,
            channel_width,
            bounds.height,
            snapshot.left.level,
            snapshot.left.hold,
            colors,
            snapshot.clip_latched,
        );
        push_channel_meter(
            &mut vertices,
            right_x,
            channel_width,
            bounds.height,
            snapshot.right.level,
            snapshot.right.hold,
            colors,
            snapshot.clip_latched,
        );

        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for vertex in &vertices {
            for value in vertex.position {
                value.to_bits().hash(&mut hasher);
            }
            for value in vertex.color {
                value.to_bits().hash(&mut hasher);
            }
        }

        Self {
            key: hasher.finish(),
            vertices: vertices.into_boxed_slice(),
        }
    }
}

impl shader::Primitive for MeterPrimitive {
    type Pipeline = MeterPipeline;

    fn prepare(
        &self,
        pipeline: &mut Self::Pipeline,
        device: &iced::wgpu::Device,
        _queue: &iced::wgpu::Queue,
        _bounds: &Rectangle,
        _viewport: &shader::Viewport,
    ) {
        pipeline.prepare(device, self.key, &self.vertices);
    }

    fn draw(
        &self,
        pipeline: &Self::Pipeline,
        render_pass: &mut iced::wgpu::RenderPass<'_>,
    ) -> bool {
        pipeline.draw(self.key, render_pass)
    }
}

#[derive(Debug)]
struct MeterPipeline {
    pipeline: iced::wgpu::RenderPipeline,
    buffers: HashMap<u64, (iced::wgpu::Buffer, u32)>,
}

impl shader::Pipeline for MeterPipeline {
    fn new(
        device: &iced::wgpu::Device,
        _queue: &iced::wgpu::Queue,
        format: iced::wgpu::TextureFormat,
    ) -> Self {
        let shader = device.create_shader_module(iced::wgpu::ShaderModuleDescriptor {
            label: Some("lilypalooza meter shader"),
            source: iced::wgpu::ShaderSource::Wgsl(METER_SHADER.into()),
        });

        let pipeline_layout =
            device.create_pipeline_layout(&iced::wgpu::PipelineLayoutDescriptor {
                label: Some("lilypalooza meter pipeline layout"),
                bind_group_layouts: &[],
                push_constant_ranges: &[],
            });

        let pipeline = device.create_render_pipeline(&iced::wgpu::RenderPipelineDescriptor {
            label: Some("lilypalooza meter pipeline"),
            layout: Some(&pipeline_layout),
            vertex: iced::wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[iced::wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<MeterVertex>() as u64,
                    step_mode: iced::wgpu::VertexStepMode::Vertex,
                    attributes: &iced::wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4],
                }],
            },
            fragment: Some(iced::wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(iced::wgpu::ColorTargetState {
                    format,
                    blend: Some(iced::wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: iced::wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: iced::wgpu::PrimitiveState {
                topology: iced::wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: iced::wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: iced::wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: iced::wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            pipeline,
            buffers: HashMap::new(),
        }
    }
}

impl MeterPipeline {
    fn prepare(&mut self, device: &iced::wgpu::Device, key: u64, vertices: &[MeterVertex]) {
        use iced::wgpu::util::DeviceExt;

        let buffer = device.create_buffer_init(&iced::wgpu::util::BufferInitDescriptor {
            label: Some("lilypalooza meter vertices"),
            contents: bytemuck::cast_slice(vertices),
            usage: iced::wgpu::BufferUsages::VERTEX,
        });

        self.buffers.insert(key, (buffer, vertices.len() as u32));
    }

    fn draw(&self, key: u64, render_pass: &mut iced::wgpu::RenderPass<'_>) -> bool {
        let Some((buffer, vertex_count)) = self.buffers.get(&key) else {
            return false;
        };

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_vertex_buffer(0, buffer.slice(..));
        render_pass.draw(0..*vertex_count, 0..1);
        true
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct MeterVertex {
    position: [f32; 2],
    color: [f32; 4],
}

#[allow(clippy::too_many_arguments)]
fn push_channel_meter(
    vertices: &mut Vec<MeterVertex>,
    x: f32,
    width: f32,
    height: f32,
    level: f32,
    hold: f32,
    colors: MeterColors,
    clip_latched: bool,
) {
    push_rect(
        vertices,
        x,
        0.0,
        width,
        height,
        colors.rail,
        width * 2.0 + METER_GAP,
        height,
    );

    let hot_cutoff = height * (1.0 - HOT_ZONE_START);
    let level_height = height * level.clamp(0.0, 1.0);
    if level_height > 0.0 {
        let low_height = (height - hot_cutoff).min(level_height);
        if low_height > 0.0 {
            push_rect(
                vertices,
                x,
                height - low_height,
                width,
                low_height,
                colors.fill,
                width * 2.0 + METER_GAP,
                height,
            );
        }

        let hot_height = (level_height - low_height).max(0.0);
        if hot_height > 0.0 {
            push_rect(
                vertices,
                x,
                hot_cutoff - hot_height,
                width,
                hot_height,
                colors.hot,
                width * 2.0 + METER_GAP,
                height,
            );
        }
    }

    let hold_y = (height * (1.0 - hold.clamp(0.0, 1.0)) - HOLD_HEIGHT * 0.5)
        .clamp(0.0, height - HOLD_HEIGHT);
    push_rect(
        vertices,
        x,
        hold_y,
        width,
        HOLD_HEIGHT,
        colors.hold,
        width * 2.0 + METER_GAP,
        height,
    );

    if clip_latched {
        push_rect(
            vertices,
            x,
            0.0,
            width,
            CLIP_HEIGHT,
            colors.clip,
            width * 2.0 + METER_GAP,
            height,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn push_rect(
    vertices: &mut Vec<MeterVertex>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    color: Color,
    total_width: f32,
    total_height: f32,
) {
    let left = x / total_width * 2.0 - 1.0;
    let right = (x + width) / total_width * 2.0 - 1.0;
    let top = 1.0 - y / total_height * 2.0;
    let bottom = 1.0 - (y + height) / total_height * 2.0;
    let color = [color.r, color.g, color.b, color.a];

    vertices.extend_from_slice(&[
        MeterVertex {
            position: [left, top],
            color,
        },
        MeterVertex {
            position: [right, top],
            color,
        },
        MeterVertex {
            position: [left, bottom],
            color,
        },
        MeterVertex {
            position: [left, bottom],
            color,
        },
        MeterVertex {
            position: [right, top],
            color,
        },
        MeterVertex {
            position: [right, bottom],
            color,
        },
    ]);
}

const METER_SHADER: &str = r#"
struct VertexIn {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
};

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(input: VertexIn) -> VertexOut {
    var out: VertexOut;
    out.position = vec4<f32>(input.position, 0.0, 1.0);
    out.color = input.color;
    return out;
}

@fragment
fn fs_main(input: VertexOut) -> @location(0) vec4<f32> {
    return input.color;
}
"#;

#[cfg(test)]
mod tests {
    use super::{HOT_ZONE_START, METER_WIDTH, push_channel_meter};
    use lilypalooza_audio::mixer::{ChannelMeterSnapshot, StripMeterSnapshot};

    #[test]
    fn meter_geometry_grows_monotonically_with_level() {
        let colors = super::MeterColors {
            rail: iced::Color::BLACK,
            fill: iced::Color::WHITE,
            hot: iced::Color::WHITE,
            hold: iced::Color::WHITE,
            clip: iced::Color::WHITE,
        };

        let mut low = Vec::new();
        push_channel_meter(&mut low, 0.0, METER_WIDTH, 120.0, 0.2, 0.2, colors, false);
        let mut high = Vec::new();
        push_channel_meter(&mut high, 0.0, METER_WIDTH, 120.0, 0.8, 0.8, colors, false);

        assert!(high.len() >= low.len());
    }

    #[test]
    fn clip_flag_adds_clip_geometry() {
        let colors = super::MeterColors {
            rail: iced::Color::BLACK,
            fill: iced::Color::WHITE,
            hot: iced::Color::WHITE,
            hold: iced::Color::WHITE,
            clip: iced::Color::WHITE,
        };
        let mut without_clip = Vec::new();
        push_channel_meter(
            &mut without_clip,
            0.0,
            METER_WIDTH,
            120.0,
            0.9,
            0.9,
            colors,
            false,
        );
        let mut with_clip = Vec::new();
        push_channel_meter(
            &mut with_clip,
            0.0,
            METER_WIDTH,
            120.0,
            0.9,
            0.9,
            colors,
            true,
        );

        assert!(with_clip.len() > without_clip.len());
    }

    #[test]
    fn hot_zone_constant_stays_in_range() {
        assert!((0.0..1.0).contains(&HOT_ZONE_START));
    }

    #[test]
    fn strip_snapshot_type_is_usable() {
        let snapshot = StripMeterSnapshot {
            left: ChannelMeterSnapshot {
                level: 0.1,
                hold: 0.4,
            },
            right: ChannelMeterSnapshot {
                level: 0.2,
                hold: 0.5,
            },
            clip_latched: true,
        };

        assert!(snapshot.clip_latched);
        assert!(snapshot.right.level > snapshot.left.level);
    }
}
