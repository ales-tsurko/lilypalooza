use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use bytemuck::{Pod, Zeroable};
use iced::widget::canvas::{self as canvas_widget, Path, Stroke, Text};
use iced::widget::{canvas, row, shader, shader as shader_widget};
use iced::{Color, Element, Length, Pixels, Rectangle, Renderer, Theme, alignment};

use lilypalooza_audio::mixer::{STRIP_METER_MAX_DB, STRIP_METER_MIN_DB, StripMeterSnapshot};

use super::controls::fader_rail_layout;

const METER_GAP: f32 = 1.0;
const FADER_HANDLE_VISUAL_WIDTH: f32 = 22.0;
const CHANNEL_WIDTH: f32 = (FADER_HANDLE_VISUAL_WIDTH - METER_GAP) * 0.5;
const METER_TOTAL_WIDTH: f32 = CHANNEL_WIDTH * 2.0 + METER_GAP;
const SCALE_WIDTH: f32 = 26.0;
const TICK_WIDTH: f32 = 5.0;
const SCALE_LABEL_MIN_GAP: f32 = 11.0;
const CHANNEL_INSET: f32 = 0.5;
const CLIP_HEIGHT: f32 = 5.0;
const HOLD_HEIGHT: f32 = 2.0;
const SCALE_DB_MARKS: [f32; 7] = [0.0, -6.0, -12.0, -18.0, -24.0, -36.0, -60.0];

pub(super) fn stereo_meter_width(with_scale: bool) -> f32 {
    if with_scale {
        METER_TOTAL_WIDTH + 1.0 + SCALE_WIDTH
    } else {
        METER_TOTAL_WIDTH
    }
}

pub(super) fn stereo_meter_bar_width() -> f32 {
    METER_TOTAL_WIDTH
}

pub(super) fn stereo_meter<'a, Message: 'a>(
    snapshot: StripMeterSnapshot,
    colors: MeterColors,
    height: f32,
) -> Element<'a, Message> {
    shader_widget(MeterProgram::new(snapshot, colors, height.max(1.0)))
        .width(Length::Fixed(METER_TOTAL_WIDTH))
        .height(Length::Fixed(height.max(1.0)))
        .into()
}

pub(super) fn stereo_meter_with_scale<'a, Message: 'a>(
    snapshot: StripMeterSnapshot,
    colors: MeterColors,
    height: f32,
) -> Element<'a, Message> {
    row![
        stereo_meter(snapshot, colors, height),
        canvas(MeterScale { colors })
            .width(Length::Fixed(SCALE_WIDTH))
            .height(Length::Fixed(height.max(1.0))),
    ]
    .spacing(1)
    .align_y(alignment::Vertical::Bottom)
    .into()
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct MeterColors {
    pub(super) rail: Color,
    pub(super) safe: Color,
    pub(super) warning: Color,
    pub(super) hot: Color,
    pub(super) hold: Color,
    pub(super) clip: Color,
    pub(super) scale_text: Color,
    pub(super) scale_tick: Color,
}

pub(super) fn meter_colors(theme: &Theme) -> MeterColors {
    let palette = theme.extended_palette();
    let safe = brighten_color(palette.success.base.color, 0.18);
    let hot = brighten_color(palette.danger.base.color, 0.12);
    let warning = mix_color(safe, hot, 0.45);

    MeterColors {
        rail: palette.background.weak.color,
        safe,
        warning,
        hot,
        hold: palette.background.base.text,
        clip: palette.danger.strong.color,
        scale_text: palette.background.strong.text,
        scale_tick: palette.background.strong.color,
    }
}

fn mix_color(a: Color, b: Color, amount: f32) -> Color {
    let t = amount.clamp(0.0, 1.0);

    Color {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: a.a + (b.a - a.a) * t,
    }
}

fn brighten_color(color: Color, amount: f32) -> Color {
    let t = amount.clamp(0.0, 1.0);
    Color {
        r: color.r + (1.0 - color.r) * t,
        g: color.g + (1.0 - color.g) * t,
        b: color.b + (1.0 - color.b) * t,
        a: color.a,
    }
}

fn meter_db_to_normalized(db: f32) -> f32 {
    ((db - STRIP_METER_MIN_DB) / (STRIP_METER_MAX_DB - STRIP_METER_MIN_DB)).clamp(0.0, 1.0)
}

#[cfg(test)]
fn meter_gradient_color(colors: MeterColors, normalized_level: f32) -> Color {
    let t = normalized_level.clamp(0.0, 1.0);
    if t < 0.72 {
        mix_color(colors.safe, colors.warning, t / 0.72)
    } else {
        mix_color(colors.warning, colors.hot, (t - 0.72) / 0.28)
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct MeterShaderData {
    left_level: f32,
    right_level: f32,
    left_hold: f32,
    right_hold: f32,
    rail_y: f32,
    rail_height: f32,
    hold_thickness: f32,
    clip_thickness: f32,
    clip_latched: f32,
    channel_inset: f32,
    _pad0: [f32; 2],
    rail_color: [f32; 4],
    safe_color: [f32; 4],
    warning_color: [f32; 4],
    hot_color: [f32; 4],
    hold_color: [f32; 4],
    clip_color: [f32; 4],
}

fn meter_shader_data(snapshot: StripMeterSnapshot, height: f32) -> MeterShaderData {
    let (rail_y, rail_height) = meter_rail_layout(height);

    MeterShaderData {
        left_level: snapshot.left.level,
        right_level: snapshot.right.level,
        left_hold: snapshot.left.hold,
        right_hold: snapshot.right.hold,
        rail_y: rail_y / height.max(1.0),
        rail_height: rail_height / height.max(1.0),
        hold_thickness: HOLD_HEIGHT / height.max(1.0),
        clip_thickness: CLIP_HEIGHT / height.max(1.0),
        clip_latched: if snapshot.clip_latched { 1.0 } else { 0.0 },
        channel_inset: CHANNEL_INSET / CHANNEL_WIDTH.max(1.0),
        _pad0: [0.0; 2],
        rail_color: [0.0; 4],
        safe_color: [0.0; 4],
        warning_color: [0.0; 4],
        hot_color: [0.0; 4],
        hold_color: [0.0; 4],
        clip_color: [0.0; 4],
    }
}

fn meter_shader_data_with_colors(
    snapshot: StripMeterSnapshot,
    colors: MeterColors,
    height: f32,
) -> MeterShaderData {
    let mut data = meter_shader_data(snapshot, height);
    data.rail_color = color_array(colors.rail);
    data.safe_color = color_array(colors.safe);
    data.warning_color = color_array(colors.warning);
    data.hot_color = color_array(colors.hot);
    data.hold_color = color_array(colors.hold);
    data.clip_color = color_array(colors.clip);
    data
}

fn color_array(color: Color) -> [f32; 4] {
    [color.r, color.g, color.b, color.a]
}

#[derive(Debug, Clone, Copy)]
struct MeterProgram {
    snapshot: StripMeterSnapshot,
    colors: MeterColors,
    height: f32,
}

impl MeterProgram {
    fn new(snapshot: StripMeterSnapshot, colors: MeterColors, height: f32) -> Self {
        Self {
            snapshot,
            colors,
            height,
        }
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
        MeterPrimitive::new(meter_shader_data_with_colors(
            self.snapshot,
            self.colors,
            self.height.min(bounds.height).max(1.0),
        ))
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct MeterVertex {
    position: [f32; 2],
    uv: [f32; 2],
    data: MeterShaderData,
}

#[derive(Debug, Clone)]
struct MeterPrimitive {
    key: u64,
    vertices: [MeterVertex; 6],
}

#[derive(Debug, Clone, Copy)]
struct MeterScale {
    colors: MeterColors,
}

impl MeterPrimitive {
    fn new(data: MeterShaderData) -> Self {
        let vertices = quad_vertices(data);
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for vertex in &vertices {
            for value in vertex.position {
                value.to_bits().hash(&mut hasher);
            }
            for value in vertex.uv {
                value.to_bits().hash(&mut hasher);
            }
            for value in bytemuck::bytes_of(&vertex.data) {
                value.hash(&mut hasher);
            }
        }

        Self {
            key: hasher.finish(),
            vertices,
        }
    }
}

fn quad_vertices(data: MeterShaderData) -> [MeterVertex; 6] {
    [
        meter_vertex([-1.0, 1.0], [0.0, 0.0], data),
        meter_vertex([1.0, 1.0], [1.0, 0.0], data),
        meter_vertex([-1.0, -1.0], [0.0, 1.0], data),
        meter_vertex([-1.0, -1.0], [0.0, 1.0], data),
        meter_vertex([1.0, 1.0], [1.0, 0.0], data),
        meter_vertex([1.0, -1.0], [1.0, 1.0], data),
    ]
}

fn meter_vertex(position: [f32; 2], uv: [f32; 2], data: MeterShaderData) -> MeterVertex {
    MeterVertex { position, uv, data }
}

impl<Message> canvas_widget::Program<Message> for MeterScale {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas_widget::Geometry> {
        let mut frame = canvas_widget::Frame::new(renderer, bounds.size());
        let tick_left = 0.0;
        let tick_right = TICK_WIDTH;
        let (rail_y, rail_height) = meter_rail_layout(bounds.height);

        let marks = visible_scale_marks(bounds.height);
        for (index, db) in marks.iter().copied().enumerate() {
            let y = rail_y + rail_height * (1.0 - meter_db_to_normalized(db));
            let tick = Path::line(
                iced::Point::new(tick_left, y),
                iced::Point::new(tick_right, y),
            );
            frame.stroke(
                &tick,
                Stroke::default()
                    .with_width(1.0)
                    .with_color(self.colors.scale_tick),
            );
            frame.fill_text(Text {
                content: format!("{:.0}", db.abs()),
                position: iced::Point::new(TICK_WIDTH + 3.0, y),
                color: self.colors.scale_text,
                size: Pixels(8.0),
                align_y: if index == 0 {
                    alignment::Vertical::Top
                } else if index == marks.len() - 1 {
                    alignment::Vertical::Bottom
                } else {
                    alignment::Vertical::Center
                },
                ..Text::default()
            });
        }

        vec![frame.into_geometry()]
    }
}

fn visible_scale_marks(height: f32) -> Vec<f32> {
    let (_, rail_height) = meter_rail_layout(height);
    let base_gap = rail_height * 0.1;
    let stride = if base_gap <= 0.0 {
        SCALE_DB_MARKS.len()
    } else {
        (SCALE_LABEL_MIN_GAP / base_gap).ceil().max(1.0) as usize
    };

    let mut marks = Vec::with_capacity(SCALE_DB_MARKS.len());
    for (index, db) in SCALE_DB_MARKS.iter().copied().enumerate() {
        if (index == 0 || index == SCALE_DB_MARKS.len() - 1 || index % stride == 0)
            && marks.last().copied() != Some(db)
        {
            marks.push(db);
        }
    }

    if marks.first().copied() != Some(SCALE_DB_MARKS[0]) {
        marks.insert(0, SCALE_DB_MARKS[0]);
    }
    let last_mark = SCALE_DB_MARKS[SCALE_DB_MARKS.len() - 1];
    if marks.last().copied() != Some(last_mark) {
        marks.push(last_mark);
    }

    marks
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
                    attributes: &iced::wgpu::vertex_attr_array![
                        0 => Float32x2,
                        1 => Float32x2,
                        2 => Float32x4,
                        3 => Float32x4,
                        4 => Float32x4,
                        5 => Float32x4,
                        6 => Float32x4,
                        7 => Float32x4,
                        8 => Float32x4,
                        9 => Float32x4,
                        10 => Float32x4
                    ],
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

    fn trim(&mut self) {
        self.buffers.clear();
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

fn meter_rail_layout(total_height: f32) -> (f32, f32) {
    fader_rail_layout(total_height)
}

const METER_SHADER: &str = r#"
struct VertexIn {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) levels: vec4<f32>,
    @location(3) rail_layout: vec4<f32>,
    @location(4) flags: vec4<f32>,
    @location(5) rail_color: vec4<f32>,
    @location(6) safe_color: vec4<f32>,
    @location(7) warning_color: vec4<f32>,
    @location(8) hot_color: vec4<f32>,
    @location(9) hold_color: vec4<f32>,
    @location(10) clip_color: vec4<f32>,
};

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) levels: vec4<f32>,
    @location(2) rail_layout: vec4<f32>,
    @location(3) flags: vec4<f32>,
    @location(4) rail_color: vec4<f32>,
    @location(5) safe_color: vec4<f32>,
    @location(6) warning_color: vec4<f32>,
    @location(7) hot_color: vec4<f32>,
    @location(8) hold_color: vec4<f32>,
    @location(9) clip_color: vec4<f32>,
};

@vertex
fn vs_main(input: VertexIn) -> VertexOut {
    var out: VertexOut;
    out.position = vec4<f32>(input.position, 0.0, 1.0);
    out.uv = input.uv;
    out.levels = input.levels;
    out.rail_layout = input.rail_layout;
    out.flags = input.flags;
    out.rail_color = input.rail_color;
    out.safe_color = input.safe_color;
    out.warning_color = input.warning_color;
    out.hot_color = input.hot_color;
    out.hold_color = input.hold_color;
    out.clip_color = input.clip_color;
    return out;
}

fn meter_gradient(safe: vec4<f32>, warning: vec4<f32>, hot: vec4<f32>, t: f32) -> vec4<f32> {
    if t < 0.72 {
        return mix(safe, warning, t / 0.72);
    }
    return mix(warning, hot, (t - 0.72) / 0.28);
}

@fragment
fn fs_main(input: VertexOut) -> @location(0) vec4<f32> {
    let gap = 1.0 / 22.0;
    let channel_width = (1.0 - gap) * 0.5;
    let is_left = input.uv.x < channel_width;
    let channel_start = select(channel_width + gap, 0.0, is_left);
    let local_x = (input.uv.x - channel_start) / channel_width;
    let rail_y = input.rail_layout.x;
    let rail_height = input.rail_layout.y;
    let hold_thickness = input.rail_layout.z;
    let clip_thickness = input.rail_layout.w;
    let clip_latched = input.flags.x;
    let channel_inset = input.flags.y;

    if input.uv.y < rail_y || input.uv.y > rail_y + rail_height {
        return vec4<f32>(0.0);
    }

    if local_x < channel_inset || local_x > 1.0 - channel_inset {
        return vec4<f32>(0.0);
    }

    let local_y = (input.uv.y - rail_y) / rail_height;
    let level = select(input.levels.y, input.levels.x, is_left);
    let hold = select(input.levels.w, input.levels.z, is_left);
    var color = input.rail_color;

    if local_y >= 1.0 - level {
        let gradient_t = clamp(1.0 - local_y, 0.0, 1.0);
        color = meter_gradient(input.safe_color, input.warning_color, input.hot_color, gradient_t);
    }

    if abs(local_y - (1.0 - hold)) <= hold_thickness * 0.5 / rail_height {
        color = vec4<f32>(input.hold_color.xyz, 1.0);
    }

    if clip_latched > 0.5 && local_y <= clip_thickness / rail_height {
        color = input.clip_color;
    }

    return color;
}
"#;

#[cfg(test)]
mod tests {
    use super::{
        FADER_HANDLE_VISUAL_WIDTH, METER_TOTAL_WIDTH, MeterColors, SCALE_DB_MARKS,
        SCALE_LABEL_MIN_GAP, brighten_color, meter_db_to_normalized, meter_gradient_color,
        meter_rail_layout, meter_shader_data, visible_scale_marks,
    };
    use crate::app::controls::fader_rail_layout;
    use lilypalooza_audio::mixer::{ChannelMeterSnapshot, StripMeterSnapshot};

    #[test]
    fn meter_shader_data_is_compact_and_preserves_values() {
        let snapshot = StripMeterSnapshot {
            left: ChannelMeterSnapshot {
                level: 0.25,
                hold: 0.5,
            },
            right: ChannelMeterSnapshot {
                level: 0.75,
                hold: 0.9,
            },
            clip_latched: true,
        };

        let data = meter_shader_data(snapshot, 120.0);

        assert_eq!(data.left_level, 0.25);
        assert_eq!(data.left_hold, 0.5);
        assert_eq!(data.right_level, 0.75);
        assert_eq!(data.right_hold, 0.9);
        assert_eq!(data.clip_latched, 1.0);
        assert!(data.rail_height > 0.0);
    }

    #[test]
    fn db_scale_normalizes_endpoints() {
        assert_eq!(meter_db_to_normalized(-60.0), 0.0);
        assert_eq!(meter_db_to_normalized(0.0), 1.0);
    }

    #[test]
    fn scale_marks_cover_the_60_db_range() {
        assert_eq!(SCALE_DB_MARKS.first().copied(), Some(0.0));
        assert_eq!(SCALE_DB_MARKS.last().copied(), Some(-60.0));
    }

    #[test]
    fn compact_heights_reduce_scale_marks() {
        let tall = visible_scale_marks(220.0);
        let short = visible_scale_marks(96.0);

        assert!(short.len() < tall.len());
        assert_eq!(short.first().copied(), Some(0.0));
        assert_eq!(short.last().copied(), Some(-60.0));
    }

    #[test]
    fn visible_scale_marks_keep_minimum_spacing() {
        let height = 96.0;
        let marks = visible_scale_marks(height);
        let (rail_y, rail_height) = meter_rail_layout(height);
        let positions: Vec<f32> = marks
            .into_iter()
            .map(|db| rail_y + rail_height * (1.0 - meter_db_to_normalized(db)))
            .collect();

        for pair in positions.windows(2) {
            assert!(pair[1] - pair[0] >= SCALE_LABEL_MIN_GAP - 0.001);
        }
    }

    #[test]
    fn meter_total_width_matches_fader_handle_width() {
        assert_eq!(METER_TOTAL_WIDTH, FADER_HANDLE_VISUAL_WIDTH);
    }

    #[test]
    fn meter_rail_matches_fader_rail_layout() {
        assert_eq!(meter_rail_layout(220.0), fader_rail_layout(220.0));
    }

    #[test]
    fn meter_geometry_stays_inside_meter_rail_height() {
        let height = 120.0;
        let (rail_y, rail_height) = meter_rail_layout(height);
        let rail_bottom = rail_y + rail_height;
        let data = meter_shader_data(StripMeterSnapshot::default(), height);

        assert!(data.rail_y >= rail_y / height - 0.001);
        assert!(data.rail_y + data.rail_height <= rail_bottom / height + 0.001);
    }

    #[test]
    fn gradient_moves_from_safe_to_hot() {
        let colors = MeterColors {
            rail: iced::Color::BLACK,
            safe: iced::Color::from_rgb(0.0, 1.0, 0.0),
            warning: iced::Color::from_rgb(1.0, 1.0, 0.0),
            hot: iced::Color::from_rgb(1.0, 0.0, 0.0),
            hold: iced::Color::WHITE,
            clip: iced::Color::WHITE,
            scale_text: iced::Color::WHITE,
            scale_tick: iced::Color::WHITE,
        };

        let cold = meter_gradient_color(colors, 0.1);
        let hot = meter_gradient_color(colors, 0.95);

        assert!(cold.g > cold.r);
        assert!(hot.r >= hot.g);
    }

    #[test]
    fn brighten_color_moves_toward_white() {
        let color = iced::Color::from_rgb(0.2, 0.4, 0.6);
        let brighter = brighten_color(color, 0.2);

        assert!(brighter.r > color.r);
        assert!(brighter.g > color.g);
        assert!(brighter.b > color.b);
        assert_eq!(brighter.a, color.a);
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
