use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;
use zcad_core::document::{
    Attribute, AttributeDefinition, DimensionKind, Document, Entity, HatchEdge, HatchGradient,
    HatchLoop, MLeaderContent, Polyline, PolylineVertex, RasterImageClip,
    RasterImageDisplayOptions,
};
use zcad_core::geometry::{Point2, Point3, Vector2};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct GoldenDocument {
    layers: Vec<GoldenLayer>,
    entities: Vec<GoldenEntity>,
    blocks: Vec<GoldenBlock>,
    #[serde(default)]
    image_definitions: Vec<GoldenImageDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    image_dictionary: Option<GoldenImageDictionary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    raster_image_variables: Option<GoldenRasterVariables>,
    #[serde(default)]
    image_def_reactors: Vec<GoldenImageDefReactor>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct GoldenLayer {
    name: String,
    is_visible: bool,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct GoldenEntity {
    id: u64,
    kind: String,
    layer: String,
    data: Value,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct GoldenBlock {
    name: String,
    base_point: [f64; 2],
    entities: Vec<GoldenEntityNoId>,
    attributes: Vec<Value>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct GoldenEntityNoId {
    kind: String,
    layer: String,
    data: Value,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct GoldenImageDef {
    handle: String,
    name: Option<String>,
    file_path: String,
    image_size_pixels: Option<[f64; 2]>,
    pixel_size: Option<[f64; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    resolved_path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct GoldenImageDictionary {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    handle: Option<String>,
    #[serde(default)]
    entries: Vec<GoldenImageDictionaryEntry>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct GoldenImageDictionaryEntry {
    name: String,
    image_def_handle: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reactor_handle: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct GoldenRasterVariables {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    handle: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    class_version: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    frame: Option<i16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    quality: Option<i16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    units: Option<i16>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct GoldenImageDefReactor {
    handle: String,
    class_version: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    owner_handle: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    image_handle: Option<String>,
}

pub fn assert_golden(name: &str, document: &Document) {
    let snapshot = GoldenDocument::from_document(document);
    let base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/golden");
    if let Err(err) = fs::create_dir_all(&base_dir) {
        panic!("无法创建黄金数据目录 {}: {err}", base_dir.display());
    }
    let golden_path = base_dir.join(format!("{name}.json"));
    let serialized = serde_json::to_string_pretty(&snapshot).expect("序列化黄金快照失败");

    if !golden_path.exists() {
        fs::write(&golden_path, &serialized)
            .unwrap_or_else(|err| panic!("写入黄金文件 {} 失败: {err}", golden_path.display()));
        panic!(
            "黄金文件 {} 不存在，已自动生成。请确认内容后重新运行测试。",
            golden_path.display()
        );
    }

    let expected_str = fs::read_to_string(&golden_path)
        .unwrap_or_else(|err| panic!("读取黄金文件 {} 失败: {err}", golden_path.display()));
    let expected: GoldenDocument = serde_json::from_str(&expected_str)
        .unwrap_or_else(|err| panic!("解析黄金文件 {} 失败: {err}", golden_path.display()));

    if expected != snapshot {
        let diff_path = base_dir.join(format!("{name}.actual.json"));
        fs::write(&diff_path, &serialized).expect("写入差异文件失败");
        panic!(
            "黄金文件 {} 与当前解析结果不一致。已生成对照输出 {}。",
            golden_path.display(),
            diff_path.display()
        );
    }
}

impl GoldenDocument {
    fn from_document(document: &Document) -> Self {
        let mut layers: Vec<GoldenLayer> = document
            .layers()
            .map(|layer| GoldenLayer {
                name: layer.name.clone(),
                is_visible: layer.is_visible,
            })
            .collect();
        layers.sort_by(|a, b| a.name.cmp(&b.name));

        let mut entities: Vec<GoldenEntity> = document
            .entities()
            .map(|(id, entity)| {
                let (kind, layer, data) = entity_payload(entity);
                GoldenEntity {
                    id: id.get(),
                    kind,
                    layer,
                    data,
                }
            })
            .collect();
        entities.sort_by(|a, b| a.id.cmp(&b.id));

        let mut blocks: Vec<GoldenBlock> = document
            .blocks()
            .map(|block| GoldenBlock {
                name: block.name.clone(),
                base_point: point_to_array(block.base_point),
                entities: block
                    .entities
                    .iter()
                    .map(|entity| {
                        let (kind, layer, data) = entity_payload(entity);
                        GoldenEntityNoId { kind, layer, data }
                    })
                    .collect(),
                attributes: block
                    .attributes
                    .iter()
                    .map(attribute_definition_to_value)
                    .collect(),
            })
            .collect();
        blocks.sort_by(|a, b| a.name.cmp(&b.name));

        let mut image_definitions: Vec<GoldenImageDef> = document
            .raster_image_definitions()
            .map(|(handle, def)| GoldenImageDef {
                handle: handle.clone(),
                name: def.name.clone(),
                file_path: def.file_path.clone(),
                image_size_pixels: def.image_size_pixels.map(|vec| [vec.x(), vec.y()]),
                pixel_size: def.pixel_size.map(|vec| [vec.x(), vec.y()]),
                resolved_path: def.resolved_path.clone(),
            })
            .collect();
        image_definitions.sort_by(|a, b| a.handle.cmp(&b.handle));

        let image_dictionary = document.image_dictionary().map(|dict| {
            let mut entries: Vec<GoldenImageDictionaryEntry> = dict
                .entries
                .iter()
                .map(|entry| GoldenImageDictionaryEntry {
                    name: entry.name.clone(),
                    image_def_handle: entry.image_def_handle.clone(),
                    reactor_handle: entry.reactor_handle.clone(),
                })
                .collect();
            entries.sort_by(|a, b| a.name.cmp(&b.name));
            GoldenImageDictionary {
                handle: dict.handle.clone(),
                entries,
            }
        });

        let raster_image_variables =
            document
                .raster_image_variables()
                .map(|vars| GoldenRasterVariables {
                    handle: vars.handle.clone(),
                    class_version: vars.class_version,
                    frame: vars.frame,
                    quality: vars.quality,
                    units: vars.units,
                });

        let mut image_def_reactors: Vec<GoldenImageDefReactor> = document
            .image_def_reactors()
            .map(|(handle, reactor)| GoldenImageDefReactor {
                handle: handle.clone(),
                class_version: reactor.class_version,
                owner_handle: reactor.owner_handle.clone(),
                image_handle: reactor.image_handle.clone(),
            })
            .collect();
        image_def_reactors.sort_by(|a, b| a.handle.cmp(&b.handle));

        Self {
            layers,
            entities,
            blocks,
            image_definitions,
            image_dictionary,
            raster_image_variables,
            image_def_reactors,
        }
    }
}

fn entity_payload(entity: &Entity) -> (String, String, Value) {
    match entity {
        Entity::Line(line) => (
            "LINE".to_string(),
            line.layer.clone(),
            json!({
                "start": point_to_array(line.start),
                "end": point_to_array(line.end)
            }),
        ),
        Entity::Circle(circle) => (
            "CIRCLE".to_string(),
            circle.layer.clone(),
            json!({
                "center": point_to_array(circle.center),
                "radius": circle.radius
            }),
        ),
        Entity::Arc(arc) => (
            "ARC".to_string(),
            arc.layer.clone(),
            json!({
                "center": point_to_array(arc.center),
                "radius": arc.radius,
                "start_angle": arc.start_angle,
                "end_angle": arc.end_angle
            }),
        ),
        Entity::Ellipse(ellipse) => (
            "ELLIPSE".to_string(),
            ellipse.layer.clone(),
            json!({
                "center": point_to_array(ellipse.center),
                "major_axis": vector_to_array(ellipse.major_axis),
                "ratio": ellipse.ratio,
                "start_parameter": ellipse.start_parameter,
                "end_parameter": ellipse.end_parameter
            }),
        ),
        Entity::Polyline(polyline) => (
            "LWPOLYLINE".to_string(),
            polyline.layer.clone(),
            polyline_to_value(polyline),
        ),
        Entity::Spline(spline) => (
            "SPLINE".to_string(),
            spline.layer.clone(),
            json!({
                "degree": spline.degree,
                "is_rational": spline.is_rational,
                "is_closed": spline.is_closed,
                "is_periodic": spline.is_periodic,
                "control_points": spline
                    .control_points
                    .iter()
                    .map(|point| point_to_array(*point))
                    .collect::<Vec<_>>(),
                "fit_points": spline
                    .fit_points
                    .iter()
                    .map(|point| point_to_array(*point))
                    .collect::<Vec<_>>(),
                "knot_values": spline.knot_values,
                "weights": spline.weights,
                "start_tangent": spline.start_tangent.map(vector_to_array),
                "end_tangent": spline.end_tangent.map(vector_to_array)
            }),
        ),
        Entity::Text(text) => (
            "TEXT".to_string(),
            text.layer.clone(),
            json!({
                "insert": point_to_array(text.insert),
                "content": text.content,
                "height": text.height,
                "rotation": text.rotation
            }),
        ),
        Entity::MText(mtext) => (
            "MTEXT".to_string(),
            mtext.layer.clone(),
            json!({
                "insert": point_to_array(mtext.insert),
                "content": mtext.content,
                "height": mtext.height,
                "reference_width": mtext.reference_width,
                "direction": vector_to_array(mtext.direction),
                "attachment_point": mtext.attachment_point,
                "drawing_direction": mtext.drawing_direction,
                "style": mtext.style
            }),
        ),
        Entity::BlockReference(reference) => (
            "INSERT".to_string(),
            reference.layer.clone(),
            json!({
                "name": reference.name,
                "insert": point_to_array(reference.insert),
                "scale": vector_to_array(reference.scale),
                "rotation": reference.rotation,
                "attributes": reference.attributes.iter().map(attribute_to_value).collect::<Vec<_>>()
            }),
        ),
        Entity::Hatch(hatch) => (
            "HATCH".to_string(),
            hatch.layer.clone(),
            json!({
                "pattern": hatch.pattern_name,
                "is_solid": hatch.is_solid,
                "gradient": hatch.gradient.as_ref().map(hatch_gradient_to_value),
                "loops": hatch.loops.iter().map(hatch_loop_to_value).collect::<Vec<_>>()
            }),
        ),
        Entity::Dimension(dimension) => (
            "DIMENSION".to_string(),
            dimension.layer.clone(),
            json!({
                "kind": dimension_kind_to_string(dimension.kind),
                "definition_point": point_to_array(dimension.definition_point),
                "text_midpoint": point_to_array(dimension.text_midpoint),
                "dimension_line_point": dimension.dimension_line_point.map(point_to_array),
                "extension_line_origin": dimension.extension_line_origin.map(point_to_array),
                "extension_line_end": dimension.extension_line_end.map(point_to_array),
                "secondary_point": dimension.secondary_point.map(point_to_array),
                "arc_definition_point": dimension.arc_definition_point.map(point_to_array),
                "center_point": dimension.center_point.map(point_to_array),
                "text": dimension.text,
                "measurement": dimension.measurement,
                "rotation": dimension.rotation,
                "text_rotation": dimension.text_rotation,
                "oblique_angle": dimension.oblique_angle
            }),
        ),
        Entity::Leader(leader) => (
            "LEADER".to_string(),
            leader.layer.clone(),
            json!({
                "style_name": leader.style_name,
                "has_arrowhead": leader.has_arrowhead,
                "vertices": leader
                    .vertices
                    .iter()
                    .map(|point| point_to_array(*point))
                    .collect::<Vec<_>>()
            }),
        ),
        Entity::MLeader(mleader) => (
            "MULTILEADER".to_string(),
            mleader.layer.clone(),
            json!({
                "style_name": mleader.style_name,
                "text_height": mleader.text_height,
                "scale": mleader.scale,
                "has_dogleg": mleader.has_dogleg,
                "dogleg_length": mleader.dogleg_length,
                "landing_gap": mleader.landing_gap,
                "leader_lines": mleader
                    .leader_lines
                    .iter()
                    .map(|line| {
                        json!({
                            "vertices": line
                                .vertices
                                .iter()
                                .map(|point| point_to_array(*point))
                                .collect::<Vec<_>>()
                        })
                    })
                    .collect::<Vec<_>>(),
                "content": match &mleader.content {
                    MLeaderContent::MText { text, location } => json!({
                        "kind": "mtext",
                        "text": text,
                        "location": point_to_array(*location),
                    }),
                    MLeaderContent::Block { block } => json!({
                        "kind": "block",
                        "block_handle": block.block_handle,
                        "block_name": block.block_name,
                        "location": point_to_array(block.location),
                        "scale": vector_to_array(block.scale),
                        "rotation": block.rotation,
                        "connection_type": block.connection_type,
                    }),
                    MLeaderContent::None => json!({
                        "kind": "none",
                    }),
                }
            }),
        ),
        Entity::RasterImage(image) => (
            "IMAGE".to_string(),
            image.layer.clone(),
            json!({
                "image_def_handle": image.image_def_handle,
                "image_def_reactor_handle": image.image_def_reactor_handle,
                "insert": point_to_array(image.insert),
                "u_vector": vector_to_array(image.u_vector),
                "v_vector": vector_to_array(image.v_vector),
                "image_size": vector_to_array(image.image_size),
                "display_options": display_options_to_value(&image.display_options),
                "clip": clip_to_value(&image.clip),
            }),
        ),
        Entity::Wipeout(wipeout) => (
            "WIPEOUT".to_string(),
            wipeout.layer.clone(),
            json!({
                "insert": point_to_array(wipeout.insert),
                "u_vector": vector_to_array(wipeout.u_vector),
                "v_vector": vector_to_array(wipeout.v_vector),
                "image_size": vector_to_array(wipeout.image_size),
                "display_options": display_options_to_value(&wipeout.display_options),
                "clip": clip_to_value(&wipeout.clip),
            }),
        ),
        Entity::Face3D(face) => (
            "3DFACE".to_string(),
            face.layer.clone(),
            json!({
                "vertices": face
                    .vertices
                    .iter()
                    .map(|vertex| point3_to_array(*vertex))
                    .collect::<Vec<_>>(),
                "invisible_edges": face.invisible_edges,
            }),
        ),
    }
}

fn hatch_loop_to_value(loop_path: &HatchLoop) -> Value {
    json!({
        "is_polyline": loop_path.is_polyline,
        "is_closed": loop_path.is_closed,
        "boundary_handles": loop_path.boundary_handles.clone(),
        "edges": loop_path.edges.iter().map(hatch_edge_to_value).collect::<Vec<_>>()
    })
}

fn display_options_to_value(options: &RasterImageDisplayOptions) -> Value {
    json!({
        "show_image": options.show_image,
        "show_border": options.show_border,
        "use_clipping": options.use_clipping,
        "brightness": options.brightness,
        "contrast": options.contrast,
        "fade": options.fade,
    })
}

fn clip_to_value(clip: &Option<RasterImageClip>) -> Value {
    match clip {
        Some(RasterImageClip::Rectangle { min, max, mode }) => json!({
            "kind": "rectangle",
            "min": point_to_array(*min),
            "max": point_to_array(*max),
            "mode": mode.describe(),
        }),
        Some(RasterImageClip::Polygon { vertices, mode }) => json!({
            "kind": "polygon",
            "vertices": vertices
                .iter()
                .map(|vertex| point_to_array(*vertex))
                .collect::<Vec<_>>(),
            "mode": mode.describe(),
        }),
        None => Value::Null,
    }
}

fn hatch_edge_to_value(edge: &HatchEdge) -> Value {
    match edge {
        HatchEdge::Line { start, end } => json!({
            "type": "Line",
            "start": point_to_array(*start),
            "end": point_to_array(*end)
        }),
        HatchEdge::Arc {
            center,
            radius,
            start_angle,
            end_angle,
            is_counter_clockwise,
        } => json!({
            "type": "Arc",
            "center": point_to_array(*center),
            "radius": radius,
            "start_angle": start_angle,
            "end_angle": end_angle,
            "ccw": is_counter_clockwise
        }),
        HatchEdge::PolylineSegment { start, end, bulge } => json!({
            "type": "PolylineSegment",
            "start": point_to_array(*start),
            "end": point_to_array(*end),
            "bulge": bulge
        }),
        HatchEdge::Ellipse {
            center,
            major_axis,
            minor_ratio,
            start_angle,
            end_angle,
            is_counter_clockwise,
        } => json!({
            "type": "Ellipse",
            "center": point_to_array(*center),
            "major_axis": vector_to_array(*major_axis),
            "minor_ratio": minor_ratio,
            "start_angle": start_angle,
            "end_angle": end_angle,
            "ccw": is_counter_clockwise
        }),
        HatchEdge::BoundaryReference { handle } => json!({
            "type": "BoundaryReference",
            "handle": handle
        }),
        HatchEdge::Spline {
            control_points,
            fit_points,
            knot_values,
            degree,
            is_rational,
            is_periodic,
        } => json!({
            "type": "Spline",
            "degree": degree,
            "is_rational": is_rational,
            "is_periodic": is_periodic,
            "control_points": control_points
                .iter()
                .map(|point| point_to_array(*point))
                .collect::<Vec<_>>(),
            "fit_points": fit_points
                .iter()
                .map(|point| point_to_array(*point))
                .collect::<Vec<_>>(),
            "knot_values": knot_values,
        }),
    }
}

fn hatch_gradient_to_value(gradient: &HatchGradient) -> Value {
    json!({
        "name": gradient.name,
        "angle": gradient.angle,
        "shift": gradient.shift,
        "tint": gradient.tint,
        "is_single_color": gradient.is_single_color,
        "color1": gradient.color1,
        "color2": gradient.color2
    })
}

fn dimension_kind_to_string(kind: DimensionKind) -> String {
    match kind {
        DimensionKind::Linear => "Linear".to_string(),
        DimensionKind::Aligned => "Aligned".to_string(),
        DimensionKind::Angular => "Angular".to_string(),
        DimensionKind::Diameter => "Diameter".to_string(),
        DimensionKind::Radius => "Radius".to_string(),
        DimensionKind::Angular3Point => "Angular3Point".to_string(),
        DimensionKind::Ordinate => "Ordinate".to_string(),
        DimensionKind::Unknown(code) => format!("Unknown({code})"),
    }
}

fn polyline_to_value(polyline: &Polyline) -> Value {
    let vertices: Vec<Value> = polyline
        .vertices
        .iter()
        .map(|vertex| polyline_vertex_to_value(vertex))
        .collect();
    json!({
        "is_closed": polyline.is_closed,
        "vertices": vertices
    })
}

fn polyline_vertex_to_value(vertex: &PolylineVertex) -> Value {
    json!({
        "position": point_to_array(vertex.position),
        "bulge": vertex.bulge
    })
}

fn attribute_to_value(attr: &Attribute) -> Value {
    json!({
        "tag": attr.tag,
        "text": attr.text,
        "layer": attr.layer,
        "insert": point_to_array(attr.insert),
        "height": attr.height,
        "rotation": attr.rotation,
        "width_factor": attr.width_factor,
        "oblique": attr.oblique,
        "style": attr.style,
        "prompt": attr.prompt,
        "alignment": attr.alignment.as_ref().map(|pt| point_to_array(*pt)),
        "horizontal_align": attr.horizontal_align,
        "vertical_align": attr.vertical_align,
        "line_spacing_factor": attr.line_spacing_factor,
        "line_spacing_style": attr.line_spacing_style,
        "is_invisible": attr.is_invisible,
        "is_constant": attr.is_constant,
        "is_verify": attr.is_verify,
        "is_preset": attr.is_preset,
        "lock_position": attr.lock_position
    })
}

fn attribute_definition_to_value(attr: &AttributeDefinition) -> Value {
    json!({
        "tag": attr.tag,
        "prompt": attr.prompt,
        "default_text": attr.default_text,
        "layer": attr.layer,
        "insert": point_to_array(attr.insert),
        "height": attr.height,
        "rotation": attr.rotation,
        "width_factor": attr.width_factor,
        "oblique": attr.oblique,
        "style": attr.style,
        "alignment": attr.alignment.as_ref().map(|pt| point_to_array(*pt)),
        "horizontal_align": attr.horizontal_align,
        "vertical_align": attr.vertical_align,
        "line_spacing_factor": attr.line_spacing_factor,
        "line_spacing_style": attr.line_spacing_style,
        "is_invisible": attr.is_invisible,
        "is_constant": attr.is_constant,
        "is_verify": attr.is_verify,
        "is_preset": attr.is_preset,
        "lock_position": attr.lock_position
    })
}

fn point_to_array(point: Point2) -> [f64; 2] {
    [point.x(), point.y()]
}

fn point3_to_array(point: Point3) -> [f64; 3] {
    [point.x(), point.y(), point.z()]
}

fn vector_to_array(vector: Vector2) -> [f64; 2] {
    let v = vector.as_vec2();
    [v.x, v.y]
}
