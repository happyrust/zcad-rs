mod golden;

use std::f64::consts::{PI, TAU};
use std::path::PathBuf;

use glam::DVec2;

use golden::assert_golden;
use zcad_core::{
    document::{
        ClipMode, DimensionKind, Entity, HatchEdge, HatchLoop, MLeaderContent, RasterImageClip,
    },
    geometry::{Point2, Vector2},
};
use zcad_io::{DocumentLoader, DxfFacade};

#[test]
fn load_basic_entities_matches_expected_document() {
    let mut fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fixtures.push("tests/data/basic_entities.dxf");

    let loader = DxfFacade::new();
    let doc = loader.load(&fixtures).expect("读取 DXF 失败");
    assert_golden("basic_entities", &doc);
}

#[test]
fn load_polyline_with_bulge_preserves_value() {
    let mut fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fixtures.push("tests/data/bulge_polyline.dxf");

    let loader = DxfFacade::new();
    let doc = loader.load(&fixtures).expect("读取带 bulge 的 DXF 失败");
    assert_golden("bulge_polyline", &doc);

    let mut polylines = doc.entities().filter_map(|(_, entity)| match entity {
        Entity::Polyline(polyline) => Some(polyline),
        _ => None,
    });

    let polyline = polylines.next().expect("未找到多段线实体");
    assert!(polylines.next().is_none(), "期望仅有一个多段线实体");
    assert_eq!(polyline.vertices.len(), 2);

    let first = &polyline.vertices[0];
    let second = &polyline.vertices[1];

    assert!((first.position.x() - 0.0).abs() < 1e-9);
    assert!((first.position.y() - 0.0).abs() < 1e-9);
    assert!((first.bulge - 1.0).abs() < 1e-9);
    assert!((second.position.x() - 10.0).abs() < 1e-9);
    assert!((second.position.y() - 0.0).abs() < 1e-9);
    assert!(second.bulge.abs() < 1e-9);
}

#[test]
fn load_mtext_entity() {
    let mut fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fixtures.push("tests/data/mtext_basic.dxf");

    let loader = DxfFacade::new();
    let doc = loader.load(&fixtures).expect("读取 MTEXT DXF 失败");
    assert_golden("mtext_basic", &doc);

    let mut mtexts = doc.entities().filter_map(|(_, entity)| match entity {
        Entity::MText(mtext) => Some(mtext),
        _ => None,
    });

    let mtext = mtexts.next().expect("未找到 MText 实体");
    assert!(mtexts.next().is_none(), "期望仅存在一个 MText 实体");

    assert!((mtext.insert.x() - 5.0).abs() < 1e-9);
    assert!((mtext.insert.y() - 6.0).abs() < 1e-9);
    assert_eq!(mtext.content, "Line1\nLine2");
    assert!((mtext.height - 2.5).abs() < 1e-9);
    assert!(mtext.reference_width.is_none());
    let dir = mtext.direction.as_vec2();
    assert!((dir.x - 1.0).abs() < 1e-9);
    assert!(dir.y.abs() < 1e-9);
    assert_eq!(mtext.attachment_point, 5);
    assert_eq!(mtext.drawing_direction, 1);
    assert_eq!(mtext.style.as_deref(), Some("Standard"));
    assert_eq!(mtext.layer, "ANNOT");
}

#[test]
fn load_ellipse_entity() {
    let mut fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fixtures.push("tests/data/ellipse_basic.dxf");

    let loader = DxfFacade::new();
    let doc = loader.load(&fixtures).expect("读取 ELLIPSE DXF 失败");
    assert_golden("ellipse_basic", &doc);

    let mut ellipses = doc.entities().filter_map(|(_, entity)| match entity {
        Entity::Ellipse(ellipse) => Some(ellipse),
        _ => None,
    });

    let ellipse = ellipses.next().expect("未找到椭圆实体");
    assert!(ellipses.next().is_none(), "期望仅有一个椭圆实体");

    assert!((ellipse.center.x() - 10.0).abs() < 1e-9);
    assert!((ellipse.center.y() - 5.0).abs() < 1e-9);
    let axis = ellipse.major_axis.as_vec2();
    assert!((axis.x - 6.0).abs() < 1e-9);
    assert!((axis.y - 2.0).abs() < 1e-9);
    assert!((ellipse.ratio - 0.5).abs() < 1e-9);
    assert!(ellipse.start_parameter.abs() < 1e-9);
    assert!((ellipse.end_parameter - PI).abs() < 1e-9);
    assert_eq!(ellipse.layer, "GEOM");
}

#[test]
fn load_spline_entity() {
    let mut fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fixtures.push("tests/data/spline_basic.dxf");

    let loader = DxfFacade::new();
    let doc = loader.load(&fixtures).expect("读取 SPLINE DXF 失败");
    assert_golden("spline_basic", &doc);

    let mut splines = doc.entities().filter_map(|(_, entity)| match entity {
        Entity::Spline(spline) => Some(spline),
        _ => None,
    });

    let spline = splines.next().expect("未找到样条实体");
    assert!(splines.next().is_none(), "期望仅有一个样条实体");

    assert_eq!(spline.degree, 3);
    assert!(!spline.is_closed);
    assert!(!spline.is_periodic);
    assert!(!spline.is_rational);
    assert_eq!(spline.control_points.len(), 4);
    assert_eq!(spline.fit_points.len(), 2);
    assert_eq!(spline.knot_values.len(), 8);
    assert_eq!(spline.weights.len(), 4);

    let start = spline.start_tangent.expect("缺少起始切向量").as_vec2();
    assert!((start.x - 1.0).abs() < 1e-9);
    assert!(start.y.abs() < 1e-9);
    let end = spline.end_tangent.expect("缺少终止切向量").as_vec2();
    assert!((end.x + 1.0).abs() < 1e-9);
    assert!(end.y.abs() < 1e-9);

    assert_eq!(spline.layer, "GEOM");
}

#[test]
fn load_block_definition_and_insert() {
    let mut fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fixtures.push("tests/data/block_insert.dxf");

    let loader = DxfFacade::new();
    let doc = loader
        .load(&fixtures)
        .expect("读取含 BLOCK/INSERT 的 DXF 失败");
    assert_golden("block_insert", &doc);

    let block = doc.block("MYBLOCK").expect("未找到块定义 MYBLOCK");
    assert_eq!(block.base_point.x(), 0.0);
    assert_eq!(block.base_point.y(), 0.0);
    assert_eq!(block.entities.len(), 1);
    assert_eq!(block.attributes.len(), 1);
    match &block.entities[0] {
        Entity::Line(line) => {
            assert!((line.start.x() - 0.0).abs() < 1e-9);
            assert!((line.end.x() - 5.0).abs() < 1e-9);
        }
        other => panic!("期望块内为线段，实际为 {:?}", other),
    }
    let def_attr = &block.attributes[0];
    assert_eq!(def_attr.tag, "TAG");
    assert_eq!(def_attr.default_text, "DEF");
    assert_eq!(def_attr.prompt.as_deref(), Some("Attribute prompt"));
    assert!(def_attr.is_invisible);
    assert!(def_attr.is_preset);
    assert!(def_attr.lock_position);

    let mut inserts = doc.entities().filter_map(|(_, entity)| match entity {
        Entity::BlockReference(reference) => Some(reference),
        _ => None,
    });
    let insert = inserts.next().expect("未找到块参照");
    assert!(inserts.next().is_none(), "期望仅有一个块参照");

    assert_eq!(insert.name, "MYBLOCK");
    assert!((insert.insert.x() - 10.0).abs() < 1e-9);
    assert!((insert.insert.y() - 10.0).abs() < 1e-9);
    let scale = insert.scale.as_vec2();
    assert!((scale.x - 2.0).abs() < 1e-9);
    assert!((scale.y - 2.0).abs() < 1e-9);
    assert!((insert.rotation.to_degrees() - 45.0).abs() < 1e-9);
    assert_eq!(insert.attributes.len(), 1);
    let attr = &insert.attributes[0];
    assert_eq!(attr.tag, "TAG");
    assert_eq!(attr.text, "VALUE");
    assert_eq!(attr.style.as_deref(), Some("Annot"));
    assert_eq!(attr.prompt.as_deref(), Some("Attribute prompt"));
    assert!(attr.is_invisible);
    assert!(attr.is_preset);
    assert!(!attr.is_constant);
    assert!(!attr.is_verify);
    assert!(attr.lock_position);
    assert!((attr.width_factor - 0.75).abs() < 1e-9);
    assert!((attr.rotation.to_degrees() - 15.0).abs() < 1e-9);
    assert!((attr.oblique.to_degrees() - 10.0).abs() < 1e-9);
    let alignment = attr.alignment.expect("alignment point missing");
    assert!((alignment.x() - 12.0).abs() < 1e-9);
    assert!((alignment.y() - 11.0).abs() < 1e-9);
    assert_eq!(attr.horizontal_align, 2);
    assert_eq!(attr.vertical_align, 1);
    assert!((attr.line_spacing_factor - 1.0).abs() < 1e-9);
    assert_eq!(attr.line_spacing_style, 1);
}

#[test]
fn load_3dface_entities() {
    let mut fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fixtures.push("tests/data/face3d_basic.dxf");

    let loader = DxfFacade::new();
    let doc = loader.load(&fixtures).expect("读取包含 3DFACE 的 DXF 失败");
    assert_golden("face3d_basic", &doc);

    let faces: Vec<_> = doc
        .entities()
        .filter_map(|(_, entity)| match entity {
            Entity::Face3D(face) => Some(face),
            _ => None,
        })
        .collect();
    assert_eq!(faces.len(), 2, "样例应仅包含两个 3DFACE");

    let first = &faces[0];
    assert_eq!(first.layer, "MESH");
    assert!((first.vertices[0].x() - 0.0).abs() < 1e-9);
    assert!((first.vertices[0].y() - 0.0).abs() < 1e-9);
    assert!((first.vertices[1].x() - 100.0).abs() < 1e-9);
    assert!((first.vertices[2].z() - 10.0).abs() < 1e-9);
    assert_eq!(
        first.invisible_edges,
        [true, false, true, false],
        "隐藏边标记应按位展开"
    );

    let second = &faces[1];
    assert_eq!(second.layer, "MESH");
    assert_eq!(
        second.invisible_edges,
        [false, true, false, false],
        "第二个样例仅隐藏第二条边"
    );
    assert!((second.vertices[2].x() - 40.0).abs() < 1e-9);
    assert!((second.vertices[2].y() - 40.0).abs() < 1e-9);
    assert!((second.vertices[2].z() - 5.0).abs() < 1e-9);
    assert!(
        (second.vertices[3].x() - second.vertices[2].x()).abs() < 1e-9
            && (second.vertices[3].y() - second.vertices[2].y()).abs() < 1e-9,
        "缺失的第四顶点应自动复用第三个坐标"
    );
}

#[test]
fn load_block_with_multiline_attribute() {
    let mut fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fixtures.push("tests/data/block_multiline.dxf");

    let loader = DxfFacade::new();
    let doc = loader.load(&fixtures).expect("读取含多行属性的 DXF 失败");
    assert_golden("block_multiline", &doc);

    let block = doc.block("ML_BLOCK").expect("未找到块定义 ML_BLOCK");
    assert_eq!(block.attributes.len(), 1);
    let def = &block.attributes[0];
    assert_eq!(def.tag, "MLTAG");
    assert_eq!(def.default_text, "Line1\nLine2");
    assert_eq!(def.prompt.as_deref(), Some("Multiline prompt"));
    assert_eq!(def.horizontal_align, 1);
    assert_eq!(def.vertical_align, 0);
    assert!((def.line_spacing_factor - 1.2).abs() < 1e-9);
    assert_eq!(def.line_spacing_style, 1);
    assert!((def.width_factor - 1.0).abs() < 1e-9);
    assert!((def.rotation.to_degrees()).abs() < 1e-9);
    assert!((def.oblique.to_degrees()).abs() < 1e-9);
    assert!((def.height - 1.5).abs() < 1e-9);

    let mut inserts = doc.entities().filter_map(|(_, entity)| match entity {
        Entity::BlockReference(reference) => Some(reference),
        _ => None,
    });
    let insert = inserts.next().expect("未找到块参照");
    assert_eq!(insert.attributes.len(), 1);
    let attr = &insert.attributes[0];
    assert_eq!(attr.tag, "MLTAG");
    assert_eq!(attr.text, "Custom value");
    assert_eq!(attr.prompt.as_deref(), Some("Multiline prompt"));
    assert_eq!(attr.layer, "0");
    assert!(attr.lock_position);
    assert_eq!(attr.horizontal_align, 1);
    assert_eq!(attr.vertical_align, 0);
    assert!((attr.line_spacing_factor - 1.2).abs() < 1e-9);
    assert_eq!(attr.line_spacing_style, 1);
    assert!((attr.height - 1.5).abs() < 1e-9);
    assert!((attr.width_factor - 1.0).abs() < 1e-9);
    assert!((attr.rotation.to_degrees()).abs() < 1e-9);
    let alignment = attr.alignment.expect("缺少对齐点");
    assert!((alignment.x() - 5.5).abs() < 1e-9);
    assert!((alignment.y() - 6.0).abs() < 1e-9);
}

#[test]
fn load_polyface_mesh_generates_face3d_entities() {
    let mut fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fixtures.push("tests/data/polyface_basic.dxf");

    let loader = DxfFacade::new();
    let doc = loader.load(&fixtures).expect("读取含 POLYFACE 的 DXF 失败");
    assert_golden("polyface_basic", &doc);

    let faces: Vec<_> = doc
        .entities()
        .filter_map(|(_, entity)| match entity {
            Entity::Face3D(face) => Some(face),
            _ => None,
        })
        .collect();
    assert_eq!(faces.len(), 2, "应将两个面记录映射为 3DFACE");
    let first = &faces[0];
    assert_eq!(first.layer, "MESH");
    assert!(first.invisible_edges[3], "第四条边应标记为隐藏");
    let second = &faces[1];
    assert!(
        (second.vertices[2].x() - second.vertices[1].x()).abs() < 1e-9,
        "缺失的顶点应复用上一顶点"
    );
}

#[test]
fn load_polygon_mesh_generates_grid_faces() {
    let mut fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fixtures.push("tests/data/mesh_grid_basic.dxf");

    let loader = DxfFacade::new();
    let doc = loader
        .load(&fixtures)
        .expect("读取含 POLYGON MESH 的 DXF 失败");
    assert_golden("mesh_grid_basic", &doc);

    let faces: Vec<_> = doc
        .entities()
        .filter_map(|(_, entity)| match entity {
            Entity::Face3D(face) => Some(face),
            _ => None,
        })
        .collect();
    assert_eq!(faces.len(), 2, "2x3 网格应生成 2 个四边面");
    assert_eq!(faces[0].layer, "MESH");
    assert!(
        (faces[0].vertices[1].x() - 0.0).abs() < 1e-9
            || (faces[0].vertices[1].x() - 10.0).abs() < 1e-9,
        "面顶点需来源于网格坐标"
    );
}

#[test]
fn load_wrapped_polygon_mesh_generates_loop_faces() {
    let mut fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fixtures.push("tests/data/mesh_wrap_basic.dxf");

    let loader = DxfFacade::new();
    let doc = loader
        .load(&fixtures)
        .expect("读取含闭合 POLYGON MESH 的 DXF 失败");
    assert_golden("mesh_wrap_basic", &doc);

    let faces: Vec<_> = doc
        .entities()
        .filter_map(|(_, entity)| match entity {
            Entity::Face3D(face) => Some(face),
            _ => None,
        })
        .collect();
    assert_eq!(faces.len(), 4, "闭合 2x2 网格应形成 4 个面");
    for face in &faces {
        assert_eq!(face.layer, "MESH");
    }
}

#[test]
fn load_block_with_hatch_and_aligned_attribute() {
    let mut fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fixtures.push("tests/data/block_hatch.dxf");

    let loader = DxfFacade::new();
    let doc = loader.load(&fixtures).expect("读取含 HATCH 的块 DXF 失败");
    assert_golden("block_hatch", &doc);

    let block = doc.block("HATCHBLOCK").expect("未找到块定义 HATCHBLOCK");
    assert_eq!(block.entities.len(), 1, "块应包含一个 HATCH 实体");

    let mut inserts = doc.entities().filter_map(|(_, entity)| match entity {
        Entity::BlockReference(reference) => Some(reference),
        _ => None,
    });
    let insert = inserts.next().expect("文档应包含一个块参照");
    assert!(inserts.next().is_none(), "仅期望一个块参照");

    assert_eq!(insert.attributes.len(), 1, "块参照应带有属性");
    let attr = &insert.attributes[0];
    assert_eq!(attr.horizontal_align, 2);
    assert_eq!(attr.vertical_align, 3);
    assert_eq!(attr.text, "VALUE");
    let alignment = attr.alignment.expect("块属性应提供对齐点");
    assert!((alignment.x() - 6.0).abs() < 1e-9);
    assert!((alignment.y() - 6.0).abs() < 1e-9);
}

#[test]
fn load_block_with_gradient_hatch_and_multiple_attributes() {
    let mut fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fixtures.push("tests/data/block_hatch_gradient.dxf");

    let loader = DxfFacade::new();
    let doc = loader
        .load(&fixtures)
        .expect("读取含渐变 HATCH 块的 DXF 失败");
    assert_golden("block_hatch_gradient", &doc);

    let block = doc.block("GRADBLOCK").expect("未找到块定义 GRADBLOCK");
    assert_eq!(block.entities.len(), 1);

    let gradient_hatch = match &block.entities[0] {
        Entity::Hatch(hatch) => hatch,
        other => panic!("期望 HATCH，实际为 {:?}", other),
    };
    assert!(
        gradient_hatch.gradient.is_some(),
        "块内 HATCH 应包含渐变信息"
    );

    let mut inserts = doc.entities().filter_map(|(_, entity)| match entity {
        Entity::BlockReference(reference) => Some(reference),
        _ => None,
    });
    let insert = inserts.next().expect("应存在一个渐变块参照");
    assert!(inserts.next().is_none(), "仅期望一个渐变块参照");

    assert_eq!(insert.attributes.len(), 2, "块参照应包含两个属性");
    let first = &insert.attributes[0];
    assert_eq!(first.horizontal_align, 1);
    assert_eq!(first.vertical_align, 0);
    assert_eq!(first.text, "P1");

    let second = &insert.attributes[1];
    assert_eq!(second.horizontal_align, 2);
    assert_eq!(second.vertical_align, 2);
    assert_eq!(second.text, "S2");
}

#[test]
fn load_solid_hatch_entity() {
    let mut fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fixtures.push("tests/data/hatch_simple.dxf");

    let loader = DxfFacade::new();
    let doc = loader.load(&fixtures).expect("读取含 HATCH 的 DXF 失败");
    assert_golden("hatch_simple", &doc);

    let mut hatches = doc.entities().filter_map(|(_, entity)| match entity {
        Entity::Hatch(hatch) => Some(hatch),
        _ => None,
    });
    let hatch = hatches.next().expect("未找到 HATCH 实体");
    assert!(hatches.next().is_none(), "期望仅有一个 HATCH 实体");
    assert_eq!(hatch.pattern_name, "SOLID");
    assert!(hatch.is_solid);
    assert_eq!(hatch.loops.len(), 1);
    let loop_path = &hatch.loops[0];
    assert!(loop_path.is_polyline);
    assert!(hatch.gradient.is_none());
    let vertices = extract_loop_vertices(loop_path).expect("无法从 HATCH 边界重建多段线顶点");
    assert!(vertices.len() >= 4);
    let area = compute_polygon_area(&vertices);
    assert!(
        (area - 24.0).abs() < 0.05,
        "填充边界面积应为 24.0，实际为 {area}"
    );
}

#[test]
fn load_linear_dimension_entity() {
    let mut fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fixtures.push("tests/data/dimension_linear.dxf");

    let loader = DxfFacade::new();
    let doc = loader
        .load(&fixtures)
        .expect("读取含 DIMENSION 的 DXF 失败");
    assert_golden("dimension_linear", &doc);

    let mut dimensions = doc.entities().filter_map(|(_, entity)| match entity {
        Entity::Dimension(dimension) => Some(dimension),
        _ => None,
    });
    let dimension = dimensions.next().expect("未找到 DIMENSION 实体");
    assert!(dimensions.next().is_none(), "期望仅有一个 DIMENSION 实体");
    assert!(matches!(dimension.kind, DimensionKind::Linear));
    assert!((dimension.definition_point.x() - 0.0).abs() < 1e-9);
    assert!((dimension.text_midpoint.x() - 4.0).abs() < 1e-9);
    assert_eq!(dimension.text.as_deref(), Some("50"));
    assert!((dimension.rotation.to_degrees()).abs() < 1e-9);
    assert_eq!(dimension.measurement, Some(50.0));
    assert_eq!(
        dimension.dimension_line_point.map(|pt| (pt.x(), pt.y())),
        Some((4.0, 0.0))
    );
    assert_eq!(
        dimension.extension_line_origin.map(|pt| (pt.x(), pt.y())),
        Some((0.0, 0.0))
    );
    assert_eq!(
        dimension.extension_line_end.map(|pt| (pt.x(), pt.y())),
        Some((4.0, 0.0))
    );
    assert!(dimension.secondary_point.is_none());
    assert!(dimension.arc_definition_point.is_none());
    assert!(dimension.center_point.is_none());
    assert!(dimension.text_rotation.is_none());
    assert!(dimension.oblique_angle.is_none());
}

#[test]
fn load_diameter_dimension_entity() {
    let mut fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fixtures.push("tests/data/dimension_diameter.dxf");

    let loader = DxfFacade::new();
    let doc = loader.load(&fixtures).expect("读取直径尺寸 DXF 失败");
    assert_golden("dimension_diameter", &doc);

    let mut dimensions = doc.entities().filter_map(|(_, entity)| match entity {
        Entity::Dimension(dimension) => Some(dimension),
        _ => None,
    });
    let dimension = dimensions.next().expect("未找到 DIMENSION 实体");
    assert!(dimensions.next().is_none());
    assert!(matches!(dimension.kind, DimensionKind::Diameter));
    assert_eq!(dimension.center_point, Some(Point2::new(2.0, 0.0)));
    assert_eq!(dimension.text.as_deref(), Some("20"));
    assert_eq!(dimension.measurement, Some(20.0));
}

#[test]
fn load_angular_dimension_entity() {
    let mut fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fixtures.push("tests/data/dimension_angular.dxf");

    let loader = DxfFacade::new();
    let doc = loader.load(&fixtures).expect("读取角度尺寸 DXF 失败");
    assert_golden("dimension_angular", &doc);

    let mut dimensions = doc.entities().filter_map(|(_, entity)| match entity {
        Entity::Dimension(dimension) => Some(dimension),
        _ => None,
    });
    let dimension = dimensions.next().expect("未找到 DIMENSION 实体");
    assert!(dimensions.next().is_none());
    assert!(matches!(dimension.kind, DimensionKind::Angular));
    assert_eq!(dimension.measurement, Some(90.0));
    assert_eq!(dimension.secondary_point, Some(Point2::new(1.0, 0.0)));
    assert_eq!(dimension.arc_definition_point, Some(Point2::new(1.0, 1.0)));
}

#[test]
fn load_radius_dimension_entity() {
    let mut fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fixtures.push("tests/data/dimension_radius.dxf");

    let loader = DxfFacade::new();
    let doc = loader.load(&fixtures).expect("读取半径尺寸 DXF 失败");
    assert_golden("dimension_radius", &doc);

    let mut dimensions = doc.entities().filter_map(|(_, entity)| match entity {
        Entity::Dimension(dimension) => Some(dimension),
        _ => None,
    });
    let dimension = dimensions.next().expect("未找到 DIMENSION 实体");
    assert!(dimensions.next().is_none());
    assert!(matches!(dimension.kind, DimensionKind::Radius));
    assert_eq!(dimension.center_point, Some(Point2::new(2.0, 0.0)));
    assert_eq!(dimension.text.as_deref(), Some("R10"));
    assert_eq!(dimension.measurement, Some(10.0));
}

#[test]
fn load_angular_three_point_dimension_entity() {
    let mut fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fixtures.push("tests/data/dimension_angular3pt.dxf");

    let loader = DxfFacade::new();
    let doc = loader.load(&fixtures).expect("读取三点角度尺寸 DXF 失败");
    assert_golden("dimension_angular3pt", &doc);

    let mut dimensions = doc.entities().filter_map(|(_, entity)| match entity {
        Entity::Dimension(dimension) => Some(dimension),
        _ => None,
    });
    let dimension = dimensions.next().expect("未找到 DIMENSION 实体");
    assert!(dimensions.next().is_none());
    assert!(matches!(dimension.kind, DimensionKind::Angular3Point));
    assert_eq!(dimension.measurement, Some(45.0));
    assert_eq!(dimension.secondary_point, Some(Point2::new(1.0, 0.0)));
    assert_eq!(dimension.arc_definition_point, Some(Point2::new(1.0, 1.0)));
}

#[test]
fn load_ellipse_hatch_entity() {
    let mut fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fixtures.push("tests/data/hatch_ellipse.dxf");

    let loader = DxfFacade::new();
    let doc = loader
        .load(&fixtures)
        .expect("读取含椭圆边界 HATCH 的 DXF 失败");
    assert_golden("hatch_ellipse", &doc);

    let mut hatches = doc.entities().filter_map(|(_, entity)| match entity {
        Entity::Hatch(hatch) => Some(hatch),
        _ => None,
    });
    let hatch = hatches.next().expect("未找到 HATCH 实体");
    assert!(hatches.next().is_none(), "期望仅有一个 HATCH 实体");
    assert_eq!(hatch.loops.len(), 1);
    let loop_path = &hatch.loops[0];
    assert_eq!(loop_path.edges.len(), 1);
    match &loop_path.edges[0] {
        HatchEdge::Ellipse {
            center,
            minor_ratio,
            ..
        } => {
            assert!((center.x() - 5.0).abs() < 1e-9);
            assert!((center.y() - 5.0).abs() < 1e-9);
            assert!((minor_ratio - 0.5).abs() < 1e-9);
        }
        other => panic!("期望椭圆边界，实际为 {other:?}"),
    }

    let vertices = extract_loop_vertices(loop_path).expect("无法采样椭圆边界");
    assert!(vertices.len() > 10, "椭圆边界采样过少");
    let area = compute_polygon_area(&vertices);
    assert!((area - 2.2820639).abs() < 0.05, "椭圆边界面积异常: {area}");
}

#[test]
fn load_spline_hatch_entity() {
    let mut fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fixtures.push("tests/data/hatch_spline.dxf");

    let loader = DxfFacade::new();
    let doc = loader
        .load(&fixtures)
        .expect("读取含样条边界 HATCH 的 DXF 失败");
    assert_golden("hatch_spline", &doc);

    let mut hatches = doc.entities().filter_map(|(_, entity)| match entity {
        Entity::Hatch(hatch) => Some(hatch),
        _ => None,
    });
    let hatch = hatches.next().expect("未找到 HATCH 实体");
    assert!(hatches.next().is_none(), "期望仅有一个 HATCH 实体");
    assert_eq!(hatch.loops.len(), 1);
    let loop_path = &hatch.loops[0];
    assert_eq!(loop_path.edges.len(), 1);
    match &loop_path.edges[0] {
        HatchEdge::Spline {
            control_points,
            degree,
            is_rational,
            ..
        } => {
            assert_eq!(control_points.len(), 4);
            assert_eq!(*degree, 2);
            assert!(!is_rational);
        }
        other => panic!("期望样条边界，实际为 {other:?}"),
    }

    let vertices = extract_loop_vertices(loop_path).expect("无法采样样条边界");
    assert!(vertices.len() >= 4, "样条边界采样点数不足");
    let area = compute_polygon_area(&vertices);
    assert!(area > 5.0, "样条边界面积过小: {area}");
}

#[test]
fn load_gradient_hatch_entity() {
    let mut fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fixtures.push("tests/data/hatch_gradient.dxf");

    let loader = DxfFacade::new();
    let doc = loader
        .load(&fixtures)
        .expect("读取含梯度 HATCH 的 DXF 失败");
    assert_golden("hatch_gradient", &doc);

    let mut hatches = doc.entities().filter_map(|(_, entity)| match entity {
        Entity::Hatch(hatch) => Some(hatch),
        _ => None,
    });
    let hatch = hatches.next().expect("未找到 HATCH 实体");
    assert!(hatches.next().is_none(), "期望仅有一个 HATCH 实体");
    let gradient = hatch.gradient.as_ref().expect("梯度信息缺失");
    assert_eq!(gradient.name, "LINEAR");
    assert!((gradient.angle - 0.0).abs() < 1e-9);
    assert_eq!(gradient.color1, Some(1));
    assert_eq!(gradient.color2, Some(3));
}

fn compute_polygon_area(vertices: &[zcad_core::geometry::Point2]) -> f64 {
    if vertices.len() < 3 {
        return 0.0;
    }
    let mut sum = 0.0;
    for window in vertices.windows(2) {
        sum += window[0].x() * window[1].y() - window[1].x() * window[0].y();
    }
    let first = vertices.first().unwrap();
    let last = vertices.last().unwrap();
    sum += last.x() * first.y() - first.x() * last.y();
    0.5 * sum.abs()
}

fn extract_loop_vertices(loop_path: &HatchLoop) -> Option<Vec<Point2>> {
    sample_hatch_loop_vertices(loop_path)
}

fn sample_hatch_loop_vertices(loop_path: &HatchLoop) -> Option<Vec<Point2>> {
    let mut points = Vec::new();
    for edge in &loop_path.edges {
        let edge_points = match edge {
            HatchEdge::Line { start, end } => vec![*start, *end],
            HatchEdge::PolylineSegment { start, end, bulge } => {
                sample_bulged_segment(*start, *end, *bulge, 24)
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
            } => sample_ellipse_segment(
                *center,
                *major_axis,
                *minor_ratio,
                *start_angle,
                *end_angle,
                *is_counter_clockwise,
            ),
            HatchEdge::Spline {
                control_points,
                fit_points,
                ..
            } => sample_spline_segment(control_points, fit_points),
            HatchEdge::BoundaryReference { .. } => return None,
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

fn sample_arc_segment(
    center: Point2,
    radius: f64,
    start_angle: f64,
    end_angle: f64,
    ccw: bool,
    min_segments: usize,
) -> Vec<Point2> {
    if radius <= f64::EPSILON {
        return Vec::new();
    }
    let (start, end) = canonical_angle_range(start_angle, end_angle, ccw);
    let span = end - start;
    let segments = ((span.abs() / (TAU / 64.0)).ceil() as usize).max(min_segments);
    let mut points = Vec::with_capacity(segments + 1);
    for i in 0..=segments {
        let angle = start + span * (i as f64 / segments as f64);
        points.push(point_on_circle(center, radius, angle));
    }
    points
}

fn sample_bulged_segment(
    start: Point2,
    end: Point2,
    bulge: f64,
    min_segments: usize,
) -> Vec<Point2> {
    if bulge.abs() <= 1e-9 {
        return vec![start, end];
    }
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

fn sample_ellipse_segment(
    center: Point2,
    major_axis: Vector2,
    minor_ratio: f64,
    start_angle: f64,
    end_angle: f64,
    ccw: bool,
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

    let (start, end) = canonical_angle_range(start_angle, end_angle, ccw);
    let span = end - start;
    let segments = ((span.abs() / (TAU / 64.0)).ceil() as usize).max(48);
    let mut points = Vec::with_capacity(segments + 1);
    for i in 0..=segments {
        let angle = start + span * (i as f64 / segments as f64);
        let offset = major_vec * angle.cos() + minor_vec * angle.sin();
        let pos = center.as_vec2() + offset;
        points.push(Point2::from_vec(pos));
    }
    points
}

fn sample_spline_segment(control_points: &[Point2], fit_points: &[Point2]) -> Vec<Point2> {
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

#[test]
fn load_leader_and_mleader_entities() {
    let mut fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fixtures.push("tests/data/leader_entities.dxf");

    let loader = DxfFacade::new();
    let doc = loader
        .load(&fixtures)
        .expect("读取含 LEADER/MULTILEADER 的 DXF 失败");
    assert_golden("leader_entities", &doc);

    let mut leaders = doc.entities().filter_map(|(_, entity)| match entity {
        Entity::Leader(leader) => Some(leader),
        _ => None,
    });
    let leader = leaders.next().expect("未找到 LEADER 实体");
    assert!(leaders.next().is_none(), "期望仅有一个 LEADER 实体");
    assert_eq!(leader.layer, "ANNOT");
    assert_eq!(leader.style_name.as_deref(), Some("Standard"));
    assert!(leader.has_arrowhead);
    assert_eq!(leader.vertices.len(), 2);
    assert!(
        (leader.vertices[0].x() - 0.0).abs() < 1e-9 && (leader.vertices[0].y() - 0.0).abs() < 1e-9
    );
    assert!(
        (leader.vertices[1].x() - 25.0).abs() < 1e-9
            && (leader.vertices[1].y() - 10.0).abs() < 1e-9
    );

    let mut mleaders = doc.entities().filter_map(|(_, entity)| match entity {
        Entity::MLeader(mleader) => Some(mleader),
        _ => None,
    });
    let mleader = mleaders.next().expect("未找到 MULTILEADER 实体");
    assert!(mleaders.next().is_none(), "期望仅有一个 MULTILEADER 实体");
    assert_eq!(mleader.layer, "ANNOT");
    assert_eq!(mleader.style_name.as_deref(), Some("Standard"));
    assert_eq!(mleader.leader_lines.len(), 1);
    let line = &mleader.leader_lines[0];
    assert_eq!(line.vertices.len(), 2);
    assert!(
        (line.vertices[0].x() - 25.0).abs() < 1e-9 && (line.vertices[0].y() - 10.0).abs() < 1e-9
    );
    assert!(
        (line.vertices[1].x() - 40.0).abs() < 1e-9 && (line.vertices[1].y() - 12.0).abs() < 1e-9
    );
    match &mleader.content {
        MLeaderContent::MText { text, location } => {
            assert_eq!(text, "Note line 1\nNote line 2");
            assert!((location.x() - 45.0).abs() < 1e-9 && (location.y() - 13.0).abs() < 1e-9);
        }
        other => panic!("预期 MULTILEADER 内容为 MText，实际为 {:?}", other),
    }
    assert!(
        mleader
            .scale
            .map(|value| (value - 2.5).abs() < 1e-9)
            .unwrap_or(false),
        "应解析 MULTILEADER 缩放为 2.5"
    );
    assert!(
        mleader.text_height.is_none(),
        "示例未提供文本高度，应为 None"
    );
}

#[test]
fn load_mleader_block_entity() {
    let mut fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fixtures.push("tests/data/mleader_block.dxf");

    let loader = DxfFacade::new();
    let doc = loader
        .load(&fixtures)
        .expect("读取包含块内容 MULTILEADER 的 DXF 失败");
    assert_golden("mleader_block", &doc);

    let mut mleaders = doc.entities().filter_map(|(_, entity)| match entity {
        Entity::MLeader(mleader) => Some(mleader),
        _ => None,
    });
    let mleader = mleaders.next().expect("未找到 MULTILEADER 实体");
    assert!(mleaders.next().is_none(), "期望仅有一个 MULTILEADER 实体");
    assert_eq!(mleader.layer, "ANNOT");
    assert_eq!(mleader.leader_lines.len(), 1);
    match &mleader.content {
        MLeaderContent::Block { block } => {
            assert_eq!(block.block_handle.as_deref(), Some("31"));
            assert!(
                (block.location.x() - 20.0).abs() < 1e-9
                    && (block.location.y() - 20.0).abs() < 1e-9
            );
            assert!((block.scale.x() - 1.0).abs() < 1e-9);
            assert!((block.scale.y() - 1.0).abs() < 1e-9);
            assert!(block.rotation.abs() < 1e-9);
            assert_eq!(block.connection_type, Some(0));
        }
        other => panic!("预期 MULTILEADER 内容为块，实际为 {:?}", other),
    }
    assert!(
        mleader
            .scale
            .map(|value| (value - 1.0).abs() < 1e-9)
            .unwrap_or(false),
        "缺少或错误的 MULTILEADER 缩放"
    );
    assert!(mleader.has_dogleg);
    assert!(
        mleader
            .dogleg_length
            .map(|value| (value - 8.0).abs() < 1e-9)
            .unwrap_or(false)
    );
    assert!(
        mleader
            .landing_gap
            .map(|gap| (gap - 2.0).abs() < 1e-9)
            .unwrap_or(false),
        "应解析落脚间隙为 2.0"
    );
}

#[test]
fn load_mleader_block_with_attributes() {
    let mut fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fixtures.push("tests/data/mleader_block_attrs.dxf");

    let loader = DxfFacade::new();
    let doc = loader
        .load(&fixtures)
        .expect("读取包含属性块内容的 MULTILEADER DXF 失败");
    assert_golden("mleader_block_attrs", &doc);

    let mut mleaders = doc.entities().filter_map(|(_, entity)| match entity {
        Entity::MLeader(mleader) => Some(mleader),
        _ => None,
    });
    let mleader = mleaders
        .next()
        .expect("未找到 MULTILEADER 实体（属性测试）");
    assert!(
        mleaders.next().is_none(),
        "期望仅有一个 MULTILEADER 实体（属性测试）"
    );

    match &mleader.content {
        MLeaderContent::Block { block } => {
            let resolved_name = block
                .block_name
                .clone()
                .or_else(|| {
                    block
                        .block_handle
                        .as_deref()
                        .and_then(|handle| doc.block_name_by_handle(handle))
                        .map(|s| s.to_string())
                })
                .unwrap_or_else(|| "<missing>".to_string());
            assert_eq!(resolved_name, "TAG_BLOCK", "块内容应解析出 TAG_BLOCK 名称");
            assert!((block.scale.x() - 1.0).abs() < 1e-9);
            assert!((block.scale.y() - 1.0).abs() < 1e-9);
        }
        other => panic!("预期 MULTILEADER 内容为块，实际为 {:?}", other),
    }

    let block = doc.block("TAG_BLOCK").expect("块定义 TAG_BLOCK 缺失");
    assert!(
        !block.attributes.is_empty(),
        "块定义应包含至少一个 ATTDEF，用于验证属性支持"
    );
    assert_eq!(
        doc.block_name_by_handle("32"),
        Some("TAG_BLOCK"),
        "块句柄应映射到块名称"
    );
    assert!(
        mleader
            .scale
            .map(|value| (value - 1.0).abs() < 1e-9)
            .unwrap_or(false)
    );
    assert!(mleader.has_dogleg);
    assert!(
        mleader
            .dogleg_length
            .map(|value| (value - 8.0).abs() < 1e-9)
            .unwrap_or(false)
    );
    assert!(
        mleader
            .landing_gap
            .map(|gap| (gap - 2.0).abs() < 1e-9)
            .unwrap_or(false)
    );
}

#[test]
fn load_mleader_block_connection_variants() {
    let mut fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fixtures.push("tests/data/mleader_block_connections.dxf");

    let loader = DxfFacade::new();
    let doc = loader
        .load(&fixtures)
        .expect("读取包含多种块连接类型的 MULTILEADER DXF 失败");
    assert_golden("mleader_block_connections", &doc);

    let mleaders = doc
        .entities()
        .filter_map(|(_, entity)| match entity {
            Entity::MLeader(mleader) => Some(mleader),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(mleaders.len(), 3, "预期生成三个 MULTILEADER 实例");

    let first = &mleaders[0];
    if let MLeaderContent::Block { block } = &first.content {
        assert_eq!(block.block_name.as_deref(), Some("VALVE_BLOCK"));
        assert_eq!(block.connection_type, Some(0));
    } else {
        panic!("首个 MULTILEADER 预期为块内容");
    }
    assert!(first.scale.map(|s| (s - 1.0).abs() < 1e-9).unwrap_or(false));
    assert!(
        first
            .landing_gap
            .map(|gap| (gap - 2.0).abs() < 1e-9)
            .unwrap_or(false)
    );
    assert!(first.has_dogleg);
    assert!(
        first
            .dogleg_length
            .map(|d| (d - 8.0).abs() < 1e-9)
            .unwrap_or(false)
    );

    let second = &mleaders[1];
    if let MLeaderContent::Block { block } = &second.content {
        assert_eq!(block.block_name.as_deref(), Some("JUNCTION_BLOCK"));
        assert_eq!(block.connection_type, Some(1));
    } else {
        panic!("第二个 MULTILEADER 预期为块内容");
    }
    assert!(
        second
            .scale
            .map(|s| (s - 1.0).abs() < 1e-9)
            .unwrap_or(false)
    );
    assert!(second.has_dogleg);
    assert!(
        second
            .dogleg_length
            .map(|d| (d - 2.0).abs() < 1e-9)
            .unwrap_or(false)
    );

    let third = &mleaders[2];
    if let MLeaderContent::Block { block } = &third.content {
        assert_eq!(block.block_name.as_deref(), Some("VALVE_BLOCK"));
        assert_eq!(block.connection_type, Some(0));
    } else {
        panic!("第三个 MULTILEADER 预期为块内容");
    }
    assert!(third.scale.map(|s| (s - 1.0).abs() < 1e-9).unwrap_or(false));
    assert!(
        third
            .landing_gap
            .map(|gap| (gap - 2.0).abs() < 1e-9)
            .unwrap_or(false)
    );
    assert!(third.has_dogleg);
    assert!(
        third
            .dogleg_length
            .map(|d| (d - 8.0).abs() < 1e-9)
            .unwrap_or(false)
    );
}

#[test]
fn load_raster_image_entity() {
    let mut fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fixtures.push("tests/data/image_basic.dxf");

    let loader = DxfFacade::new();
    let doc = loader
        .load(&fixtures)
        .expect("读取包含 IMAGE 实体的 DXF 失败");
    assert_golden("image_basic", &doc);

    let mut images = doc.entities().filter_map(|(_, entity)| match entity {
        Entity::RasterImage(image) => Some(image),
        _ => None,
    });
    let image = images.next().expect("未找到 IMAGE 实体");
    assert!(images.next().is_none(), "期望仅有一个 IMAGE 实体");

    assert_eq!(image.layer, "RASTER");
    assert_eq!(image.image_def_handle, "ABC");
    assert!((image.insert.x() - 0.0).abs() < 1e-9);
    assert!((image.insert.y() - 0.0).abs() < 1e-9);
    let u = image.u_vector.as_vec2();
    assert!((u.x - 1.0).abs() < 1e-9);
    assert!(u.y.abs() < 1e-9);
    let v = image.v_vector.as_vec2();
    assert!(v.x.abs() < 1e-9);
    assert!((v.y - 1.0).abs() < 1e-9);
    let size = image.image_size.as_vec2();
    assert!((size.x - 2.0).abs() < 1e-9);
    assert!((size.y - 1.0).abs() < 1e-9);
    assert!(image.display_options.show_image);
    assert!(image.display_options.show_border);
    assert!(image.display_options.use_clipping);
    assert_eq!(image.display_options.brightness, Some(50));
    assert_eq!(image.display_options.contrast, Some(50));
    assert_eq!(image.display_options.fade, Some(10));
    assert!(image.clip.is_none());
    assert_eq!(
        image.image_def_reactor_handle.as_deref(),
        Some("R1"),
        "应解析 IMAGEDEF_REACTOR 句柄"
    );

    let definition = doc
        .raster_image_definition("ABC")
        .expect("未找到对应的 IMAGEDEF");
    assert_eq!(definition.file_path, "images/sample.png");
    assert_eq!(definition.name.as_deref(), Some("SAMPLE_IMAGE"));
    let size_pixels = definition
        .image_size_pixels
        .expect("应包含像素尺寸")
        .as_vec2();
    assert!((size_pixels.x - 1024.0).abs() < 1e-9);
    assert!((size_pixels.y - 512.0).abs() < 1e-9);
    let pixel_size = definition.pixel_size.expect("应包含单像素尺寸").as_vec2();
    assert!((pixel_size.x - 0.0009765625).abs() < 1e-12);
    assert!((pixel_size.y - 0.001953125).abs() < 1e-12);

    let dictionary = doc.image_dictionary().expect("应解析 ACAD_IMAGE_DICT 字典");
    assert_eq!(dictionary.handle.as_deref(), Some("D1"));
    assert_eq!(dictionary.entries.len(), 1);
    let entry = &dictionary.entries[0];
    assert_eq!(entry.name, "SampleImage");
    assert_eq!(entry.image_def_handle, "ABC");
    assert_eq!(
        entry.reactor_handle.as_deref(),
        Some("R1"),
        "字典条目应关联 REACTOR 句柄"
    );

    let reactor = doc
        .image_def_reactor("R1")
        .expect("应解析 IMAGEDEF_REACTOR 对象");
    assert_eq!(reactor.owner_handle.as_deref(), Some("ABC"));
    assert_eq!(reactor.image_handle.as_deref(), Some("1"));

    let vars = doc
        .raster_image_variables()
        .expect("应解析 RASTERVARIABLES 对象");
    assert_eq!(vars.handle.as_deref(), Some("RV1"));
    assert_eq!(vars.frame, Some(1));
    assert_eq!(vars.quality, Some(1));
    assert_eq!(vars.units, Some(3));
}

#[test]
fn load_raster_image_with_missing_file() {
    let mut fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fixtures.push("tests/data/image_missing_file.dxf");

    let loader = DxfFacade::new();
    let doc = loader
        .load(&fixtures)
        .expect("读取包含缺失文件的 IMAGE DXF 失败");
    assert_golden("image_missing_file", &doc);

    let mut images = doc.entities().filter_map(|(_, entity)| match entity {
        Entity::RasterImage(image) => Some(image),
        _ => None,
    });
    let image = images.next().expect("未找到 IMAGE 实体");
    assert!(images.next().is_none(), "期望仅有一个 IMAGE 实体");

    assert_eq!(image.layer, "RASTER");
    assert_eq!(image.image_def_handle, "20");
    assert!(image.image_def_reactor_handle.is_none());
    assert!(image.display_options.show_image);
    assert!(!image.display_options.show_border);
    assert!(!image.display_options.use_clipping);

    let definition = doc
        .raster_image_definition("20")
        .expect("缺少 IMAGEDEF 定义");
    assert_eq!(definition.file_path, "missing/path/to/image.png");
    assert!(
        definition.resolved_path.is_none(),
        "缺失文件不应解析到可用路径"
    );
}

#[test]
fn load_raster_image_with_polygon_clip() {
    let mut fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fixtures.push("tests/data/image_clip_polygon.dxf");

    let loader = DxfFacade::new();
    let doc = loader
        .load(&fixtures)
        .expect("读取包含多边形裁剪的 IMAGE DXF 失败");
    assert_golden("image_clip_polygon", &doc);

    let mut images = doc.entities().filter_map(|(_, entity)| match entity {
        Entity::RasterImage(image) => Some(image),
        _ => None,
    });

    let image = images.next().expect("应存在一个 IMAGE 实体");
    assert!(images.next().is_none());
    assert_eq!(
        image.image_def_reactor_handle.as_deref(),
        Some("R2"),
        "应解析多边形裁剪样例的 REACTOR 句柄"
    );

    let clip = image.clip.as_ref().expect("应解析出裁剪多边形");
    match clip {
        RasterImageClip::Polygon { vertices, mode } => {
            assert_eq!(*mode, ClipMode::Outside, "默认样例应为普通裁剪模式");
            assert_eq!(vertices.len(), 4);
            assert!((vertices[0].x() - 0.0).abs() < 1e-9);
            assert!((vertices[0].y() - 0.0).abs() < 1e-9);
            assert!((vertices[1].x() - 2.0).abs() < 1e-9);
            assert!((vertices[1].y() - 0.2).abs() < 1e-9);
        }
        _ => panic!("裁剪结果应为多边形"),
    }

    let dictionary = doc.image_dictionary().expect("应解析 ACAD_IMAGE_DICT 字典");
    assert!(
        dictionary
            .entries
            .iter()
            .any(|entry| entry.name == "ClipImage" && entry.reactor_handle.as_deref() == Some("R2")),
        "字典应包含 ClipImage 条目并关联 REACTOR"
    );

    let reactor = doc
        .image_def_reactor("R2")
        .expect("应解析第二个 IMAGEDEF_REACTOR");
    assert_eq!(reactor.owner_handle.as_deref(), Some("DEF1"));
    assert_eq!(reactor.image_handle.as_deref(), Some("100"));

    let vars = doc
        .raster_image_variables()
        .expect("应共享 RASTERVARIABLES");
    assert_eq!(vars.frame, Some(1));
}

#[test]
fn load_raster_image_with_dict_clip_basic() {
    let mut fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fixtures.push("tests/data/image_clip_dict_basic.dxf");

    let loader = DxfFacade::new();
    let doc = loader
        .load(&fixtures)
        .expect("读取包含字典裁剪的 IMAGE DXF 失败");
    assert_golden("image_clip_dict_basic", &doc);

    let mut images = doc.entities().filter_map(|(_, entity)| match entity {
        Entity::RasterImage(image) => Some(image),
        _ => None,
    });
    let image = images.next().expect("应存在 IMAGE 实体");
    assert!(images.next().is_none());

    assert_eq!(
        image.image_def_reactor_handle.as_deref(),
        Some("R_DICT1"),
        "应解析 IMAGEDEF_REACTOR 句柄"
    );

    let clip = image.clip.as_ref().expect("应解析出裁剪多边形");
    match clip {
        RasterImageClip::Polygon { vertices, mode } => {
            assert_eq!(*mode, ClipMode::Outside, "字典裁剪样例应为默认模式");
            assert_eq!(vertices.len(), 4);
        }
        _ => panic!("裁剪结果应为多边形"),
    }

    let dictionary = doc.image_dictionary().expect("应解析 ACAD_IMAGE_DICT 字典");
    let entry = dictionary
        .entries
        .iter()
        .find(|entry| entry.name == "DictRasterImage")
        .expect("应包含 DictRasterImage 条目");
    assert_eq!(entry.image_def_handle, "DEF_DICT1");
    assert_eq!(entry.reactor_handle.as_deref(), Some("R_DICT1"));

    let vars = doc
        .raster_image_variables()
        .expect("应解析 RASTERVARIABLES");
    assert_eq!(vars.frame, Some(2));
    assert_eq!(vars.quality, Some(2));
    assert_eq!(vars.units, Some(3));
}

#[test]
fn load_raster_image_with_dict_clip_runtime() {
    let mut fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fixtures.push("tests/data/image_clip_dict_runtime.dxf");

    let loader = DxfFacade::new();
    let doc = loader
        .load(&fixtures)
        .expect("读取包含运行时裁剪样例的 IMAGE DXF 失败");
    assert_golden("image_clip_dict_runtime", &doc);

    let mut images = doc.entities().filter_map(|(_, entity)| match entity {
        Entity::RasterImage(image) => Some(image),
        _ => None,
    });
    let image = images.next().expect("应存在 IMAGE 实体");
    assert!(images.next().is_none());

    let clip = image.clip.as_ref().expect("应解析出裁剪多边形");
    match clip {
        RasterImageClip::Polygon { vertices, mode } => {
            assert_eq!(*mode, ClipMode::Outside, "实时裁剪样例也应该是默认裁剪方向");
            assert_eq!(vertices.len(), 5);
        }
        _ => panic!("裁剪结果应为多边形"),
    }

    let dictionary = doc.image_dictionary().expect("应解析 ACAD_IMAGE_DICT 字典");
    let entry = dictionary
        .entries
        .iter()
        .find(|entry| entry.name == "RuntimeImage")
        .expect("应包含 RuntimeImage 条目");
    assert_eq!(entry.image_def_handle, "DEF_RUNTIME1");
    assert_eq!(entry.reactor_handle.as_deref(), Some("R_RUNTIME1"));

    let vars = doc
        .raster_image_variables()
        .expect("应解析 RASTERVARIABLES");
    assert_eq!(vars.frame, Some(3));
    assert_eq!(vars.quality, Some(4));
    assert_eq!(vars.units, Some(1));
}

#[test]
fn load_wipeout_clip_mode() {
    let mut fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fixtures.push("tests/data/wipeout_clip.dxf");

    let loader = DxfFacade::new();
    let doc = loader
        .load(&fixtures)
        .expect("读取包含裁剪的 WIPEOUT DXF 失败");
    assert_golden("wipeout_clip", &doc);

    let mut wipeouts = doc.entities().filter_map(|(_, entity)| match entity {
        Entity::Wipeout(wipeout) => Some(wipeout),
        _ => None,
    });

    let wipeout = wipeouts.next().expect("应存在 WIPEOUT 实体");
    assert!(wipeouts.next().is_none());

    let clip = wipeout.clip.as_ref().expect("应解析出裁剪多边形");
    match clip {
        RasterImageClip::Polygon { mode, vertices } => {
            assert_eq!(*mode, ClipMode::Inside, "WIPEOUT 样例应使用反向裁剪");
            assert_eq!(vertices.len(), 4);
        }
        _ => panic!("裁剪结果应为多边形"),
    }
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

#[test]
fn load_raster_image_with_inverted_clip() {
    let mut fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fixtures.push("tests/data/image_clip_polygon_inverted.dxf");

    let loader = DxfFacade::new();
    let doc = loader
        .load(&fixtures)
        .expect("读取包含反向裁剪的 IMAGE DXF 失败");
    assert_golden("image_clip_polygon_inverted", &doc);

    let mut images = doc.entities().filter_map(|(_, entity)| match entity {
        Entity::RasterImage(image) => Some(image),
        _ => None,
    });

    let image = images.next().expect("应存在 IMAGE 实体");
    assert!(images.next().is_none());

    let clip = image.clip.as_ref().expect("应解析出裁剪多边形");
    match clip {
        RasterImageClip::Polygon { mode, vertices } => {
            assert_eq!(
                *mode,
                ClipMode::Inside,
                "反向裁剪样例应将 clip_mode 设为 Inside"
            );
            assert_eq!(vertices.len(), 4);
        }
        _ => panic!("裁剪结果应为多边形"),
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
