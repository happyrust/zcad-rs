use crate::loader::{DocumentSource, load_scene_from_env_or_demo};
use tracing::{info, warn};
use zcad_core::document::{
    ClipMode, DimensionKind, Entity, HatchEdge, RasterImageClip, RasterImageDisplayOptions,
};
use zcad_core::geometry::{Point2, Vector2};
use zcad_engine::command::{CommandBus, CommandContext, CommandRequest};

/// 简易 CLI 演示：尝试加载 DXF 文档，若失败则回退到内置示例，并打印场景概览。
pub fn run_demo() {
    let loaded = load_scene_from_env_or_demo();
    let mut scene = loaded.scene;
    let command_bus = CommandBus::new();
    let mut context = CommandContext { scene: &mut scene };
    if let Err(err) = dispatch_cli_command(&command_bus, "focus_selection", &mut context) {
        warn!("CLI 命令执行失败: {err}");
    }
    let commands: Vec<&str> = command_bus.available_commands().copied().collect();
    println!("支持的命令: {}", commands.join(", "));

    let selection_ids: Vec<u64> = context.scene.selection().map(|id| id.get()).collect();
    let viewport = context.scene.viewport();
    let document = context.scene.document();

    let layer_count = document.layers().count();
    let entity_count = document.entities().count();
    let block_count = document.blocks().count();
    info!(layer_count, entity_count, block_count, "CLI 演示文档统计");

    println!("Rust 版 ZCAD CLI 演示");
    match &loaded.source {
        DocumentSource::Dxf(path) => {
            println!("已从 DXF 加载文档：{}", path.display());
        }
        DocumentSource::Demo => {
            if let Some(ids) = &loaded.demo_entities {
                println!("已构建内置示例图元：");
                println!("  - 基础线段 ID = {}", ids.baseline.get());
                println!("  - 标注圆 ID = {}", ids.circle.get());
                println!("  - 圆弧 ID = {}", ids.arc.get());
                println!("  - 多段线 ID = {}", ids.polyline.get());
                println!("  - 文字 ID = {}", ids.label.get());
            }
        }
    }

    if selection_ids.is_empty() {
        println!("当前尚未选中任何实体。");
    } else {
        let ids: Vec<String> = selection_ids.iter().map(|id| id.to_string()).collect();
        println!("选中集包含实体 ID：{}", ids.join(", "));
    }
    println!(
        "视口中心=({:.2}, {:.2}), 缩放={:.3}",
        viewport.center.x(),
        viewport.center.y(),
        viewport.zoom
    );

    println!("当前文档图层：");
    for layer in document.layers() {
        println!("  - {} (可见: {})", layer.name, layer.is_visible);
    }

    println!("当前文档实体：");
    for (id, entity) in document.entities() {
        match entity {
            Entity::Line(line) => {
                println!(
                    "  - 线段 #{}, Layer={}, 起点=({:.2}, {:.2}), 终点=({:.2}, {:.2})",
                    id.get(),
                    line.layer,
                    line.start.x(),
                    line.start.y(),
                    line.end.x(),
                    line.end.y()
                );
            }
            Entity::Circle(circle) => {
                println!(
                    "  - 圆 #{}, Layer={}, 圆心=({:.2}, {:.2}), 半径={:.2}",
                    id.get(),
                    circle.layer,
                    circle.center.x(),
                    circle.center.y(),
                    circle.radius
                );
            }
            Entity::Arc(arc) => {
                println!(
                    "  - 圆弧 #{}, Layer={}, 圆心=({:.2}, {:.2}), 半径={:.2}, 起始角={:.1}°, 结束角={:.1}°",
                    id.get(),
                    arc.layer,
                    arc.center.x(),
                    arc.center.y(),
                    arc.radius,
                    arc.start_angle.to_degrees(),
                    arc.end_angle.to_degrees()
                );
            }
            Entity::Ellipse(ellipse) => {
                let major = ellipse.major_axis.as_vec2();
                println!(
                    "  - 椭圆 #{}, Layer={}, 圆心=({:.2}, {:.2}), 主轴=({:.2}, {:.2}), 比例={:.3}, 参数范围=[{:.1}°, {:.1}°]",
                    id.get(),
                    ellipse.layer,
                    ellipse.center.x(),
                    ellipse.center.y(),
                    major.x,
                    major.y,
                    ellipse.ratio,
                    ellipse.start_parameter.to_degrees(),
                    ellipse.end_parameter.to_degrees()
                );
            }
            Entity::Polyline(polyline) => {
                let coords: Vec<String> = polyline
                    .vertices
                    .iter()
                    .map(|vertex| {
                        let pos = vertex.position;
                        if vertex.bulge.abs() > 1e-6 {
                            format!(
                                "({:.2}, {:.2}; bulge={:.3})",
                                pos.x(),
                                pos.y(),
                                vertex.bulge
                            )
                        } else {
                            format!("({:.2}, {:.2})", pos.x(), pos.y())
                        }
                    })
                    .collect();
                println!(
                    "  - 多段线 #{}, Layer={}, 顶点数={}, 闭合={}, 顶点={}",
                    id.get(),
                    polyline.layer,
                    polyline.vertices.len(),
                    if polyline.is_closed { "是" } else { "否" },
                    coords.join(" -> ")
                );
            }
            Entity::Spline(spline) => {
                println!(
                    "  - 样条 #{}, Layer={}, 阶数={}, 控制点数={}, 拟合点数={}, 闭合={}, 周期={}, 有理={}",
                    id.get(),
                    spline.layer,
                    spline.degree,
                    spline.control_points.len(),
                    spline.fit_points.len(),
                    if spline.is_closed { "是" } else { "否" },
                    if spline.is_periodic { "是" } else { "否" },
                    if spline.is_rational { "是" } else { "否" }
                );
                if !spline.control_points.is_empty() {
                    let preview: Vec<String> = spline
                        .control_points
                        .iter()
                        .take(5)
                        .map(|pt| format_point(*pt))
                        .collect();
                    println!("    控制点示例: {}", preview.join(" -> "));
                }
                if !spline.fit_points.is_empty() {
                    let preview: Vec<String> = spline
                        .fit_points
                        .iter()
                        .take(5)
                        .map(|pt| format_point(*pt))
                        .collect();
                    println!("    拟合点示例: {}", preview.join(" -> "));
                }
                if !spline.knot_values.is_empty() {
                    let preview: Vec<String> = spline
                        .knot_values
                        .iter()
                        .take(6)
                        .map(|k| format!("{k:.3}"))
                        .collect();
                    println!("    节点值 (前{}): {}", preview.len(), preview.join(", "));
                }
                if !spline.weights.is_empty() {
                    let preview: Vec<String> = spline
                        .weights
                        .iter()
                        .take(6)
                        .map(|w| format!("{w:.3}"))
                        .collect();
                    println!("    权重 (前{}): {}", preview.len(), preview.join(", "));
                }
                if let Some(tangent) = spline.start_tangent {
                    let dir = tangent.as_vec2();
                    println!("    起始切向量=({:.3}, {:.3})", dir.x, dir.y);
                }
                if let Some(tangent) = spline.end_tangent {
                    let dir = tangent.as_vec2();
                    println!("    终止切向量=({:.3}, {:.3})", dir.x, dir.y);
                }
            }
            Entity::Text(text) => {
                println!(
                    "  - 文字 #{}, Layer={}, 位置=({:.2}, {:.2}), 内容=\"{}\", 高度={:.2}, 旋转={:.1}°",
                    id.get(),
                    text.layer,
                    text.insert.x(),
                    text.insert.y(),
                    text.content,
                    text.height,
                    text.rotation.to_degrees()
                );
            }
            Entity::MText(mtext) => {
                let dir = mtext.direction.as_vec2();
                let width_display = mtext
                    .reference_width
                    .map(|w| format!("{w:.2}"))
                    .unwrap_or_else(|| "auto".to_string());
                println!(
                    "  - MText #{}, Layer={}, 位置=({:.2}, {:.2}), 内容=\"{}\", 高度={:.2}, 宽度={}, 方向=({:.2}, {:.2}), 附着={}, 方向标志={}, 样式={}",
                    id.get(),
                    mtext.layer,
                    mtext.insert.x(),
                    mtext.insert.y(),
                    mtext.content.replace('\n', "\\n"),
                    mtext.height,
                    width_display,
                    dir.x,
                    dir.y,
                    mtext.attachment_point,
                    mtext.drawing_direction,
                    mtext.style.as_deref().unwrap_or("<默认>")
                );
            }
            Entity::BlockReference(block) => {
                let scale = block.scale.as_vec2();
                let attr_summary = if block.attributes.is_empty() {
                    "无属性".to_string()
                } else {
                    let preview: Vec<String> = block
                        .attributes
                        .iter()
                        .take(3)
                        .map(|attr| {
                            let mut entry =
                                format!("{}={}", attr.tag, attr.text.replace('\n', "\\n"));
                            let mut flags = Vec::new();
                            if attr.is_invisible {
                                flags.push("隐");
                            }
                            if attr.is_constant {
                                flags.push("常");
                            }
                            if attr.is_verify {
                                flags.push("核");
                            }
                            if attr.is_preset {
                                flags.push("预");
                            }
                            if attr.lock_position {
                                flags.push("锁");
                            }
                            if !flags.is_empty() {
                                entry.push_str(&format!("({})", flags.join("")));
                            }
                            entry
                        })
                        .collect();
                    if block.attributes.len() > 3 {
                        format!("{} (共 {} 项)", preview.join(", "), block.attributes.len())
                    } else {
                        format!("{} ({} 项)", preview.join(", "), block.attributes.len())
                    }
                };
                println!(
                    "  - 块参照 #{}, Layer={}, 名称={}, 位置=({:.2}, {:.2}), 缩放=({:.2}, {:.2}), 旋转={:.1}°, 属性={}",
                    id.get(),
                    block.layer,
                    block.name,
                    block.insert.x(),
                    block.insert.y(),
                    scale.x,
                    scale.y,
                    block.rotation.to_degrees(),
                    attr_summary
                );
            }
            Entity::Leader(leader) => {
                println!(
                    "  - 引线 #{}, Layer={}, 顶点数={}, 含箭头={}",
                    id.get(),
                    leader.layer,
                    leader.vertices.len(),
                    if leader.has_arrowhead { "是" } else { "否" }
                );
            }
            Entity::MLeader(mleader) => {
                let content_desc = match &mleader.content {
                    zcad_core::document::MLeaderContent::MText { text, .. } => {
                        format!("MText({} 行)", text.lines().count())
                    }
                    zcad_core::document::MLeaderContent::Block { block } => {
                        let handle = block
                            .block_handle
                            .as_deref()
                            .or(block.block_name.as_deref())
                            .unwrap_or("<未解析>");
                        format!(
                            "Block(handle={}, 位置=({:.2}, {:.2}), 缩放=({:.2}, {:.2}), 旋转={:.1}°)",
                            handle,
                            block.location.x(),
                            block.location.y(),
                            block.scale.x(),
                            block.scale.y(),
                            block.rotation.to_degrees()
                        )
                    }
                    zcad_core::document::MLeaderContent::None => "无内容".to_string(),
                };
                let scale_desc = mleader
                    .scale
                    .map(|value| format!("{:.2}", value))
                    .unwrap_or_else(|| "-".to_string());
                let dogleg_desc = mleader
                    .dogleg_length
                    .map(|value| format!("{:.2}", value))
                    .unwrap_or_else(|| "-".to_string());
                let landing_desc = mleader
                    .landing_gap
                    .map(|value| format!("{:.2}", value))
                    .unwrap_or_else(|| "-".to_string());
                let dogleg_display = if mleader.has_dogleg {
                    dogleg_desc.clone()
                } else {
                    "-".to_string()
                };
                println!(
                    "  - 多引线 #{}, Layer={}, 线数量={}, 样式={}, 内容={}, 文本高={:?}, 缩放={}, 狗腿={}, LandingGap={}",
                    id.get(),
                    mleader.layer,
                    mleader.leader_lines.len(),
                    mleader.style_name.as_deref().unwrap_or("<默认>"),
                    content_desc,
                    mleader.text_height,
                    scale_desc,
                    dogleg_display,
                    landing_desc
                );
            }
            Entity::RasterImage(image) => {
                println!(
                    "  - 图像 #{}, Layer={}, IMAGEDEF={}, 插入点=({:.2}, {:.2}), 尺寸=({:.2}, {:.2}), {}",
                    id.get(),
                    image.layer,
                    image.image_def_handle,
                    image.insert.x(),
                    image.insert.y(),
                    image.image_size.x(),
                    image.image_size.y(),
                    clip_description(&image.display_options, &image.clip),
                );
            }
            Entity::Wipeout(wipeout) => {
                println!(
                    "  - Wipeout #{}, Layer={}, 插入点=({:.2}, {:.2}), 尺寸=({:.2}, {:.2}), {}",
                    id.get(),
                    wipeout.layer,
                    wipeout.insert.x(),
                    wipeout.insert.y(),
                    wipeout.image_size.x(),
                    wipeout.image_size.y(),
                    clip_description(&wipeout.display_options, &wipeout.clip),
                );
            }
            Entity::Hatch(hatch) => {
                let bounds_desc = document
                    .entity_bounds(*id)
                    .map(|bounds| {
                        format!(
                            "min=({:.2}, {:.2}), max=({:.2}, {:.2})",
                            bounds.min().x(),
                            bounds.min().y(),
                            bounds.max().x(),
                            bounds.max().y()
                        )
                    })
                    .unwrap_or_else(|| "<未定义>".to_string());
                println!(
                    "  - 填充 #{}, Layer={}, 模式={}, 实心={}, 包围盒={}",
                    id.get(),
                    hatch.layer,
                    hatch.pattern_name,
                    if hatch.is_solid { "是" } else { "否" },
                    bounds_desc
                );
                if let Some(gradient) = &hatch.gradient {
                    println!(
                        "    渐变: 名称={}, 角度={:.1}°, 单色={}, 颜色1={:?}, 颜色2={:?}",
                        gradient.name,
                        gradient.angle.to_degrees(),
                        if gradient.is_single_color {
                            "是"
                        } else {
                            "否"
                        },
                        gradient.color1,
                        gradient.color2
                    );
                }
                for (index, loop_path) in hatch.loops.iter().enumerate() {
                    let edge_desc: Vec<String> =
                        loop_path.edges.iter().map(describe_hatch_edge).collect();
                    println!(
                        "    Loop {} [{}|polyline={}]: {}",
                        index + 1,
                        if loop_path.is_closed {
                            "闭合"
                        } else {
                            "开放"
                        },
                        if loop_path.is_polyline { "是" } else { "否" },
                        edge_desc.join(" ; ")
                    );
                    if !loop_path.boundary_handles.is_empty() {
                        println!("      引用边界: {}", loop_path.boundary_handles.join(", "));
                    }
                }
            }
            Entity::Dimension(dimension) => {
                let kind_label = match dimension.kind {
                    DimensionKind::Linear => "线性".to_string(),
                    DimensionKind::Aligned => "对齐".to_string(),
                    DimensionKind::Angular => "角度".to_string(),
                    DimensionKind::Diameter => "直径".to_string(),
                    DimensionKind::Radius => "半径".to_string(),
                    DimensionKind::Angular3Point => "三点角度".to_string(),
                    DimensionKind::Ordinate => "坐标".to_string(),
                    DimensionKind::Unknown(code) => format!("未知({code})"),
                };
                let dim_line = format_point_option(dimension.dimension_line_point);
                let ext_origin = format_point_option(dimension.extension_line_origin);
                let ext_end = format_point_option(dimension.extension_line_end);
                let secondary = format_point_option(dimension.secondary_point);
                let arc_point = format_point_option(dimension.arc_definition_point);
                let center = format_point_option(dimension.center_point);
                let bounds_desc = document
                    .entity_bounds(*id)
                    .map(|bounds| {
                        format!(
                            "min=({:.2}, {:.2}), max=({:.2}, {:.2})",
                            bounds.min().x(),
                            bounds.min().y(),
                            bounds.max().x(),
                            bounds.max().y()
                        )
                    })
                    .unwrap_or_else(|| "<未定义>".to_string());
                println!(
                    "  - 尺寸 #{}, Layer={}, 类型={}, 定义点=({:.2}, {:.2}), 文本位置=({:.2}, {:.2}), 尺寸线={}, 引线起点={}, 引线终点={}, 次要点={}, 弧定义点={}, 圆心={}, 文本={}, 测量值={:?}, 旋转={:.1}°, 文本旋转={}, 倾斜角={}, 包围盒={}",
                    id.get(),
                    dimension.layer,
                    kind_label,
                    dimension.definition_point.x(),
                    dimension.definition_point.y(),
                    dimension.text_midpoint.x(),
                    dimension.text_midpoint.y(),
                    dim_line,
                    ext_origin,
                    ext_end,
                    secondary,
                    arc_point,
                    center,
                    dimension.text.as_deref().unwrap_or("<自动>"),
                    dimension.measurement,
                    dimension.rotation.to_degrees(),
                    dimension
                        .text_rotation
                        .map(|r| format!("{:.1}°", r.to_degrees()))
                        .unwrap_or_else(|| "<保持>".to_string()),
                    dimension
                        .oblique_angle
                        .map(|r| format!("{:.1}°", r.to_degrees()))
                        .unwrap_or_else(|| "<无>".to_string()),
                    bounds_desc
                );
            }
            Entity::Face3D(face) => {
                let vertices_desc: Vec<String> = face
                    .vertices
                    .iter()
                    .enumerate()
                    .map(|(idx, vertex)| {
                        format!(
                            "V{}=({:.2}, {:.2}, {:.2})",
                            idx + 1,
                            vertex.x(),
                            vertex.y(),
                            vertex.z()
                        )
                    })
                    .collect();
                let hidden_edges: Vec<String> = face
                    .invisible_edges
                    .iter()
                    .enumerate()
                    .filter_map(|(idx, hidden)| hidden.then(|| (idx + 1).to_string()))
                    .collect();
                let normal_label = face
                    .normal()
                    .map(|normal| {
                        let vec = normal.as_vec3();
                        let magnitude = vec.length();
                        if magnitude <= f64::EPSILON {
                            "<退化>".to_string()
                        } else {
                            format!(
                                "dir=({:.3}, {:.3}, {:.3}), |n|={:.3}",
                                vec.x / magnitude,
                                vec.y / magnitude,
                                vec.z / magnitude,
                                magnitude
                            )
                        }
                    })
                    .unwrap_or_else(|| "<退化>".to_string());
                println!(
                    "  - 3DFACE #{}, Layer={}, 顶点={}, 隐藏边={}, 平均高度={:.3}, 法向={}",
                    id.get(),
                    face.layer,
                    vertices_desc.join(", "),
                    if hidden_edges.is_empty() {
                        "<无>".to_string()
                    } else {
                        hidden_edges.join(", ")
                    },
                    face.average_height(),
                    normal_label
                );
            }
        }
    }

    if document.blocks().next().is_some() {
        println!("块定义：");
        for block in document.blocks() {
            println!(
                "  - {} 基点=({:.2}, {:.2}), 实体数={}, 属性数={}",
                block.name,
                block.base_point.x(),
                block.base_point.y(),
                block.entities.len(),
                block.attributes.len()
            );
        }
    }
}

fn dispatch_cli_command(
    bus: &CommandBus,
    name: &str,
    context: &mut CommandContext<'_>,
) -> Result<(), String> {
    let request = CommandRequest {
        name: name.to_string(),
        args: Vec::new(),
    };
    let response = bus.dispatch(&request, context);
    if response.success {
        if let Some(message) = response.message {
            println!("[命令] {message}");
        }
        Ok(())
    } else {
        Err(response.message.unwrap_or_else(|| "未知错误".to_string()))
    }
}

fn describe_hatch_edge(edge: &HatchEdge) -> String {
    match edge {
        HatchEdge::Line { start, end } => {
            format!("Line {} -> {}", format_point(*start), format_point(*end))
        }
        HatchEdge::Arc {
            center,
            radius,
            start_angle,
            end_angle,
            is_counter_clockwise,
        } => format!(
            "Arc center={}, r={:.2}, start={:.1}°, end={:.1}°, ccw={}",
            format_point(*center),
            radius,
            start_angle.to_degrees(),
            end_angle.to_degrees(),
            if *is_counter_clockwise { "是" } else { "否" }
        ),
        HatchEdge::PolylineSegment { start, end, bulge } => format!(
            "Polyline {} -> {} (bulge={:.3})",
            format_point(*start),
            format_point(*end),
            bulge
        ),
        HatchEdge::Ellipse {
            center,
            major_axis,
            minor_ratio,
            start_angle,
            end_angle,
            is_counter_clockwise,
        } => format!(
            "Ellipse center={}, major={}, ratio={:.3}, start={:.1}°, end={:.1}°, ccw={}",
            format_point(*center),
            format_vector(*major_axis),
            minor_ratio,
            start_angle.to_degrees(),
            end_angle.to_degrees(),
            if *is_counter_clockwise { "是" } else { "否" }
        ),
        HatchEdge::BoundaryReference { handle } => {
            format!("Boundary handle={handle}")
        }
        HatchEdge::Spline {
            control_points,
            fit_points,
            knot_values,
            degree,
            is_rational,
            is_periodic,
        } => format!(
            "Spline degree={}, control={}, fit={}, knots={}, rational={}, periodic={}",
            degree,
            control_points.len(),
            fit_points.len(),
            knot_values.len(),
            if *is_rational { "是" } else { "否" },
            if *is_periodic { "是" } else { "否" }
        ),
    }
}

fn format_point(point: Point2) -> String {
    format!("({:.2}, {:.2})", point.x(), point.y())
}

fn format_vector(vec: Vector2) -> String {
    let v = vec.as_vec2();
    format!("({:.2}, {:.2})", v.x, v.y)
}

fn format_point_option(value: Option<Point2>) -> String {
    value
        .map(format_point)
        .unwrap_or_else(|| "<无>".to_string())
}

fn clip_mode_label(mode: ClipMode) -> &'static str {
    match mode {
        ClipMode::Outside => "保留图像内部",
        ClipMode::Inside => "反向裁剪",
    }
}

fn clip_description(options: &RasterImageDisplayOptions, clip: &Option<RasterImageClip>) -> String {
    if options.use_clipping {
        match clip {
            Some(RasterImageClip::Rectangle { mode, .. }) => {
                format!("矩形裁剪 ({})", clip_mode_label(*mode))
            }
            Some(RasterImageClip::Polygon { vertices, mode }) => {
                format!(
                    "多边形裁剪({} 点, {})",
                    vertices.len(),
                    clip_mode_label(*mode)
                )
            }
            None => format!(
                "启用裁剪但未提供边界 ({})",
                clip_mode_label(ClipMode::Outside)
            ),
        }
    } else {
        "未使用裁剪".to_string()
    }
}
