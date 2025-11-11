use std::collections::{HashMap, HashSet};
use std::f64::consts::TAU;
use std::path::Path;

use bevy::asset::RenderAssetUsages;
use bevy::color::Mix;
use bevy::input::ButtonInput;
use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::mesh::Indices;
use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;
use bevy::sprite::Anchor;
use bevy::text::{Justify, LineBreak, TextBounds, TextLayout};
use bevy::window::PresentMode;
use bevy_egui::{EguiContexts, EguiPlugin, egui};
use glam::DVec2;
use image::ImageReader;
use tracing::{info, trace, warn};

use crate::loader::{DocumentSource, load_scene_from_env_or_demo};
use zcad_core::document::{
    Attribute, BlockDefinition, BlockReference, ClipMode, Document, Entity as DocEntity, HatchEdge,
    HatchLoop, ImageDictionaryEntry, RasterImage, RasterImageClip, RasterImageDefinition,
    RasterImageVariables, ThreeDFace, Wipeout,
};
use zcad_core::geometry::{Bounds2D, Point2, Vector2, Vector3};
use zcad_engine::command::{CommandBus, CommandContext, CommandRequest};
use zcad_engine::scene::{DemoEntities, Scene};

#[derive(Resource)]
struct SceneResource {
    scene: Scene,
    source: DocumentSource,
    demo_entities: Option<DemoEntities>,
    last_command_feedback: Option<String>,
}

#[derive(Resource, Clone)]
struct LoadedDocument(Document);

#[derive(Resource)]
struct CommandBusResource(CommandBus);

#[derive(Component)]
struct MainCamera;

#[derive(Resource, Default)]
struct PanState {
    is_dragging: bool,
}

#[derive(Component)]
struct SelectionHighlight;

#[derive(Resource, Clone)]
struct HighlightAssets {
    material: Handle<ColorMaterial>,
}

#[derive(Resource, Clone)]
struct TextAssets {
    font: Handle<Font>,
}

#[derive(Resource, Clone)]
struct RenderAssets {
    line_material: Handle<ColorMaterial>,
    hatch_material: Handle<ColorMaterial>,
}

#[derive(Resource, Default)]
struct RasterTextureCache {
    entries: HashMap<String, CachedRasterTexture>,
    fallback: Option<Handle<Image>>,
}

struct CachedRasterTexture {
    handle: Handle<Image>,
    usage_count: usize,
}

impl RasterTextureCache {
    fn get(&self, key: &str) -> Option<Handle<Image>> {
        self.entries.get(key).map(|entry| entry.handle.clone())
    }

    fn insert(&mut self, key: String, handle: Handle<Image>) {
        self.entries.insert(
            key,
            CachedRasterTexture {
                handle,
                usage_count: 1,
            },
        );
    }

    fn bump_usage(&mut self, key: &str) {
        if let Some(entry) = self.entries.get_mut(key) {
            entry.usage_count = entry.usage_count.saturating_add(1);
        }
    }

    fn retain_only(&mut self, keep: &HashSet<String>) {
        self.entries.retain(|key, _| keep.contains(key));
    }

    fn set_fallback(&mut self, handle: Handle<Image>) {
        self.fallback = Some(handle);
    }

    fn fallback(&self) -> Option<Handle<Image>> {
        self.fallback.clone()
    }
}

const RASTER_DEPTH: f32 = -0.1;
const FACE3D_DEPTH_BASE: f32 = -0.2;
const FACE3D_DEPTH_SCALE: f32 = -0.001;
const FACE3D_DEPTH_MIN: f32 = -0.95;
const FACE3D_DEPTH_MAX: f32 = -0.02;

pub fn launch(title: &str) {
    let loaded = load_scene_from_env_or_demo();
    let document_clone = loaded.scene.document().clone();

    App::new()
        .insert_resource(SceneResource {
            scene: loaded.scene,
            source: loaded.source,
            demo_entities: loaded.demo_entities,
            last_command_feedback: None,
        })
        .insert_resource(LoadedDocument(document_clone))
        .insert_resource(CommandBusResource(CommandBus::new()))
        .insert_resource(PanState::default())
        .insert_resource(RasterTextureCache::default())
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: title.into(),
                present_mode: PresentMode::AutoVsync,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(EguiPlugin::default())
        .add_systems(Startup, setup_camera)
        .add_systems(Startup, setup_highlight_assets)
        .add_systems(Startup, setup_render_assets)
        .add_systems(Startup, setup_text_assets)
        .add_systems(Startup, spawn_document_entities)
        .add_systems(Update, handle_keyboard_commands)
        .add_systems(Update, handle_zoom)
        .add_systems(Update, handle_pan)
        .add_systems(Update, egui_overlay)
        .add_systems(Update, update_selection_highlight)
        .run();
}

fn setup_camera(mut commands: Commands, doc: Res<LoadedDocument>) {
    let mut transform = Transform::from_xyz(0.0, 0.0, 999.9);
    let mut projection = OrthographicProjection::default_2d();
    if let Some(bounds) = doc.0.bounds() {
        let min = bounds.min();
        let max = bounds.max();
        let width = (max.x() - min.x()) as f32;
        let height = (max.y() - min.y()) as f32;
        let viewport_height = (height.max(width).max(10.0)) * 1.2;
        transform.translation.x = bounds.center().x() as f32;
        transform.translation.y = bounds.center().y() as f32;
        projection.scaling_mode = bevy::camera::ScalingMode::FixedVertical { viewport_height };
    }
    commands.spawn((
        Camera2d,
        MainCamera,
        Projection::Orthographic(projection),
        transform,
        GlobalTransform::default(),
    ));
}

fn setup_text_assets(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font = asset_server.load("fonts/FiraSans-Bold.ttf");
    commands.insert_resource(TextAssets { font });
}

fn setup_render_assets(
    mut commands: Commands,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut texture_cache: ResMut<RasterTextureCache>,
) {
    let line_material = materials.add(ColorMaterial::from(Color::WHITE));
    let hatch_material = materials.add(ColorMaterial::from(Color::WHITE));
    let fallback_image = create_placeholder_texture(&mut images);
    texture_cache.set_fallback(fallback_image.clone());
    commands.insert_resource(RenderAssets {
        line_material,
        hatch_material,
    });
}

fn spawn_document_entities(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut color_materials: ResMut<Assets<ColorMaterial>>,
    mut image_assets: ResMut<Assets<Image>>,
    mut texture_cache: ResMut<RasterTextureCache>,
    text_assets: Res<TextAssets>,
    render_assets: Res<RenderAssets>,
    doc: Res<LoadedDocument>,
) {
    let mut used_texture_keys: HashSet<String> = HashSet::new();
    for (_, entity) in doc.0.entities() {
        match entity {
            DocEntity::Hatch(hatch) => {
                spawn_hatch_fill(&mut commands, &mut meshes, &render_assets, hatch);
                for polyline in hatch_edge_polylines(hatch) {
                    for segment in polyline.windows(2) {
                        if let [start, end] = segment {
                            let _ = spawn_line_segment(
                                &mut commands,
                                &mut meshes,
                                render_assets.line_material.clone(),
                                *start,
                                *end,
                                0.0,
                            );
                        }
                    }
                }
                continue;
            }
            DocEntity::Text(text) => {
                spawn_single_line_text(
                    &mut commands,
                    &text_assets,
                    &text.content,
                    text.insert,
                    text.height,
                    text.rotation,
                    Anchor::BOTTOM_LEFT,
                );
                continue;
            }
            DocEntity::MText(mtext) => {
                spawn_multiline_text(
                    &mut commands,
                    &text_assets,
                    sanitize_mtext_content(&mtext.content),
                    mtext.insert,
                    mtext.height,
                    direction_to_angle(mtext.direction),
                    mtext.reference_width,
                    Anchor::BOTTOM_LEFT,
                );
                continue;
            }
            DocEntity::BlockReference(reference) => {
                spawn_block_reference(
                    &mut commands,
                    &mut meshes,
                    &mut color_materials,
                    &mut image_assets,
                    &mut texture_cache,
                    &mut used_texture_keys,
                    &render_assets,
                    &text_assets,
                    &doc.0,
                    reference,
                );
                continue;
            }
            DocEntity::MLeader(mleader) => {
                if let zcad_core::document::MLeaderContent::Block { block } = &mleader.content {
                    spawn_mleader_block_content(
                        &mut commands,
                        &mut meshes,
                        &mut color_materials,
                        &mut image_assets,
                        &mut texture_cache,
                        &mut used_texture_keys,
                        &render_assets,
                        &text_assets,
                        &doc.0,
                        &mleader.layer,
                        block,
                    );
                }
            }
            DocEntity::RasterImage(image) => {
                spawn_raster_image(
                    &mut commands,
                    &mut meshes,
                    &mut color_materials,
                    &mut image_assets,
                    &mut texture_cache,
                    &mut used_texture_keys,
                    render_assets.line_material.clone(),
                    &doc.0,
                    image,
                );
                continue;
            }
            DocEntity::Face3D(face) => {
                spawn_face3d(&mut commands, &mut meshes, &render_assets, face);
                continue;
            }
            _ => {}
        }
        for polyline in entity_polylines(entity) {
            for segment in polyline.windows(2) {
                if let [start, end] = segment {
                    let _ = spawn_line_segment(
                        &mut commands,
                        &mut meshes,
                        render_assets.line_material.clone(),
                        *start,
                        *end,
                        0.0,
                    );
                }
            }
        }
    }

    texture_cache.retain_only(&used_texture_keys);
}

fn egui_overlay(
    mut contexts: EguiContexts,
    scene_res: Res<SceneResource>,
    doc: Res<LoadedDocument>,
    command_bus: Res<CommandBusResource>,
) {
    let doc_ref = &doc.0;
    let layer_count = doc_ref.layers().count();
    let entity_count = doc_ref.entities().count();
    let block_count = doc_ref.blocks().count();
    let selection_len = scene_res.scene.selection_len();
    let source_label = match &scene_res.source {
        DocumentSource::Dxf(path) => format!("DXF: {}", path.display()),
        DocumentSource::Demo => "内置示例".to_string(),
    };

    let commands: Vec<&str> = command_bus.0.available_commands().copied().collect();

    if let Ok(ctx) = contexts.ctx_mut() {
        egui::Window::new("ZCAD Rust 原型").show(ctx, |ui| {
            ui.label(format!("文档来源：{source_label}"));
            ui.label(format!("图层数：{layer_count}"));
            ui.label(format!("实体数：{entity_count}"));
            ui.label(format!("块定义：{block_count}"));
            ui.label(format!("当前选中实体数：{selection_len}"));
            if !commands.is_empty() {
                ui.separator();
                ui.label(format!("可用命令：{}", commands.join(", ")));
            }
            if let Some(feedback) = &scene_res.last_command_feedback {
                ui.label(format!("最近命令：{feedback}"));
            }
            if let Some(demo) = &scene_res.demo_entities {
                ui.separator();
                ui.label("演示实体 ID：");
                ui.monospace(format!(
                    " baseline={} circle={} arc={} polyline={} label={} ",
                    demo.baseline.get(),
                    demo.circle.get(),
                    demo.arc.get(),
                    demo.polyline.get(),
                    demo.label.get()
                ));
            }
        });
    }
}

fn spawn_line_segment(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    material: Handle<ColorMaterial>,
    start: Point2,
    end: Point2,
    depth: f32,
) -> Entity {
    let mut mesh = Mesh::new(PrimitiveTopology::LineList, RenderAssetUsages::RENDER_WORLD);
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vec![
            [start.x() as f32, start.y() as f32, 0.0],
            [end.x() as f32, end.y() as f32, 0.0],
        ],
    );
    let handle = meshes.add(mesh);
    let transform = Transform::from_xyz(0.0, 0.0, depth);
    commands
        .spawn((
            Mesh2d(handle),
            MeshMaterial2d(material),
            transform,
            GlobalTransform::default(),
            Visibility::default(),
            InheritedVisibility::default(),
        ))
        .id()
}

fn spawn_polyline_segments(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    material: Handle<ColorMaterial>,
    points: &[Point2],
) {
    for segment in points.windows(2) {
        if let [start, end] = segment {
            let _ = spawn_line_segment(commands, meshes, material.clone(), *start, *end, 0.0);
        }
    }
}

fn spawn_single_line_text(
    commands: &mut Commands,
    assets: &TextAssets,
    content: &str,
    position: Point2,
    height: f64,
    rotation: f64,
    anchor: Anchor,
) {
    let mut transform = Transform::from_xyz(position.x() as f32, position.y() as f32, 1.0);
    transform.rotation = Quat::from_rotation_z(rotation as f32);
    commands.spawn((
        Text2d::new(content.to_string()),
        TextFont {
            font: assets.font.clone(),
            font_size: text_height_to_font_size(height),
            ..default()
        },
        TextColor(Color::WHITE),
        anchor,
        transform,
        GlobalTransform::default(),
        Visibility::default(),
        InheritedVisibility::default(),
    ));
}

fn spawn_multiline_text(
    commands: &mut Commands,
    assets: &TextAssets,
    content: String,
    position: Point2,
    height: f64,
    rotation: f64,
    reference_width: Option<f64>,
    anchor: Anchor,
) {
    let mut transform = Transform::from_xyz(position.x() as f32, position.y() as f32, 1.2);
    transform.rotation = Quat::from_rotation_z(rotation as f32);
    let mut entity = commands.spawn((
        Text2d::new(content),
        TextFont {
            font: assets.font.clone(),
            font_size: text_height_to_font_size(height),
            ..default()
        },
        TextColor(Color::WHITE),
        anchor,
        transform,
        GlobalTransform::default(),
        Visibility::default(),
        InheritedVisibility::default(),
    ));

    if let Some(width) = reference_width {
        entity.insert(TextBounds::from(Vec2::new(width as f32, f32::MAX)));
        entity.insert(TextLayout::new(Justify::Left, LineBreak::WordBoundary));
    }
}

fn sanitize_mtext_content(content: &str) -> String {
    content
        .replace("\\P", "\n")
        .replace("\\p", "\n")
        .replace("\\N", "\n")
        .replace("\\n", "\n")
}

fn text_height_to_font_size(height: f64) -> f32 {
    // CAD 文本高度以绘图单位表示，这里采用线性缩放便于预览。
    (height.max(0.1) as f32) * 8.0
}

fn direction_to_angle(direction: Vector2) -> f64 {
    let vec = direction.as_vec2();
    if vec.length_squared() > 1e-12 {
        vec.y.atan2(vec.x)
    } else {
        0.0
    }
}

struct GradientSpec {
    direction: Vec2,
    start: Color,
    end: Color,
    shift: f32,
}

fn gradient_spec(gradient: Option<&zcad_core::document::HatchGradient>) -> GradientSpec {
    let (direction, color_start, color_end, shift) = gradient_direction(gradient);
    GradientSpec {
        direction,
        start: color_start,
        end: color_end,
        shift,
    }
}

fn rotate_vec2(vec: Vec2, angle: f64) -> Vec2 {
    let (sin, cos) = angle.sin_cos();
    let sin = sin as f32;
    let cos = cos as f32;
    Vec2::new(vec.x * cos - vec.y * sin, vec.x * sin + vec.y * cos)
}

fn spawn_filled_polylines(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    render_assets: &RenderAssets,
    loops: &[Vec<Point2>],
    gradient: GradientSpec,
    depth: f32,
) {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut base_index: u32 = 0;

    for polygon in loops {
        if polygon.len() < 3 {
            continue;
        }
        let mut vertices = polygon.clone();
        if points_close(vertices[0], *vertices.last().unwrap()) {
            vertices.pop();
        }
        if vertices.len() < 3 {
            continue;
        }

        let projections: Vec<f32> = vertices
            .iter()
            .map(|p| Vec2::new(p.x() as f32, p.y() as f32).dot(gradient.direction))
            .collect();
        let min_proj = projections
            .iter()
            .fold(f32::INFINITY, |min, value| min.min(*value));
        let max_proj = projections
            .iter()
            .fold(f32::NEG_INFINITY, |max, value| max.max(*value));
        let range = (max_proj - min_proj).max(1e-6);

        for window in 1..vertices.len() - 1 {
            let triangle = [vertices[0], vertices[window], vertices[window + 1]];
            for point in triangle {
                positions.push([point.x() as f32, point.y() as f32, 0.0]);
                let projection =
                    Vec2::new(point.x() as f32, point.y() as f32).dot(gradient.direction);
                let mut t = ((projection - min_proj) / range).clamp(0.0, 1.0);
                t = apply_gradient_shift(t, gradient.shift);
                let color = gradient.start.mix(&gradient.end, t);
                colors.push(color_to_rgba(color));
            }
            indices.extend_from_slice(&[base_index, base_index + 1, base_index + 2]);
            base_index += 3;
        }
    }

    if positions.is_empty() {
        return;
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));

    let mesh_handle = meshes.add(mesh);
    commands.spawn((
        Mesh2d(mesh_handle),
        MeshMaterial2d(render_assets.hatch_material.clone()),
        Transform::from_xyz(0.0, 0.0, depth),
        GlobalTransform::default(),
        Visibility::default(),
        InheritedVisibility::default(),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotate_vec2_rotates_by_angle() {
        let dir = Vec2::new(1.0, 0.0);
        let rotated = rotate_vec2(dir, std::f64::consts::FRAC_PI_2);
        assert!(rotated.x.abs() < 1e-6);
        assert!((rotated.y - 1.0).abs() < 1e-6);
    }

    #[test]
    fn attribute_anchor_maps_codes() {
        assert!(matches!(attribute_anchor(0, 0), Anchor::BOTTOM_LEFT));
        assert!(matches!(attribute_anchor(1, 0), Anchor::BOTTOM_CENTER));
        assert!(matches!(attribute_anchor(2, 2), Anchor::TOP_RIGHT));
    }

    #[test]
    fn gradient_color_to_bevy_supports_true_color() {
        let color = super::gradient_color_to_bevy(Some(0xFF3366)).expect("颜色转换失败");
        let [r, g, b, a] = super::color_to_rgba(color);
        assert!((r - 1.0).abs() < 1e-6, "红色分量异常: {r}");
        assert!((g - 0.2).abs() < 1e-6, "绿色分量异常: {g}");
        assert!((b - 0.4).abs() < 1e-6, "蓝色分量异常: {b}");
        assert!((a - 0.85).abs() < 1e-6, "透明度异常: {a}");
    }

    #[test]
    fn apply_gradient_shift_wraps_into_unit_interval() {
        let shifted = super::apply_gradient_shift(0.9, 0.3);
        assert!((shifted - 0.2).abs() < 1e-6, "位移未正确回绕: {shifted}");
        let negative = super::apply_gradient_shift(0.1, -0.4);
        assert!(
            (negative - 0.7).abs() < 1e-6,
            "负位移未正确回绕: {negative}"
        );
    }

    #[test]
    fn gradient_direction_clamps_shift_and_preserves_single_color() {
        let gradient = zcad_core::document::HatchGradient {
            name: "LINEAR".to_string(),
            angle: std::f64::consts::FRAC_PI_4,
            shift: Some(1.75),
            tint: None,
            is_single_color: true,
            color1: Some(0xFF8844),
            color2: Some(0x00FF00),
        };
        let (direction, start, end, shift) = super::gradient_direction(Some(&gradient));
        let expected = std::f32::consts::FRAC_1_SQRT_2;
        assert!(
            (direction.x - expected).abs() < 1e-6 && (direction.y - expected).abs() < 1e-6,
            "梯度方向未归一化: ({}, {})",
            direction.x,
            direction.y
        );
        assert!(
            (shift - 1.0).abs() < 1e-6,
            "Shift 未被限制在 [-1, 1] 区间: {shift}"
        );
        let start_rgba = super::color_to_rgba(start);
        let end_rgba = super::color_to_rgba(end);
        for idx in 0..4 {
            assert!(
                (start_rgba[idx] - end_rgba[idx]).abs() < 1e-6,
                "单色渐变的起止颜色应相同，通道 {idx} 出现差异"
            );
        }
    }

    #[test]
    fn gradient_direction_handles_negative_shift() {
        let gradient = zcad_core::document::HatchGradient {
            name: "LINEAR".to_string(),
            angle: 0.0,
            shift: Some(-2.4),
            tint: None,
            is_single_color: false,
            color1: Some(1),
            color2: Some(5),
        };
        let (direction, start, end, shift) = super::gradient_direction(Some(&gradient));
        assert!(
            direction.y.abs() < 1e-6 && (direction.x - 1.0).abs() < 1e-6,
            "零角度应返回 X 轴方向: ({}, {})",
            direction.x,
            direction.y
        );
        assert!(
            (shift + 1.0).abs() < 1e-6,
            "负 shift 未被限制在 [-1, 1] 区间: {shift}"
        );
        let start_rgba = super::color_to_rgba(start);
        let end_rgba = super::color_to_rgba(end);
        assert!(
            (start_rgba[0] - 1.0).abs() < 1e-6,
            "ACI=1 期望红色通道为 1，实际为 {}",
            start_rgba[0]
        );
        assert!(
            (end_rgba[2] - 1.0).abs() < 1e-6,
            "ACI=5 期望蓝色通道为 1，实际为 {}",
            end_rgba[2]
        );
    }

    #[test]
    fn face3d_outline_points_handles_triangles() {
        use zcad_core::geometry::Point3;

        let face = zcad_core::document::ThreeDFace {
            layer: "MESH".to_string(),
            vertices: [
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(10.0, 0.0, 0.0),
                Point3::new(0.0, 5.0, 0.0),
                Point3::new(0.0, 5.0, 0.0),
            ],
            invisible_edges: [false; 4],
        };
        let vertices_xy: [Point2; 4] = face
            .vertices
            .map(|vertex| Point2::new(vertex.x(), vertex.y()));
        let outline = super::face3d_outline_points(&vertices_xy);
        assert_eq!(outline.len(), 4, "三角形面应包含闭合点");
        assert!(super::points_close(outline[0], *outline.last().unwrap()));
    }

    #[test]
    fn face3d_gradient_is_solid_color_with_alpha() {
        use zcad_core::geometry::Point3;

        let face = ThreeDFace {
            layer: "MESH".to_string(),
            vertices: [
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(4.0, 0.0, 0.0),
                Point3::new(0.0, 3.0, 0.0),
                Point3::new(0.0, 3.0, 0.0),
            ],
            invisible_edges: [false; 4],
        };
        let gradient = super::face3d_solid_gradient(&face);
        assert_eq!(gradient.start, gradient.end, "3DFACE 填充应为纯色");
        let rgba = super::color_to_rgba(gradient.start);
        assert!(rgba[3] > 0.4, "填充的透明度应大于 0.4，当前为 {}", rgba[3]);
    }

    #[test]
    fn face3d_shading_varies_with_orientation() {
        use zcad_core::geometry::Point3;

        let upward = ThreeDFace {
            layer: String::new(),
            vertices: [
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(5.0, 0.0, 0.0),
                Point3::new(0.0, 2.0, 0.0),
                Point3::new(0.0, 2.0, 0.0),
            ],
            invisible_edges: [false; 4],
        };
        let downward = ThreeDFace {
            vertices: [
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(0.0, 2.0, 0.0),
                Point3::new(5.0, 0.0, 0.0),
                Point3::new(5.0, 0.0, 0.0),
            ],
            ..upward.clone()
        };
        let up_rgba = super::color_to_rgba(super::face3d_shaded_color(&upward));
        let down_rgba = super::color_to_rgba(super::face3d_shaded_color(&downward));
        assert!(
            up_rgba[0] > down_rgba[0],
            "朝向光源的面应更亮，红色通道 {} <= {}",
            up_rgba[0],
            down_rgba[0]
        );
    }

    #[test]
    fn face3d_depth_varies_with_height() {
        use zcad_core::geometry::Point3;

        let low = ThreeDFace {
            layer: String::new(),
            vertices: [
                Point3::new(0.0, 0.0, -10.0),
                Point3::new(1.0, 0.0, -10.0),
                Point3::new(0.0, 1.0, -10.0),
                Point3::new(0.0, 1.0, -10.0),
            ],
            invisible_edges: [false; 4],
        };
        let high = ThreeDFace {
            vertices: [
                Point3::new(0.0, 0.0, 50.0),
                Point3::new(1.0, 0.0, 50.0),
                Point3::new(0.0, 1.0, 50.0),
                Point3::new(0.0, 1.0, 50.0),
            ],
            ..low.clone()
        };

        let low_depth = super::face3d_depth(&low);
        let high_depth = super::face3d_depth(&high);
        assert!(
            low_depth < high_depth,
            "低高度应位于更靠后的深度: low={low_depth}, high={high_depth}"
        );
        assert!(
            (low_depth - FACE3D_DEPTH_MIN).abs() < 0.1
                || (high_depth - FACE3D_DEPTH_MAX).abs() < 0.1,
            "深度应被限制在预设区间"
        );
    }
}

fn spawn_block_reference(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    color_materials: &mut Assets<ColorMaterial>,
    image_assets: &mut Assets<Image>,
    texture_cache: &mut RasterTextureCache,
    used_texture_keys: &mut HashSet<String>,
    render_assets: &RenderAssets,
    text_assets: &TextAssets,
    document: &Document,
    reference: &BlockReference,
) {
    if let Some(block) = document.block(&reference.name) {
        spawn_block_definition(
            commands,
            meshes,
            color_materials,
            image_assets,
            texture_cache,
            used_texture_keys,
            render_assets,
            text_assets,
            document,
            reference,
            block,
        );
    } else {
        warn!(block = %reference.name, "块定义缺失，无法绘制");
    }
}

fn spawn_mleader_block_content(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    color_materials: &mut Assets<ColorMaterial>,
    image_assets: &mut Assets<Image>,
    texture_cache: &mut RasterTextureCache,
    used_texture_keys: &mut HashSet<String>,
    render_assets: &RenderAssets,
    text_assets: &TextAssets,
    document: &Document,
    mleader_layer: &str,
    content: &zcad_core::document::MLeaderBlockContent,
) {
    let block_name = content.block_name.clone().or_else(|| {
        content
            .block_handle
            .as_deref()
            .and_then(|handle| document.block_name_by_handle(handle))
            .map(|name| name.to_string())
    });

    let Some(name) = block_name else {
        warn!(
            handle = ?content.block_handle,
            "MLeader 块内容缺少块名称，无法渲染"
        );
        return;
    };

    if content.connection_type.is_some() && content.connection_type != Some(0) {
        trace!(
            connection = content.connection_type,
            "暂未处理的 MLeader 块连接类型，仍按插入点渲染"
        );
    }

    let reference = BlockReference {
        name,
        insert: content.location,
        scale: Vector2::new(content.scale.x(), content.scale.y()),
        rotation: content.rotation,
        attributes: Vec::new(),
        layer: mleader_layer.to_string(),
    };

    spawn_block_reference(
        commands,
        meshes,
        color_materials,
        image_assets,
        texture_cache,
        used_texture_keys,
        render_assets,
        text_assets,
        document,
        &reference,
    );
}

fn spawn_block_definition(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    color_materials: &mut Assets<ColorMaterial>,
    image_assets: &mut Assets<Image>,
    texture_cache: &mut RasterTextureCache,
    used_texture_keys: &mut HashSet<String>,
    render_assets: &RenderAssets,
    text_assets: &TextAssets,
    document: &Document,
    reference: &BlockReference,
    block: &BlockDefinition,
) {
    for entity in &block.entities {
        spawn_block_entity(
            commands,
            meshes,
            color_materials,
            image_assets,
            texture_cache,
            used_texture_keys,
            render_assets,
            text_assets,
            document,
            reference,
            block.base_point,
            entity,
        );
    }

    for attribute in &reference.attributes {
        spawn_block_attribute(
            commands,
            text_assets,
            reference,
            block.base_point,
            attribute,
        );
    }
}

fn spawn_block_entity(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    color_materials: &mut Assets<ColorMaterial>,
    image_assets: &mut Assets<Image>,
    texture_cache: &mut RasterTextureCache,
    used_texture_keys: &mut HashSet<String>,
    render_assets: &RenderAssets,
    text_assets: &TextAssets,
    document: &Document,
    reference: &BlockReference,
    base_point: Point2,
    entity: &DocEntity,
) {
    match entity {
        DocEntity::Text(text) => {
            let position = apply_block_transform(reference, base_point, text.insert);
            let rotation = text.rotation + reference.rotation;
            let height = text.height * average_scale(reference);
            spawn_single_line_text(
                commands,
                text_assets,
                &text.content,
                position,
                height,
                rotation,
                Anchor::BOTTOM_LEFT,
            );
        }
        DocEntity::MText(mtext) => {
            let position = apply_block_transform(reference, base_point, mtext.insert);
            let direction = transform_direction(reference, mtext.direction);
            let width = mtext
                .reference_width
                .map(|value| value * average_scale(reference));
            spawn_multiline_text(
                commands,
                text_assets,
                sanitize_mtext_content(&mtext.content),
                position,
                mtext.height * average_scale(reference),
                direction_to_angle(direction),
                width,
                Anchor::BOTTOM_LEFT,
            );
        }
        DocEntity::BlockReference(inner) => {
            if let Some(_) = document.block(&inner.name) {
                let combined = compose_block_reference(reference, base_point, inner);
                spawn_block_reference(
                    commands,
                    meshes,
                    color_materials,
                    image_assets,
                    texture_cache,
                    used_texture_keys,
                    render_assets,
                    text_assets,
                    document,
                    &combined,
                );
            }
        }
        DocEntity::Hatch(hatch) => {
            let loops = hatch_edge_polylines(hatch);
            if loops.is_empty() {
                return;
            }
            let transformed: Vec<Vec<Point2>> = loops
                .into_iter()
                .map(|polyline| {
                    polyline
                        .into_iter()
                        .map(|point| apply_block_transform(reference, base_point, point))
                        .collect::<Vec<_>>()
                })
                .collect();

            let mut gradient = gradient_spec(hatch.gradient.as_ref());
            gradient.direction = rotate_vec2(gradient.direction, reference.rotation);

            spawn_filled_polylines(commands, meshes, render_assets, &transformed, gradient, 0.0);

            for polyline in transformed {
                spawn_polyline_segments(
                    commands,
                    meshes,
                    render_assets.line_material.clone(),
                    &polyline,
                );
            }
        }
        DocEntity::RasterImage(image) => {
            let transformed = transform_raster_image(reference, base_point, image);
            spawn_raster_image(
                commands,
                meshes,
                color_materials,
                image_assets,
                texture_cache,
                used_texture_keys,
                render_assets.line_material.clone(),
                document,
                &transformed,
            );
        }
        _ => {
            for polyline in entity_polylines(entity) {
                if polyline.len() < 2 {
                    continue;
                }
                let transformed: Vec<Point2> = polyline
                    .into_iter()
                    .map(|point| apply_block_transform(reference, base_point, point))
                    .collect();
                spawn_polyline_segments(
                    commands,
                    meshes,
                    render_assets.line_material.clone(),
                    &transformed,
                );
            }
        }
    }
}

fn spawn_block_attribute(
    commands: &mut Commands,
    text_assets: &TextAssets,
    reference: &BlockReference,
    base_point: Point2,
    attribute: &Attribute,
) {
    let target = attribute.alignment.unwrap_or(attribute.insert);
    let position = apply_block_transform(reference, base_point, target);
    let rotation = attribute.rotation + reference.rotation;
    let height = attribute.height * average_scale(reference);
    spawn_single_line_text(
        commands,
        text_assets,
        &attribute.text,
        position,
        height,
        rotation,
        attribute_anchor(attribute.horizontal_align, attribute.vertical_align),
    );
}

fn attribute_anchor(horizontal: i16, vertical: i16) -> Anchor {
    let h = match horizontal {
        1 | 4 => 1,
        2 | 5 => 2,
        _ => 0,
    };
    let v = match vertical {
        1 => 1,
        2 | 3 => 2,
        _ => 0,
    };

    match (h, v) {
        (0, 0) => Anchor::BOTTOM_LEFT,
        (1, 0) => Anchor::BOTTOM_CENTER,
        (2, 0) => Anchor::BOTTOM_RIGHT,
        (0, 1) => Anchor::CENTER_LEFT,
        (1, 1) => Anchor::CENTER,
        (2, 1) => Anchor::CENTER_RIGHT,
        (0, 2) => Anchor::TOP_LEFT,
        (1, 2) => Anchor::TOP_CENTER,
        (2, 2) => Anchor::TOP_RIGHT,
        _ => Anchor::BOTTOM_LEFT,
    }
}

fn compose_block_reference(
    parent: &BlockReference,
    parent_base: Point2,
    child: &BlockReference,
) -> BlockReference {
    let mut combined = child.clone();
    combined.insert = apply_block_transform(parent, parent_base, child.insert);
    let parent_scale = parent.scale.as_vec2();
    let child_scale = child.scale.as_vec2();
    combined.scale = Vector2::new(
        parent_scale.x * child_scale.x,
        parent_scale.y * child_scale.y,
    );
    combined.rotation = child.rotation + parent.rotation;
    combined
}

fn apply_block_transform(reference: &BlockReference, base_point: Point2, point: Point2) -> Point2 {
    let scale = reference.scale.as_vec2();
    let mut local = point.as_vec2() - base_point.as_vec2();
    local.x *= scale.x;
    local.y *= scale.y;
    let sin = reference.rotation.sin();
    let cos = reference.rotation.cos();
    let rotated = DVec2::new(local.x * cos - local.y * sin, local.x * sin + local.y * cos);
    let translated = rotated + reference.insert.as_vec2();
    Point2::from_vec(translated)
}

fn transform_direction(reference: &BlockReference, direction: Vector2) -> Vector2 {
    let vec = direction.as_vec2();
    let sin = reference.rotation.sin();
    let cos = reference.rotation.cos();
    let rotated = DVec2::new(vec.x * cos - vec.y * sin, vec.x * sin + vec.y * cos);
    Vector2::from(rotated)
}

fn average_scale(reference: &BlockReference) -> f64 {
    let scale = reference.scale.as_vec2();
    ((scale.x.abs() + scale.y.abs()) * 0.5).max(1e-6)
}

fn transform_raster_image(
    reference: &BlockReference,
    base_point: Point2,
    image: &RasterImage,
) -> RasterImage {
    let mut transformed = image.clone();
    transformed.insert = apply_block_transform(reference, base_point, image.insert);
    transformed.u_vector = transform_raster_axis(reference, image.u_vector);
    transformed.v_vector = transform_raster_axis(reference, image.v_vector);
    transformed
}

fn transform_raster_axis(reference: &BlockReference, axis: Vector2) -> Vector2 {
    let scale = reference.scale.as_vec2();
    let mut vec = axis.as_vec2();
    vec.x *= scale.x;
    vec.y *= scale.y;
    let sin = reference.rotation.sin();
    let cos = reference.rotation.cos();
    let rotated = DVec2::new(vec.x * cos - vec.y * sin, vec.x * sin + vec.y * cos);
    Vector2::from(rotated)
}

fn raster_local_to_world_point(image: &RasterImage, local: Point2) -> Point2 {
    raster_like_local_to_world_point(image.insert, image.u_vector, image.v_vector, local)
}

fn raster_like_local_to_world_point(
    insert: Point2,
    u_vector: Vector2,
    v_vector: Vector2,
    local: Point2,
) -> Point2 {
    let origin = insert.as_vec2();
    let u = u_vector.as_vec2();
    let v = v_vector.as_vec2();
    let local_vec = local.as_vec2();
    let world = origin + u * local_vec.x + v * local_vec.y;
    Point2::from_vec(world)
}

fn raster_local_polygon(image: &RasterImage) -> (Vec<Point2>, ClipMode) {
    raster_like_local_polygon(
        image.image_size,
        image.display_options.use_clipping,
        image.clip.as_ref(),
    )
}

fn raster_like_local_polygon(
    image_size: Vector2,
    use_clipping: bool,
    clip: Option<&RasterImageClip>,
) -> (Vec<Point2>, ClipMode) {
    let width = image_size.x();
    let height = image_size.y();
    if width <= 0.0 || height <= 0.0 {
        return (Vec::new(), ClipMode::Outside);
    }

    let default_rect = || {
        vec![
            Point2::new(0.0, 0.0),
            Point2::new(width, 0.0),
            Point2::new(width, height),
            Point2::new(0.0, height),
        ]
    };

    let mut clip_mode = ClipMode::Outside;
    let mut polygon = if use_clipping {
        if let Some(clip) = clip {
            match clip {
                RasterImageClip::Rectangle { min, max, mode } => {
                    clip_mode = *mode;
                    vec![
                        Point2::new(min.x(), min.y()),
                        Point2::new(max.x(), min.y()),
                        Point2::new(max.x(), max.y()),
                        Point2::new(min.x(), max.y()),
                    ]
                }
                RasterImageClip::Polygon { vertices, mode } if vertices.len() >= 3 => {
                    clip_mode = *mode;
                    vertices.clone()
                }
                _ => default_rect(),
            }
        } else {
            default_rect()
        }
    } else {
        default_rect()
    };

    if polygon.len() >= 3 {
        polygon = normalize_clip_polygon(polygon, clip_mode);
    }

    (polygon, clip_mode)
}

fn wipeout_local_polygon(wipeout: &Wipeout) -> (Vec<Point2>, ClipMode) {
    raster_like_local_polygon(
        wipeout.image_size,
        wipeout.display_options.use_clipping,
        wipeout.clip.as_ref(),
    )
}

fn normalize_clip_polygon(mut polygon: Vec<Point2>, mode: ClipMode) -> Vec<Point2> {
    if polygon.len() < 3 {
        return polygon;
    }

    if let (Some(first), Some(last)) = (polygon.first().cloned(), polygon.last().cloned()) {
        if last == first {
            polygon.pop();
        }
    }

    if polygon.len() < 3 {
        return polygon;
    }

    let area = signed_polygon_area(&polygon);
    if area.abs() < f64::EPSILON {
        return polygon;
    }

    let is_ccw = area >= 0.0;
    if is_ccw == mode.wants_ccw() {
        polygon
    } else {
        polygon.reverse();
        polygon
    }
}

fn signed_polygon_area(points: &[Point2]) -> f64 {
    let mut area = 0.0;
    let len = points.len();
    for i in 0..len {
        let current = points[i];
        let next = points[(i + 1) % len];
        area += current.x() * next.y() - current.y() * next.x();
    }
    0.5 * area
}

fn raster_clip_summary(image: &RasterImage, clip_mode: ClipMode) -> String {
    if image.display_options.use_clipping {
        match &image.clip {
            Some(RasterImageClip::Rectangle { min, max, .. }) => format!(
                "rect min=({:.2},{:.2}) max=({:.2},{:.2}) mode={}",
                min.x(),
                min.y(),
                max.x(),
                max.y(),
                clip_mode.describe()
            ),
            Some(RasterImageClip::Polygon { vertices, .. }) => format!(
                "polygon {} pts mode={}",
                vertices.len(),
                clip_mode.describe()
            ),
            None => format!(
                "clipping enabled without boundary mode={}",
                clip_mode.describe()
            ),
        }
    } else {
        format!("clipping disabled mode={}", clip_mode.describe())
    }
}

fn spawn_raster_image(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    color_materials: &mut Assets<ColorMaterial>,
    image_assets: &mut Assets<Image>,
    texture_cache: &mut RasterTextureCache,
    used_texture_keys: &mut HashSet<String>,
    border_material: Handle<ColorMaterial>,
    document: &Document,
    image: &RasterImage,
) {
    if !image.display_options.show_image && !image.display_options.show_border {
        return;
    }

    let (polygon_local, clip_mode) = raster_local_polygon(image);
    let clip_summary = raster_clip_summary(image, clip_mode);
    if polygon_local.len() < 3 {
        warn!(
            handle = %image.image_def_handle,
            "Raster 图像裁剪顶点不足，跳过渲染"
        );
        return;
    }

    let world_points: Vec<Point2> = polygon_local
        .iter()
        .map(|point| raster_local_to_world_point(image, *point))
        .collect();

    if image.display_options.show_image {
        let mut texture_to_use: Option<Handle<Image>> = None;
        let mut texture_diag = "missing IMAGEDEF definition".to_string();
        let mut used_fallback = false;

        if let Some(definition) = document.raster_image_definition(&image.image_def_handle) {
            let dictionary_entry = dictionary_entry_for_image(document, image);
            if document.image_dictionary().is_some() && dictionary_entry.is_none() {
                trace!(
                    handle = %image.image_def_handle,
                    "IMAGE 字典存在但未找到与当前插图关联的条目"
                );
            }

            let cache_key = raster_texture_cache_key(document, image, definition, dictionary_entry);

            if let Some(handle) = texture_cache.get(&cache_key) {
                texture_cache.bump_usage(&cache_key);
                used_texture_keys.insert(cache_key.clone());
                texture_to_use = Some(handle);
                texture_diag = format!("cache hit (key={cache_key})");
            } else {
                let resolved_path = definition.resolved_path.clone().or_else(|| {
                    let candidate = Path::new(&definition.file_path);
                    if candidate.is_absolute() {
                        Some(candidate.to_string_lossy().into_owned())
                    } else {
                        None
                    }
                });

                if let Some(path_string) = resolved_path {
                    match load_raster_texture(&cache_key, &path_string, texture_cache, image_assets)
                    {
                        Some(handle) => {
                            used_texture_keys.insert(cache_key.clone());
                            texture_to_use = Some(handle);
                            texture_diag = format!("loaded {} (key={cache_key})", path_string);
                        }
                        None => {
                            warn!(path = %path_string, "Raster 图像纹理加载失败，使用占位纹理");
                            texture_diag =
                                format!("failed to load {} (key={cache_key})", path_string);
                        }
                    }
                } else {
                    warn!(
                        handle = %image.image_def_handle,
                        path = %definition.file_path,
                        "Raster 图像未能解析出有效路径，使用占位纹理"
                    );
                    texture_diag = format!(
                        "no resolved path for {} (key={cache_key})",
                        definition.file_path
                    );
                }
            }
        } else {
            warn!(
                handle = %image.image_def_handle,
                "Raster 图像缺少对应的 IMAGEDEF 定义，使用占位纹理"
            );
        }

        if texture_to_use.is_none() {
            texture_to_use = texture_cache.fallback();
            if texture_to_use.is_some() {
                used_fallback = true;
            }
        }

        let texture_summary = if used_fallback {
            format!("{texture_diag}; using fallback placeholder")
        } else {
            texture_diag.clone()
        };

        info!(
            handle = %image.image_def_handle,
            clip = %clip_summary,
            texture = %texture_summary,
            "Raster 图像渲染诊断"
        );

        if let Some(texture_handle) = texture_to_use {
            let width = image.image_size.x().max(1e-6);
            let height = image.image_size.y().max(1e-6);

            let positions: Vec<[f32; 3]> = world_points
                .iter()
                .map(|point| [point.x() as f32, point.y() as f32, RASTER_DEPTH])
                .collect();

            let uvs: Vec<[f32; 2]> = polygon_local
                .iter()
                .map(|point| [(point.x() / width) as f32, (point.y() / height) as f32])
                .collect();

            let mut indices: Vec<u32> = Vec::new();
            for idx in 1..polygon_local.len() - 1 {
                indices.extend_from_slice(&[0, idx as u32, (idx + 1) as u32]);
            }

            if !indices.is_empty() {
                let mut mesh = Mesh::new(
                    PrimitiveTopology::TriangleList,
                    RenderAssetUsages::RENDER_WORLD,
                );
                mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
                mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
                mesh.insert_indices(Indices::U32(indices));

                let mesh_handle = meshes.add(mesh);
                let material_handle = color_materials.add(ColorMaterial::from(texture_handle));
                commands.spawn((
                    Mesh2d(mesh_handle),
                    MeshMaterial2d(material_handle),
                    Transform::default(),
                    GlobalTransform::default(),
                    Visibility::default(),
                    InheritedVisibility::default(),
                ));
            }
        }
    }

    if image.display_options.show_border {
        let mut border = world_points.clone();
        if let Some(first) = world_points.first() {
            border.push(*first);
        }
        spawn_polyline_segments(commands, meshes, border_material, &border);
    }
}

fn load_raster_texture(
    cache_key: &str,
    path: &str,
    cache: &mut RasterTextureCache,
    image_assets: &mut Assets<Image>,
) -> Option<Handle<Image>> {
    let reader = match ImageReader::open(path) {
        Ok(reader) => reader,
        Err(err) => {
            warn!(path, %err, "读取 Raster 图像失败");
            return None;
        }
    };

    let reader = match reader.with_guessed_format() {
        Ok(reader) => reader,
        Err(err) => {
            warn!(path, %err, "识别 Raster 图像格式失败");
            return None;
        }
    };

    let dynamic = match reader.decode() {
        Ok(image) => image,
        Err(err) => {
            warn!(path, %err, "解码 Raster 图像失败");
            return None;
        }
    };

    let image = Image::from_dynamic(dynamic, true, RenderAssetUsages::RENDER_WORLD);
    let handle = image_assets.add(image);
    cache.insert(cache_key.to_string(), handle.clone());
    Some(handle)
}

fn raster_texture_cache_key(
    document: &Document,
    image: &RasterImage,
    definition: &RasterImageDefinition,
    dictionary_entry: Option<&ImageDictionaryEntry>,
) -> String {
    let base_key = if let Some(reactor_handle) = image.image_def_reactor_handle.as_ref() {
        format!("reactor:{}", reactor_handle)
    } else if let Some(entry) = dictionary_entry {
        if let Some(reactor_handle) = entry.reactor_handle.as_ref() {
            format!("dict-reactor:{}", reactor_handle)
        } else {
            format!("dict-name:{}", entry.name.to_lowercase())
        }
    } else if let Some(resolved) = definition.resolved_path.as_ref() {
        format!("path:{}", resolved)
    } else {
        format!("path:{}", definition.file_path)
    };

    if let Some(signature) = raster_variable_signature(document.raster_image_variables()) {
        format!("{base_key}|{signature}")
    } else {
        base_key
    }
}

fn dictionary_entry_for_image<'a>(
    document: &'a Document,
    image: &RasterImage,
) -> Option<&'a ImageDictionaryEntry> {
    document.image_dictionary().and_then(|dictionary| {
        dictionary
            .entries
            .iter()
            .find(|entry| entry.image_def_handle == image.image_def_handle)
    })
}

fn raster_variable_signature(vars: Option<&RasterImageVariables>) -> Option<String> {
    let vars = vars?;
    let mut parts: Vec<String> = Vec::new();
    if let Some(frame) = vars.frame {
        parts.push(format!("frame={frame}"));
    }
    if let Some(quality) = vars.quality {
        parts.push(format!("quality={quality}"));
    }
    if let Some(units) = vars.units {
        parts.push(format!("units={units}"));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(","))
    }
}

fn spawn_hatch_fill(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    render_assets: &RenderAssets,
    hatch: &zcad_core::document::Hatch,
) {
    let loops: Vec<Vec<Point2>> = hatch_edge_polylines(hatch);
    if loops.is_empty() {
        return;
    }

    let gradient = gradient_spec(hatch.gradient.as_ref());
    spawn_filled_polylines(commands, meshes, render_assets, &loops, gradient, 0.0);
}

fn spawn_face3d(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    render_assets: &RenderAssets,
    face: &ThreeDFace,
) {
    let vertices_xy: [Point2; 4] = face
        .vertices
        .map(|vertex| Point2::new(vertex.x(), vertex.y()));
    let outline = face3d_outline_points(&vertices_xy);
    let depth = face3d_depth(face);
    if outline.len() >= 3 {
        let loops = vec![outline];
        let gradient = face3d_solid_gradient(face);
        spawn_filled_polylines(commands, meshes, render_assets, &loops, gradient, depth);
    }
    spawn_face3d_edges(
        commands,
        meshes,
        render_assets.line_material.clone(),
        &vertices_xy,
        face.invisible_edges,
        depth + 0.01,
    );
}

fn face3d_outline_points(vertices: &[Point2; 4]) -> Vec<Point2> {
    let mut outline = vec![vertices[0], vertices[1], vertices[2]];
    if !points_close(vertices[2], vertices[3]) {
        outline.push(vertices[3]);
    }
    if let Some(first) = outline.first().copied() {
        if let Some(last) = outline.last() {
            if !points_close(first, *last) {
                outline.push(first);
            }
        }
    }
    outline
}

fn spawn_face3d_edges(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    material: Handle<ColorMaterial>,
    vertices: &[Point2; 4],
    invisible_edges: [bool; 4],
    depth: f32,
) {
    let edges = [
        (0usize, 1usize, invisible_edges[0]),
        (1, 2, invisible_edges[1]),
        (2, 3, invisible_edges[2]),
        (3, 0, invisible_edges[3]),
    ];
    for (start_idx, end_idx, hidden) in edges {
        if hidden {
            continue;
        }
        let start = vertices[start_idx];
        let end = vertices[end_idx];
        if points_close(start, end) {
            continue;
        }
        let _ = spawn_line_segment(commands, meshes, material.clone(), start, end, depth);
    }
}

fn face3d_solid_gradient(face: &ThreeDFace) -> GradientSpec {
    let color = face3d_shaded_color(face);
    GradientSpec {
        direction: Vec2::X,
        start: color,
        end: color,
        shift: 0.0,
    }
}

fn face3d_depth(face: &ThreeDFace) -> f32 {
    let scaled = FACE3D_DEPTH_BASE + face.average_height() as f32 * FACE3D_DEPTH_SCALE;
    scaled.clamp(FACE3D_DEPTH_MIN, FACE3D_DEPTH_MAX)
}

fn face3d_shaded_color(face: &ThreeDFace) -> Color {
    let mut intensity = 0.35;
    if let Some(normal) = face
        .normal()
        .and_then(|n| vector3_to_vec3(n).try_normalize())
    {
        let light = Vec3::new(0.35, 0.25, 0.9).normalize();
        let lambert = normal.dot(light).max(0.0);
        intensity += lambert * 0.6;
    }
    let height_bias = (face.average_height() as f32 * 0.02).tanh() * 0.1;
    intensity = (intensity + height_bias).clamp(0.2, 0.95);
    let base = Vec3::new(0.35, 0.5, 0.75);
    let highlight = Vec3::new(0.65, 0.8, 0.95);
    let color_vec = base.lerp(highlight, intensity);
    Color::srgba(color_vec.x, color_vec.y, color_vec.z, 0.55)
}

fn vector3_to_vec3(vector: Vector3) -> Vec3 {
    let raw = vector.as_vec3();
    Vec3::new(raw.x as f32, raw.y as f32, raw.z as f32)
}

fn entity_polylines(entity: &DocEntity) -> Vec<Vec<Point2>> {
    match entity {
        DocEntity::Line(line) => vec![vec![line.start, line.end]],
        DocEntity::Circle(circle) => sample_circle(circle.center, circle.radius, 64),
        DocEntity::Arc(arc) => sample_arc_points(
            arc.center,
            arc.radius,
            arc.start_angle,
            arc.end_angle,
            true,
            48,
        ),
        DocEntity::Ellipse(ellipse) => {
            let is_counter_clockwise = ellipse.end_parameter >= ellipse.start_parameter;
            let sampled = sample_ellipse_points(
                ellipse.center,
                ellipse.major_axis,
                ellipse.ratio,
                ellipse.start_parameter,
                ellipse.end_parameter,
                is_counter_clockwise,
            );
            if sampled.is_empty() {
                Vec::new()
            } else {
                vec![sampled]
            }
        }
        DocEntity::Polyline(polyline) => {
            let mut points: Vec<Point2> = polyline
                .vertices
                .iter()
                .map(|vertex| vertex.position)
                .collect();
            if polyline.is_closed && !points.is_empty() {
                points.push(points[0]);
            }
            vec![points]
        }
        DocEntity::Spline(spline) => {
            let mut sampled = sample_spline_points(&spline.control_points, &spline.fit_points);
            if spline.is_closed && !sampled.is_empty() {
                sampled.push(*sampled.first().unwrap());
            }
            if sampled.is_empty() {
                Vec::new()
            } else {
                vec![sampled]
            }
        }
        DocEntity::Leader(leader) => {
            if leader.vertices.len() >= 2 {
                vec![leader.vertices.clone()]
            } else {
                Vec::new()
            }
        }
        DocEntity::MLeader(mleader) => mleader
            .leader_lines
            .iter()
            .filter_map(|line| {
                if line.vertices.len() >= 2 {
                    Some(line.vertices.clone())
                } else {
                    None
                }
            })
            .collect(),
        DocEntity::Dimension(dimension) => {
            let mut lines = Vec::new();
            if let (Some(origin), Some(end)) = (
                dimension.extension_line_origin,
                dimension.extension_line_end,
            ) {
                lines.push(vec![origin, end]);
            }
            if let Some(point) = dimension.dimension_line_point {
                lines.push(vec![dimension.definition_point, point]);
            }
            if let Some(point) = dimension.secondary_point {
                lines.push(vec![dimension.definition_point, point]);
            }
            if let Some(point) = dimension.arc_definition_point {
                lines.push(vec![dimension.definition_point, point]);
            }
            lines
        }
        DocEntity::BlockReference(_) => Vec::new(),
        DocEntity::Text(_) | DocEntity::MText(_) => Vec::new(),
        DocEntity::Hatch(_) => Vec::new(),
        DocEntity::RasterImage(_) => Vec::new(),
        DocEntity::Wipeout(wipeout) => {
            let (local_polygon, _) = wipeout_local_polygon(wipeout);
            if local_polygon.len() < 3 {
                Vec::new()
            } else {
                vec![
                    local_polygon
                        .into_iter()
                        .map(|point| {
                            raster_like_local_to_world_point(
                                wipeout.insert,
                                wipeout.u_vector,
                                wipeout.v_vector,
                                point,
                            )
                        })
                        .collect::<Vec<_>>(),
                ]
            }
        }
        DocEntity::Face3D(_) => Vec::new(),
    }
}

fn hatch_edge_polylines(hatch: &zcad_core::document::Hatch) -> Vec<Vec<Point2>> {
    hatch.loops.iter().filter_map(sample_hatch_loop).collect()
}

fn sample_hatch_loop(loop_path: &HatchLoop) -> Option<Vec<Point2>> {
    let mut points: Vec<Point2> = Vec::new();
    for edge in &loop_path.edges {
        let edge_points = match edge {
            HatchEdge::Line { start, end } => vec![*start, *end],
            HatchEdge::PolylineSegment { start, end, bulge } => {
                sample_polyline_segment(*start, *end, *bulge)
            }
            HatchEdge::Arc {
                center,
                radius,
                start_angle,
                end_angle,
                is_counter_clockwise,
            } => sample_arc_segment(
                *center,
                *radius,
                *start_angle,
                *end_angle,
                *is_counter_clockwise,
                32,
            ),
            HatchEdge::Ellipse {
                center,
                major_axis,
                minor_ratio,
                start_angle,
                end_angle,
                is_counter_clockwise,
            } => sample_ellipse_points(
                *center,
                *major_axis,
                *minor_ratio,
                *start_angle,
                *end_angle,
                *is_counter_clockwise,
            ),
            HatchEdge::BoundaryReference { .. } => return None,
            HatchEdge::Spline {
                control_points,
                fit_points,
                ..
            } => sample_spline_points(control_points, fit_points),
        };
        append_edge_points(&mut points, edge_points);
    }

    if let Some(first) = points.first().copied() {
        if let Some(last) = points.last() {
            if !points_close(*last, first) {
                points.push(first);
            }
        }
    }

    if points.len() >= 2 {
        Some(points)
    } else {
        None
    }
}

fn sample_circle(center: Point2, radius: f64, segments: usize) -> Vec<Vec<Point2>> {
    if radius <= f64::EPSILON {
        return Vec::new();
    }
    let segs = segments.max(16);
    let mut points = Vec::with_capacity(segs + 1);
    for i in 0..=segs {
        let angle = TAU * (i as f64) / (segs as f64);
        points.push(point_on_circle(center, radius, angle));
    }
    vec![points]
}

fn sample_arc_points(
    center: Point2,
    radius: f64,
    start_angle: f64,
    end_angle: f64,
    is_counter_clockwise: bool,
    min_segments: usize,
) -> Vec<Vec<Point2>> {
    let segment = sample_arc_segment(
        center,
        radius,
        start_angle,
        end_angle,
        is_counter_clockwise,
        min_segments,
    );
    if segment.is_empty() {
        Vec::new()
    } else {
        vec![segment]
    }
}

fn sample_arc_segment(
    center: Point2,
    radius: f64,
    start_angle: f64,
    end_angle: f64,
    is_counter_clockwise: bool,
    min_segments: usize,
) -> Vec<Point2> {
    if radius <= f64::EPSILON {
        return Vec::new();
    }
    let (start, end) = canonical_angle_range(start_angle, end_angle, is_counter_clockwise);
    let span = end - start;
    let segments = ((span.abs() / (TAU / 64.0)).ceil() as usize).max(min_segments);
    let mut points = Vec::with_capacity(segments + 1);
    for i in 0..=segments {
        let angle = start + span * (i as f64 / segments as f64);
        points.push(point_on_circle(center, radius, angle));
    }
    points
}

fn sample_polyline_segment(start: Point2, end: Point2, bulge: f64) -> Vec<Point2> {
    if bulge.abs() <= 1e-9 {
        return vec![start, end];
    }
    sample_bulged_segment(start, end, bulge, 24)
}

fn sample_bulged_segment(
    start: Point2,
    end: Point2,
    bulge: f64,
    min_segments: usize,
) -> Vec<Point2> {
    let start_vec = start.as_vec2();
    let end_vec = end.as_vec2();
    let chord = end_vec - start_vec;
    let chord_len = chord.length();
    if chord_len <= f64::EPSILON {
        return vec![start, end];
    }
    let theta = 4.0 * bulge.atan();
    if theta.abs() <= 1e-9 {
        return vec![start, end];
    }
    let half_theta = theta / 2.0;
    let sin_half = half_theta.sin();
    if sin_half.abs() <= 1e-9 {
        return vec![start, end];
    }
    let radius = chord_len / (2.0 * sin_half);
    let midpoint = (start_vec + end_vec) * 0.5;
    let perp = DVec2::new(-chord.y, chord.x);
    if perp.length_squared() <= f64::EPSILON {
        return vec![start, end];
    }
    let perp_dir = perp.normalize();
    let sagitta = bulge * chord_len / 2.0;
    let center_vec = midpoint + perp_dir * sagitta;
    let center = Point2::from_vec(center_vec);
    let start_dir = start_vec - center_vec;
    let start_angle = start_dir.y.atan2(start_dir.x);
    let end_angle = start_angle + theta;
    sample_arc_segment(
        center,
        radius.abs(),
        start_angle,
        end_angle,
        theta > 0.0,
        min_segments,
    )
}

fn sample_ellipse_points(
    center: Point2,
    major_axis: Vector2,
    minor_ratio: f64,
    start_angle: f64,
    end_angle: f64,
    is_counter_clockwise: bool,
) -> Vec<Point2> {
    let major_vec = major_axis.as_vec2();
    let major_length = major_vec.length();
    if major_length <= f64::EPSILON {
        return Vec::new();
    }
    let minor_length = major_length * minor_ratio.abs();
    let major_dir = major_vec / major_length;
    let minor_dir = DVec2::new(-major_dir.y, major_dir.x);
    let minor_vec = minor_dir * minor_length;

    let (start, end) = canonical_angle_range(start_angle, end_angle, is_counter_clockwise);
    let span = end - start;
    let segments = ((span.abs() / (TAU / 64.0)).ceil() as usize).max(32);
    let mut points = Vec::with_capacity(segments + 1);
    for i in 0..=segments {
        let angle = start + span * (i as f64 / segments as f64);
        let offset = major_vec * angle.cos() + minor_vec * angle.sin();
        let pos = center.as_vec2() + offset;
        points.push(Point2::from_vec(pos));
    }
    points
}

fn sample_spline_points(control_points: &[Point2], fit_points: &[Point2]) -> Vec<Point2> {
    let mut points = Vec::new();
    if control_points.is_empty() {
        return points;
    }
    points.extend_from_slice(control_points);
    if !fit_points.is_empty() {
        if let Some(last_control) = points.last().copied() {
            let mut iter = fit_points.iter();
            if let Some(first_fit) = iter.next() {
                if !points_close(last_control, *first_fit) {
                    points.push(*first_fit);
                }
            }
            points.extend(iter.copied());
        }
    }
    points
}

fn append_edge_points(points: &mut Vec<Point2>, mut edge_points: Vec<Point2>) {
    if edge_points.is_empty() {
        return;
    }
    if points.is_empty() {
        points.extend(edge_points);
        return;
    }
    if let (Some(last), Some(first)) = (points.last(), edge_points.first()) {
        if points_close(*last, *first) {
            edge_points.remove(0);
        }
    }
    points.extend(edge_points);
}

fn points_close(a: Point2, b: Point2) -> bool {
    (a.x() - b.x()).abs() < 1e-6 && (a.y() - b.y()).abs() < 1e-6
}

fn gradient_direction(
    gradient: Option<&zcad_core::document::HatchGradient>,
) -> (Vec2, Color, Color, f32) {
    if let Some(gradient) = gradient {
        let angle = gradient.angle as f32;
        let mut dir = Vec2::new(angle.cos(), angle.sin());
        if dir.length_squared() <= f32::EPSILON {
            dir = Vec2::X;
        } else {
            dir = dir.normalize();
        }
        let base_color = gradient_color_to_bevy(gradient.color1)
            .unwrap_or_else(|| Color::srgba(0.8, 0.8, 0.8, 0.85));
        let end_color = if gradient.is_single_color {
            base_color
        } else {
            gradient_color_to_bevy(gradient.color2).unwrap_or(base_color)
        };
        let shift = gradient
            .shift
            .map(|value| value.clamp(-1.0, 1.0) as f32)
            .unwrap_or(0.0);
        (dir, base_color, end_color, shift)
    } else {
        (
            Vec2::new(1.0, 0.0),
            Color::srgba(0.25, 0.55, 0.85, 0.6),
            Color::srgba(0.1, 0.25, 0.6, 0.6),
            0.0,
        )
    }
}

fn aci_to_color(index: u32) -> Color {
    match index {
        1 => Color::srgb(1.0, 0.0, 0.0),
        2 => Color::srgb(1.0, 1.0, 0.0),
        3 => Color::srgb(0.0, 1.0, 0.0),
        4 => Color::srgb(0.0, 1.0, 1.0),
        5 => Color::srgb(0.0, 0.0, 1.0),
        6 => Color::srgb(1.0, 0.0, 1.0),
        7 => Color::srgb(1.0, 1.0, 1.0),
        _ => Color::srgba(0.7, 0.7, 0.7, 0.8),
    }
}

fn gradient_color_to_bevy(raw: Option<u32>) -> Option<Color> {
    let value = raw?;
    if value == 0 {
        return None;
    }
    if value <= 255 {
        let base = aci_to_color(value);
        let [r, g, b, _] = color_to_rgba(base);
        Some(Color::srgba(r, g, b, 0.85))
    } else {
        Some(true_color_to_color(value))
    }
}

fn true_color_to_color(rgb: u32) -> Color {
    let red = ((rgb >> 16) & 0xFF) as f32 / 255.0;
    let green = ((rgb >> 8) & 0xFF) as f32 / 255.0;
    let blue = (rgb & 0xFF) as f32 / 255.0;
    Color::srgba(red, green, blue, 0.85)
}

fn apply_gradient_shift(value: f32, shift: f32) -> f32 {
    if shift.abs() <= f32::EPSILON {
        value
    } else {
        (value + shift).rem_euclid(1.0)
    }
}

fn color_to_rgba(color: Color) -> [f32; 4] {
    let srgba = color.to_srgba();
    [srgba.red, srgba.green, srgba.blue, srgba.alpha]
}

fn canonical_angle_range(start: f64, end: f64, ccw: bool) -> (f64, f64) {
    if ccw {
        let start = normalize_angle(start);
        let mut end = normalize_angle(end);
        if (end - start).abs() < 1e-9 {
            end = start + TAU;
        } else if end < start {
            end += TAU;
        }
        (start, end)
    } else {
        let mut start = normalize_angle(start);
        let end = normalize_angle(end);
        if (start - end).abs() < 1e-9 {
            start = start + TAU;
        } else if start < end {
            start += TAU;
        }
        (start, end)
    }
}

fn normalize_angle(angle: f64) -> f64 {
    let mut result = angle % TAU;
    if result < 0.0 {
        result += TAU;
    }
    result
}

fn point_on_circle(center: Point2, radius: f64, angle: f64) -> Point2 {
    center.translate(Vector2::new(radius * angle.cos(), radius * angle.sin()))
}

fn handle_keyboard_commands(
    keys: Res<ButtonInput<KeyCode>>,
    mut scene_res: ResMut<SceneResource>,
    command_bus: Res<CommandBusResource>,
) {
    let mut triggered = Vec::new();
    if keys.just_pressed(KeyCode::KeyF) {
        triggered.push("focus_selection");
    }
    if keys.just_pressed(KeyCode::Escape) {
        triggered.push("clear_selection");
    }

    for name in triggered {
        dispatch_command(&command_bus.0, &mut scene_res, name);
    }
}

fn handle_zoom(
    mut events: MessageReader<MouseWheel>,
    mut query: Query<&mut Projection, With<MainCamera>>,
) {
    let mut scale_delta = 1.0f32;
    for event in events.read() {
        let scroll_amount = if event.unit == bevy::input::mouse::MouseScrollUnit::Line {
            event.y * 0.1
        } else {
            event.y * 0.02
        };
        scale_delta *= (1.0 - scroll_amount).clamp(0.2, 5.0);
    }

    if (scale_delta - 1.0).abs() < f32::EPSILON {
        return;
    }

    if let Ok(mut projection) = query.single_mut() {
        if let Projection::Orthographic(ref mut ortho) = *projection {
            ortho.scale = (ortho.scale * scale_delta).clamp(0.1, 10.0);
        }
    }
}

fn handle_pan(
    mut pan_state: ResMut<PanState>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut motion_events: MessageReader<MouseMotion>,
    mut query: Query<(&mut Transform, &Projection), With<MainCamera>>,
) {
    let dragging = buttons.pressed(MouseButton::Middle) || buttons.pressed(MouseButton::Right);
    pan_state.is_dragging = dragging;
    if !pan_state.is_dragging {
        return;
    }

    let mut delta = Vec2::ZERO;
    for motion in motion_events.read() {
        delta += motion.delta;
    }
    if delta == Vec2::ZERO {
        return;
    }

    if let Ok((mut transform, projection)) = query.single_mut() {
        let scale = match projection {
            Projection::Orthographic(ortho) => ortho.scale,
            _ => 1.0,
        };
        transform.translation.x -= delta.x * scale;
        transform.translation.y += delta.y * scale;
    }
}

fn dispatch_command(command_bus: &CommandBus, scene_res: &mut SceneResource, name: &str) {
    let request = CommandRequest {
        name: name.to_string(),
        args: Vec::new(),
    };
    let mut context = CommandContext {
        scene: &mut scene_res.scene,
    };
    let response = command_bus.dispatch(&request, &mut context);
    let message = response
        .message
        .unwrap_or_else(|| "命令未返回消息".to_string());
    if response.success {
        info!(command = name, message = %message, "命令执行成功");
        scene_res.last_command_feedback = Some(format!("Ok({name}): {message}"));
    } else {
        warn!(command = name, message = %message, "命令执行失败");
        scene_res.last_command_feedback = Some(format!("Err({name}): {message}"));
    }
}

fn setup_highlight_assets(mut commands: Commands, mut materials: ResMut<Assets<ColorMaterial>>) {
    let material = materials.add(ColorMaterial::from(Color::srgba(1.0, 0.8, 0.0, 0.85)));
    commands.insert_resource(HighlightAssets { material });
}

fn update_selection_highlight(
    mut commands: Commands,
    scene_res: Res<SceneResource>,
    highlight_entities: Query<Entity, With<SelectionHighlight>>,
    mut meshes: ResMut<Assets<Mesh>>,
    highlight_assets: Res<HighlightAssets>,
) {
    if !scene_res.is_added() && !scene_res.is_changed() {
        return;
    }

    for entity in highlight_entities.iter() {
        commands.entity(entity).despawn();
    }

    let document = scene_res.scene.document();
    for id in scene_res.scene.selection() {
        if let Some(bounds) = document.entity_bounds(id) {
            spawn_highlight_for_bounds(
                &mut commands,
                &mut meshes,
                highlight_assets.material.clone(),
                bounds,
            );
        }
    }
}

fn spawn_highlight_for_bounds(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    material: Handle<ColorMaterial>,
    bounds: Bounds2D,
) {
    let min = bounds.min();
    let max = bounds.max();
    let mut width = (max.x() - min.x()).abs();
    let mut height = (max.y() - min.y()).abs();
    if width < 1e-6 && height < 1e-6 {
        width = 0.5;
        height = 0.5;
    }
    let padding = (width.max(height) * 0.05).max(0.25);
    let min_x = min.x() - padding;
    let min_y = min.y() - padding;
    let max_x = max.x() + padding;
    let max_y = max.y() + padding;

    let corners = [
        Point2::new(min_x, min_y),
        Point2::new(max_x, min_y),
        Point2::new(max_x, max_y),
        Point2::new(min_x, max_y),
    ];

    let mut spawn_edge = |start: Point2, end: Point2| {
        let entity = spawn_line_segment(commands, meshes, material.clone(), start, end, 5.0);
        commands.entity(entity).insert(SelectionHighlight);
    };

    for window in corners.windows(2) {
        if let [start, end] = window {
            spawn_edge(*start, *end);
        }
    }
    spawn_edge(corners[3], corners[0]);
}
