pub mod geometry {
    use glam::{DVec2, DVec3};
    use serde::{Deserialize, Serialize};

    /// 二维点，内部以 `glam::DVec2` 表示，确保与双精度 Pascal 版本兼容。
    #[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
    pub struct Point2(pub DVec2);

    impl Point2 {
        #[inline]
        pub fn new(x: f64, y: f64) -> Self {
            Self(DVec2::new(x, y))
        }

        #[inline]
        pub fn from_vec(vec: DVec2) -> Self {
            Self(vec)
        }

        #[inline]
        pub fn x(self) -> f64 {
            self.0.x
        }

        #[inline]
        pub fn y(self) -> f64 {
            self.0.y
        }

        #[inline]
        pub fn translate(self, offset: Vector2) -> Self {
            Self(self.0 + offset.0)
        }

        #[inline]
        pub fn vector_to(self, other: Point2) -> Vector2 {
            Vector2(other.0 - self.0)
        }

        #[inline]
        pub fn as_vec2(self) -> DVec2 {
            self.0
        }
    }

    impl From<DVec2> for Point2 {
        fn from(value: DVec2) -> Self {
            Self::from_vec(value)
        }
    }

    /// 二维向量。提供基础运算，未来可扩展矩阵变换。
    #[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
    pub struct Vector2(pub DVec2);

    impl Vector2 {
        #[inline]
        pub fn new(x: f64, y: f64) -> Self {
            Self(DVec2::new(x, y))
        }

        #[inline]
        pub fn from_points(start: Point2, end: Point2) -> Self {
            Self(end.0 - start.0)
        }

        #[inline]
        pub fn length_squared(self) -> f64 {
            self.0.length_squared()
        }

        #[inline]
        pub fn as_vec2(self) -> DVec2 {
            self.0
        }

        #[inline]
        pub fn x(self) -> f64 {
            self.0.x
        }

        #[inline]
        pub fn y(self) -> f64 {
            self.0.y
        }
    }

    impl From<DVec2> for Vector2 {
        fn from(value: DVec2) -> Self {
            Self(value)
        }
    }

    /// 三维点，供 3D 相关实体（如 3DFace）使用。
    #[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
    pub struct Point3(pub DVec3);

    impl Point3 {
        #[inline]
        pub fn new(x: f64, y: f64, z: f64) -> Self {
            Self(DVec3::new(x, y, z))
        }

        #[inline]
        pub fn x(self) -> f64 {
            self.0.x
        }

        #[inline]
        pub fn y(self) -> f64 {
            self.0.y
        }

        #[inline]
        pub fn z(self) -> f64 {
            self.0.z
        }

        #[inline]
        pub fn as_vec3(self) -> DVec3 {
            self.0
        }
    }

    impl From<DVec3> for Point3 {
        fn from(value: DVec3) -> Self {
            Self(value)
        }
    }

    /// 三维向量，当前主要用于 3DFace 扩展。
    #[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
    pub struct Vector3(pub DVec3);

    impl Vector3 {
        #[inline]
        pub fn new(x: f64, y: f64, z: f64) -> Self {
            Self(DVec3::new(x, y, z))
        }

        #[inline]
        pub fn as_vec3(self) -> DVec3 {
            self.0
        }

        #[inline]
        pub fn length_squared(self) -> f64 {
            self.0.length_squared()
        }

        #[inline]
        pub fn normalize(self) -> Option<Self> {
            let len = self.0.length();
            if len <= f64::EPSILON {
                None
            } else {
                Some(Self(self.0 / len))
            }
        }

        #[inline]
        pub fn dot(self, other: Vector3) -> f64 {
            self.0.dot(other.0)
        }

        #[inline]
        pub fn cross(self, other: Vector3) -> Vector3 {
            Self(self.0.cross(other.0))
        }
    }

    impl From<DVec3> for Vector3 {
        fn from(value: DVec3) -> Self {
            Self(value)
        }
    }

    /// 轴对齐边界框，用于估算文档/实体范围。
    #[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
    pub struct Bounds2D {
        min: Point2,
        max: Point2,
    }

    impl Bounds2D {
        #[inline]
        pub fn new(min: Point2, max: Point2) -> Self {
            Self { min, max }
        }

        #[inline]
        pub fn empty() -> Self {
            Self {
                min: Point2::new(f64::INFINITY, f64::INFINITY),
                max: Point2::new(f64::NEG_INFINITY, f64::NEG_INFINITY),
            }
        }

        #[inline]
        pub fn is_empty(&self) -> bool {
            self.min.x() > self.max.x() || self.min.y() > self.max.y()
        }

        #[inline]
        pub fn min(&self) -> Point2 {
            self.min
        }

        #[inline]
        pub fn max(&self) -> Point2 {
            self.max
        }

        pub fn include_point(&mut self, point: Point2) {
            if self.is_empty() {
                self.min = point;
                self.max = point;
                return;
            }
            let min_vec = self.min.as_vec2().min(point.as_vec2());
            let max_vec = self.max.as_vec2().max(point.as_vec2());
            self.min = Point2::from_vec(min_vec);
            self.max = Point2::from_vec(max_vec);
        }

        pub fn include_bounds(&mut self, other: &Bounds2D) {
            if other.is_empty() {
                return;
            }
            self.include_point(other.min);
            self.include_point(other.max);
        }

        #[inline]
        pub fn center(&self) -> Point2 {
            debug_assert!(!self.is_empty());
            let min_vec = self.min.as_vec2();
            let max_vec = self.max.as_vec2();
            let center = (min_vec + max_vec) * 0.5;
            Point2::from_vec(center)
        }
    }
}

pub mod document {
    use std::collections::HashMap;
    use std::f64::consts::{FRAC_PI_2, PI, TAU};

    use glam::DVec2;
    use serde::{Deserialize, Serialize};

    use crate::geometry::{Bounds2D, Point2, Point3, Vector2, Vector3};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub struct EntityId(u64);

    impl EntityId {
        #[inline]
        pub fn new(raw: u64) -> Self {
            Self(raw)
        }

        /// 提供原始数值，便于序列化或日志输出。
        #[inline]
        pub fn get(self) -> u64 {
            self.0
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Layer {
        pub name: String,
        pub is_visible: bool,
    }

    impl Layer {
        #[inline]
        pub fn new(name: impl Into<String>) -> Self {
            Self {
                name: name.into(),
                is_visible: true,
            }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum Entity {
        Line(Line),
        Circle(Circle),
        Arc(Arc),
        Ellipse(Ellipse),
        Polyline(Polyline),
        Spline(Spline),
        Text(Text),
        MText(MText),
        BlockReference(BlockReference),
        Hatch(Hatch),
        Dimension(Dimension),
        Leader(Leader),
        MLeader(MLeader),
        RasterImage(RasterImage),
        Wipeout(Wipeout),
        Face3D(ThreeDFace),
    }

    impl Entity {
        #[inline]
        pub fn layer_name(&self) -> &str {
            match self {
                Entity::Line(line) => &line.layer,
                Entity::Circle(circle) => &circle.layer,
                Entity::Arc(arc) => &arc.layer,
                Entity::Ellipse(ellipse) => &ellipse.layer,
                Entity::Polyline(polyline) => &polyline.layer,
                Entity::Spline(spline) => &spline.layer,
                Entity::Text(text) => &text.layer,
                Entity::MText(mtext) => &mtext.layer,
                Entity::BlockReference(reference) => &reference.layer,
                Entity::Hatch(hatch) => &hatch.layer,
                Entity::Dimension(dimension) => &dimension.layer,
                Entity::Leader(leader) => &leader.layer,
                Entity::MLeader(mleader) => &mleader.layer,
                Entity::RasterImage(image) => &image.layer,
                Entity::Wipeout(wipeout) => &wipeout.layer,
                Entity::Face3D(face) => &face.layer,
            }
        }

        /// 计算实体的 2D 轴对齐范围，少数对象（文本、块参照）退化为点。
        pub fn bounds(&self) -> Option<Bounds2D> {
            let mut bounds = Bounds2D::empty();
            match self {
                Entity::Line(line) => {
                    bounds.include_point(line.start);
                    bounds.include_point(line.end);
                }
                Entity::Circle(circle) => {
                    let radius = circle.radius.abs();
                    let center = circle.center;
                    bounds.include_point(Point2::new(center.x() - radius, center.y() - radius));
                    bounds.include_point(Point2::new(center.x() + radius, center.y() + radius));
                }
                Entity::Arc(arc) => {
                    arc_bounds(arc, &mut bounds);
                }
                Entity::Ellipse(ellipse) => {
                    ellipse_bounds(ellipse, &mut bounds);
                }
                Entity::Polyline(polyline) => {
                    for vertex in &polyline.vertices {
                        bounds.include_point(vertex.position);
                    }
                }
                Entity::Spline(spline) => {
                    for point in &spline.control_points {
                        bounds.include_point(*point);
                    }
                    for point in &spline.fit_points {
                        bounds.include_point(*point);
                    }
                }
                Entity::Text(text) => {
                    bounds.include_point(text.insert);
                }
                Entity::MText(mtext) => {
                    bounds.include_point(mtext.insert);
                }
                Entity::BlockReference(reference) => {
                    bounds.include_point(reference.insert);
                    for attr in &reference.attributes {
                        bounds.include_point(attr.insert);
                        if let Some(alignment) = attr.alignment {
                            bounds.include_point(alignment);
                        }
                    }
                }
                Entity::Hatch(hatch) => {
                    for loop_path in &hatch.loops {
                        for edge in &loop_path.edges {
                            include_hatch_edge_bounds(edge, &mut bounds);
                        }
                    }
                }
                Entity::Dimension(dimension) => {
                    bounds.include_point(dimension.definition_point);
                    bounds.include_point(dimension.text_midpoint);
                    if let Some(dim_line) = dimension.dimension_line_point {
                        bounds.include_point(dim_line);
                    }
                    if let Some(ext1) = dimension.extension_line_origin {
                        bounds.include_point(ext1);
                    }
                    if let Some(ext2) = dimension.extension_line_end {
                        bounds.include_point(ext2);
                    }
                    if let Some(point) = dimension.secondary_point {
                        bounds.include_point(point);
                    }
                    if let Some(point) = dimension.arc_definition_point {
                        bounds.include_point(point);
                    }
                    if let Some(point) = dimension.center_point {
                        bounds.include_point(point);
                    }
                }
                Entity::Leader(leader) => {
                    for vertex in &leader.vertices {
                        bounds.include_point(*vertex);
                    }
                }
                Entity::MLeader(mleader) => {
                    for line in &mleader.leader_lines {
                        for vertex in &line.vertices {
                            bounds.include_point(*vertex);
                        }
                    }
                    match &mleader.content {
                        MLeaderContent::MText { location, .. } => {
                            bounds.include_point(*location);
                        }
                        MLeaderContent::Block { block } => {
                            bounds.include_point(block.location);
                        }
                        MLeaderContent::None => {}
                    }
                }
                Entity::RasterImage(image) => {
                    include_clip_bounds(
                        &mut bounds,
                        image.insert,
                        image.u_vector,
                        image.v_vector,
                        image.image_size,
                        image.clip.as_ref(),
                    );
                }
                Entity::Wipeout(wipeout) => {
                    include_clip_bounds(
                        &mut bounds,
                        wipeout.insert,
                        wipeout.u_vector,
                        wipeout.v_vector,
                        wipeout.image_size,
                        wipeout.clip.as_ref(),
                    );
                }
                Entity::Face3D(face) => {
                    for vertex in &face.vertices {
                        bounds.include_point(Point2::new(vertex.x(), vertex.y()));
                    }
                }
            }
            if bounds.is_empty() {
                None
            } else {
                Some(bounds)
            }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Line {
        pub start: Point2,
        pub end: Point2,
        pub layer: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Circle {
        pub center: Point2,
        pub radius: f64,
        pub layer: String,
    }

    /// 圆弧实体，角度以弧度形式储存，遵循数学正方向。
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Arc {
        pub center: Point2,
        pub radius: f64,
        pub start_angle: f64,
        pub end_angle: f64,
        pub layer: String,
    }

    /// 椭圆实体，记录主轴向量与参数范围（单位为弧度）。
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Ellipse {
        pub center: Point2,
        pub major_axis: Vector2,
        pub ratio: f64,
        pub start_parameter: f64,
        pub end_parameter: f64,
        pub layer: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Polyline {
        pub vertices: Vec<PolylineVertex>,
        pub is_closed: bool,
        pub layer: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Spline {
        pub degree: i32,
        pub is_rational: bool,
        pub is_closed: bool,
        pub is_periodic: bool,
        pub control_points: Vec<Point2>,
        pub fit_points: Vec<Point2>,
        pub knot_values: Vec<f64>,
        pub weights: Vec<f64>,
        pub start_tangent: Option<Vector2>,
        pub end_tangent: Option<Vector2>,
        pub layer: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct PolylineVertex {
        pub position: Point2,
        pub bulge: f64,
    }

    impl PolylineVertex {
        #[inline]
        pub fn new(position: Point2) -> Self {
            Self {
                position,
                bulge: 0.0,
            }
        }

        #[inline]
        pub fn with_bulge(position: Point2, bulge: f64) -> Self {
            Self { position, bulge }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Text {
        pub insert: Point2,
        pub content: String,
        pub height: f64,
        pub rotation: f64,
        pub layer: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct MText {
        pub insert: Point2,
        pub content: String,
        pub height: f64,
        pub reference_width: Option<f64>,
        pub direction: Vector2,
        pub attachment_point: i16,
        pub drawing_direction: i16,
        pub style: Option<String>,
        pub layer: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct HatchLoop {
        pub is_polyline: bool,
        pub is_closed: bool,
        pub edges: Vec<HatchEdge>,
        pub boundary_handles: Vec<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum HatchEdge {
        Line {
            start: Point2,
            end: Point2,
        },
        Arc {
            center: Point2,
            radius: f64,
            start_angle: f64,
            end_angle: f64,
            is_counter_clockwise: bool,
        },
        PolylineSegment {
            start: Point2,
            end: Point2,
            bulge: f64,
        },
        Ellipse {
            center: Point2,
            major_axis: Vector2,
            minor_ratio: f64,
            start_angle: f64,
            end_angle: f64,
            is_counter_clockwise: bool,
        },
        BoundaryReference {
            handle: String,
        },
        Spline {
            control_points: Vec<Point2>,
            fit_points: Vec<Point2>,
            knot_values: Vec<f64>,
            degree: i32,
            is_rational: bool,
            is_periodic: bool,
        },
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct HatchGradient {
        pub name: String,
        pub angle: f64,
        pub shift: Option<f64>,
        pub tint: Option<f64>,
        pub is_single_color: bool,
        pub color1: Option<u32>,
        pub color2: Option<u32>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Hatch {
        pub pattern_name: String,
        pub is_solid: bool,
        pub loops: Vec<HatchLoop>,
        pub gradient: Option<HatchGradient>,
        pub layer: String,
    }

    #[derive(Debug, Clone, Copy, Serialize, Deserialize)]
    pub enum DimensionKind {
        Linear,
        Aligned,
        Angular,
        Diameter,
        Radius,
        Angular3Point,
        Ordinate,
        Unknown(i16),
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Dimension {
        pub kind: DimensionKind,
        pub definition_point: Point2,
        pub text_midpoint: Point2,
        pub dimension_line_point: Option<Point2>,
        pub extension_line_origin: Option<Point2>,
        pub extension_line_end: Option<Point2>,
        pub secondary_point: Option<Point2>,
        pub arc_definition_point: Option<Point2>,
        pub center_point: Option<Point2>,
        pub text: Option<String>,
        pub measurement: Option<f64>,
        pub rotation: f64,
        pub text_rotation: Option<f64>,
        pub oblique_angle: Option<f64>,
        pub layer: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Leader {
        pub layer: String,
        pub style_name: Option<String>,
        pub vertices: Vec<Point2>,
        pub has_arrowhead: bool,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct LeaderLine {
        pub vertices: Vec<Point2>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct MLeaderBlockContent {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub block_handle: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub block_name: Option<String>,
        pub location: Point2,
        pub scale: Vector2,
        pub rotation: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub connection_type: Option<i16>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(tag = "kind", rename_all = "snake_case")]
    pub enum MLeaderContent {
        /// 简化实现：记录 MLeader 的文字内容及插入点。
        MText { text: String, location: Point2 },
        /// 使用块参照作为 MLeader 内容。
        Block { block: MLeaderBlockContent },
        /// 暂未解析的内容类型。
        None,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct MLeader {
        pub layer: String,
        pub style_name: Option<String>,
        pub leader_lines: Vec<LeaderLine>,
        pub content: MLeaderContent,
        pub text_height: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub scale: Option<f64>,
        #[serde(default)]
        pub has_dogleg: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub dogleg_length: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub landing_gap: Option<f64>,
    }

    /// 3D 面（3DFACE）实体，主要用于边缘模型。
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ThreeDFace {
        pub layer: String,
        pub vertices: [Point3; 4],
        /// DXF 组码 70 对应的隐蔽边标记：依次表示边 1-4。
        pub invisible_edges: [bool; 4],
    }

    impl ThreeDFace {
        /// 计算未归一化的法向量。若顶点退化则返回 None。
        pub fn normal(&self) -> Option<Vector3> {
            let a = self.vertices[0].as_vec3();
            let b = self.vertices[1].as_vec3();
            let c = self.vertices[2].as_vec3();
            let ab = b - a;
            let ac = c - a;
            let normal = ab.cross(ac);
            if normal.length_squared() <= f64::EPSILON {
                None
            } else {
                Some(Vector3::from(normal))
            }
        }

        /// 平均高度（Z 值），用于排序或简单着色。
        pub fn average_height(&self) -> f64 {
            let sum: f64 = self.vertices.iter().map(|vertex| vertex.z()).sum();
            sum / (self.vertices.len() as f64)
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct RasterImageDisplayOptions {
        pub show_image: bool,
        pub show_border: bool,
        pub use_clipping: bool,
        pub brightness: Option<i16>,
        pub contrast: Option<i16>,
        pub fade: Option<i16>,
    }

    impl Default for RasterImageDisplayOptions {
        fn default() -> Self {
            Self {
                show_image: true,
                show_border: false,
                use_clipping: false,
                brightness: None,
                contrast: None,
                fade: None,
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum ClipMode {
        Outside,
        Inside,
    }

    impl Default for ClipMode {
        fn default() -> Self {
            ClipMode::Outside
        }
    }

    impl ClipMode {
        pub fn describe(&self) -> &'static str {
            match self {
                ClipMode::Outside => "retain interior (default)",
                ClipMode::Inside => "invert clipping (exclude interior)",
            }
        }

        pub fn wants_ccw(&self) -> bool {
            matches!(self, ClipMode::Outside)
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct RasterImage {
        pub layer: String,
        pub image_def_handle: String,
        pub insert: Point2,
        pub u_vector: Vector2,
        pub v_vector: Vector2,
        pub image_size: Vector2,
        pub display_options: RasterImageDisplayOptions,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub image_def_reactor_handle: Option<String>,
        pub clip: Option<RasterImageClip>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct RasterImageDefinition {
        pub handle: String,
        pub name: Option<String>,
        pub file_path: String,
        pub image_size_pixels: Option<Vector2>,
        pub pixel_size: Option<Vector2>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub resolved_path: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum RasterImageClip {
        Rectangle {
            min: Point2,
            max: Point2,
            #[serde(default)]
            mode: ClipMode,
        },
        Polygon {
            vertices: Vec<Point2>,
            #[serde(default)]
            mode: ClipMode,
        },
    }

    impl RasterImageClip {
        pub fn mode(&self) -> ClipMode {
            match self {
                RasterImageClip::Rectangle { mode, .. } => *mode,
                RasterImageClip::Polygon { mode, .. } => *mode,
            }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Wipeout {
        pub layer: String,
        pub insert: Point2,
        pub u_vector: Vector2,
        pub v_vector: Vector2,
        pub image_size: Vector2,
        pub display_options: RasterImageDisplayOptions,
        pub clip: Option<RasterImageClip>,
    }

    fn include_clip_bounds(
        bounds: &mut Bounds2D,
        insert: Point2,
        u_vector: Vector2,
        v_vector: Vector2,
        image_size: Vector2,
        clip: Option<&RasterImageClip>,
    ) {
        let origin = insert.as_vec2();
        let u = u_vector.as_vec2();
        let v = v_vector.as_vec2();
        let mut include_local = |local: Point2| {
            let local_vec = local.as_vec2();
            let world = origin + u * local_vec.x + v * local_vec.y;
            bounds.include_point(Point2::from_vec(world));
        };

        if let Some(clip) = clip {
            match clip {
                RasterImageClip::Rectangle { min, max, .. } => {
                    let corners = [
                        *min,
                        Point2::new(max.x(), min.y()),
                        Point2::new(min.x(), max.y()),
                        *max,
                    ];
                    for corner in corners {
                        include_local(corner);
                    }
                }
                RasterImageClip::Polygon { vertices, .. } => {
                    for vertex in vertices {
                        include_local(*vertex);
                    }
                }
            }
        } else {
            let width = image_size.x();
            let height = image_size.y();
            let corners = [
                Point2::new(0.0, 0.0),
                Point2::new(width, 0.0),
                Point2::new(0.0, height),
                Point2::new(width, height),
            ];
            for corner in corners {
                include_local(corner);
            }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ImageDefReactor {
        pub handle: String,
        pub class_version: i32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub owner_handle: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub image_handle: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ImageDictionaryEntry {
        pub name: String,
        pub image_def_handle: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub reactor_handle: Option<String>,
    }

    #[derive(Debug, Default, Clone, Serialize, Deserialize)]
    pub struct ImageDictionary {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub handle: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pub entries: Vec<ImageDictionaryEntry>,
    }

    impl ImageDictionary {
        pub fn get(&self, name: &str) -> Option<&ImageDictionaryEntry> {
            self.entries
                .iter()
                .find(|entry| entry.name.eq_ignore_ascii_case(name))
        }
    }

    #[derive(Debug, Default, Clone, Serialize, Deserialize)]
    pub struct RasterImageVariables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub handle: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub class_version: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub frame: Option<i16>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub quality: Option<i16>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub units: Option<i16>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Attribute {
        pub tag: String,
        pub text: String,
        pub insert: Point2,
        pub height: f64,
        pub rotation: f64,
        pub width_factor: f64,
        pub oblique: f64,
        pub style: Option<String>,
        pub prompt: Option<String>,
        pub alignment: Option<Point2>,
        pub horizontal_align: i16,
        pub vertical_align: i16,
        pub line_spacing_factor: f64,
        pub line_spacing_style: i16,
        pub is_invisible: bool,
        pub is_constant: bool,
        pub is_verify: bool,
        pub is_preset: bool,
        pub lock_position: bool,
        pub layer: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct BlockReference {
        pub name: String,
        pub insert: Point2,
        pub scale: Vector2,
        pub rotation: f64,
        pub attributes: Vec<Attribute>,
        pub layer: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct BlockDefinition {
        pub name: String,
        pub base_point: Point2,
        pub entities: Vec<Entity>,
        pub attributes: Vec<AttributeDefinition>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct AttributeDefinition {
        pub tag: String,
        pub prompt: Option<String>,
        pub default_text: String,
        pub insert: Point2,
        pub height: f64,
        pub rotation: f64,
        pub width_factor: f64,
        pub oblique: f64,
        pub style: Option<String>,
        pub alignment: Option<Point2>,
        pub horizontal_align: i16,
        pub vertical_align: i16,
        pub line_spacing_factor: f64,
        pub line_spacing_style: i16,
        pub is_invisible: bool,
        pub is_constant: bool,
        pub is_verify: bool,
        pub is_preset: bool,
        pub lock_position: bool,
        pub layer: String,
    }

    #[derive(Debug, Default, Clone, Serialize, Deserialize)]
    pub struct Document {
        layers: HashMap<String, Layer>,
        entities: Vec<(EntityId, Entity)>,
        next_entity_id: u64,
        blocks: HashMap<String, BlockDefinition>,
        #[serde(default, skip_serializing_if = "HashMap::is_empty")]
        block_handles: HashMap<String, String>,
        image_definitions: HashMap<String, RasterImageDefinition>,
        #[serde(default)]
        image_def_reactors: HashMap<String, ImageDefReactor>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        image_dictionary: Option<ImageDictionary>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        raster_image_variables: Option<RasterImageVariables>,
    }

    impl Document {
        pub fn new() -> Self {
            let mut doc = Self::default();
            doc.ensure_layer("0");
            doc
        }

        pub fn ensure_layer(&mut self, name: impl AsRef<str>) {
            let key = name.as_ref();
            self.layers
                .entry(key.to_string())
                .or_insert_with(|| Layer::new(key));
        }

        pub fn add_line(
            &mut self,
            start: Point2,
            end: Point2,
            layer: impl Into<String>,
        ) -> EntityId {
            let layer = layer.into();
            self.ensure_layer(&layer);
            let id = self.next_id();
            self.entities
                .push((id, Entity::Line(Line { start, end, layer })));
            id
        }

        pub fn add_circle(
            &mut self,
            center: Point2,
            radius: f64,
            layer: impl Into<String>,
        ) -> EntityId {
            let layer = layer.into();
            self.ensure_layer(&layer);
            let id = self.next_id();
            self.entities.push((
                id,
                Entity::Circle(Circle {
                    center,
                    radius,
                    layer,
                }),
            ));
            id
        }

        pub fn add_arc(
            &mut self,
            center: Point2,
            radius: f64,
            start_angle: f64,
            end_angle: f64,
            layer: impl Into<String>,
        ) -> EntityId {
            let layer = layer.into();
            self.ensure_layer(&layer);
            let id = self.next_id();
            self.entities.push((
                id,
                Entity::Arc(Arc {
                    center,
                    radius,
                    start_angle,
                    end_angle,
                    layer,
                }),
            ));
            id
        }

        pub fn add_ellipse(
            &mut self,
            center: Point2,
            major_axis: Vector2,
            ratio: f64,
            start_parameter: f64,
            end_parameter: f64,
            layer: impl Into<String>,
        ) -> EntityId {
            let layer = layer.into();
            self.ensure_layer(&layer);
            let id = self.next_id();
            self.entities.push((
                id,
                Entity::Ellipse(Ellipse {
                    center,
                    major_axis,
                    ratio,
                    start_parameter,
                    end_parameter,
                    layer,
                }),
            ));
            id
        }

        pub fn add_polyline<I>(
            &mut self,
            vertices: I,
            is_closed: bool,
            layer: impl Into<String>,
        ) -> EntityId
        where
            I: IntoIterator<Item = Point2>,
        {
            let collected = vertices
                .into_iter()
                .map(PolylineVertex::new)
                .collect::<Vec<_>>();
            self.add_polyline_with_vertices(collected, is_closed, layer)
        }

        pub fn add_polyline_with_vertices<I>(
            &mut self,
            vertices: I,
            is_closed: bool,
            layer: impl Into<String>,
        ) -> EntityId
        where
            I: IntoIterator<Item = PolylineVertex>,
        {
            let layer = layer.into();
            self.ensure_layer(&layer);
            let collected: Vec<PolylineVertex> = vertices.into_iter().collect();
            let id = self.next_id();
            self.entities.push((
                id,
                Entity::Polyline(Polyline {
                    vertices: collected,
                    is_closed,
                    layer,
                }),
            ));
            id
        }

        #[allow(clippy::too_many_arguments)]
        pub fn add_spline(
            &mut self,
            degree: i32,
            is_rational: bool,
            is_closed: bool,
            is_periodic: bool,
            control_points: Vec<Point2>,
            fit_points: Vec<Point2>,
            knot_values: Vec<f64>,
            weights: Vec<f64>,
            start_tangent: Option<Vector2>,
            end_tangent: Option<Vector2>,
            layer: impl Into<String>,
        ) -> EntityId {
            let layer = layer.into();
            self.ensure_layer(&layer);
            let id = self.next_id();
            self.entities.push((
                id,
                Entity::Spline(Spline {
                    degree,
                    is_rational,
                    is_closed,
                    is_periodic,
                    control_points,
                    fit_points,
                    knot_values,
                    weights,
                    start_tangent,
                    end_tangent,
                    layer,
                }),
            ));
            id
        }

        pub fn add_text(
            &mut self,
            insert: Point2,
            content: impl Into<String>,
            height: f64,
            rotation: f64,
            layer: impl Into<String>,
        ) -> EntityId {
            let layer = layer.into();
            self.ensure_layer(&layer);
            let id = self.next_id();
            self.entities.push((
                id,
                Entity::Text(Text {
                    insert,
                    content: content.into(),
                    height,
                    rotation,
                    layer,
                }),
            ));
            id
        }

        pub fn add_mtext(
            &mut self,
            insert: Point2,
            content: impl Into<String>,
            height: f64,
            reference_width: Option<f64>,
            direction: Vector2,
            attachment_point: i16,
            drawing_direction: i16,
            style: Option<String>,
            layer: impl Into<String>,
        ) -> EntityId {
            let layer = layer.into();
            self.ensure_layer(&layer);
            let id = self.next_id();
            self.entities.push((
                id,
                Entity::MText(MText {
                    insert,
                    content: content.into(),
                    height,
                    reference_width,
                    direction,
                    attachment_point,
                    drawing_direction,
                    style,
                    layer,
                }),
            ));
            id
        }

        pub fn add_block_reference(
            &mut self,
            name: impl Into<String>,
            insert: Point2,
            scale: Vector2,
            rotation: f64,
            attributes: Vec<Attribute>,
            layer: impl Into<String>,
        ) -> EntityId {
            let layer = layer.into();
            self.ensure_layer(&layer);
            let name = name.into();
            let resolved_attributes = if attributes.is_empty() {
                self.block(&name)
                    .map(|definition| {
                        definition
                            .attributes
                            .iter()
                            .map(|def| Attribute {
                                tag: def.tag.clone(),
                                text: def.default_text.clone(),
                                insert: def.insert,
                                height: def.height,
                                rotation: def.rotation,
                                width_factor: def.width_factor,
                                oblique: def.oblique,
                                style: def.style.clone(),
                                prompt: def.prompt.clone(),
                                alignment: def.alignment,
                                horizontal_align: def.horizontal_align,
                                vertical_align: def.vertical_align,
                                line_spacing_factor: def.line_spacing_factor,
                                line_spacing_style: def.line_spacing_style,
                                is_invisible: def.is_invisible,
                                is_constant: def.is_constant,
                                is_verify: def.is_verify,
                                is_preset: def.is_preset,
                                lock_position: def.lock_position,
                                layer: def.layer.clone(),
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            } else {
                attributes
            };

            for attribute in &resolved_attributes {
                self.ensure_layer(&attribute.layer);
            }
            let id = self.next_id();
            self.entities.push((
                id,
                Entity::BlockReference(BlockReference {
                    name,
                    insert,
                    scale,
                    rotation,
                    attributes: resolved_attributes,
                    layer,
                }),
            ));
            id
        }

        pub fn add_hatch(
            &mut self,
            pattern_name: impl Into<String>,
            is_solid: bool,
            loops: Vec<HatchLoop>,
            gradient: Option<HatchGradient>,
            layer: impl Into<String>,
        ) -> EntityId {
            let layer = layer.into();
            self.ensure_layer(&layer);
            let id = self.next_id();
            self.entities.push((
                id,
                Entity::Hatch(Hatch {
                    pattern_name: pattern_name.into(),
                    is_solid,
                    loops,
                    gradient,
                    layer,
                }),
            ));
            id
        }

        #[allow(clippy::too_many_arguments)]
        pub fn add_dimension(
            &mut self,
            kind: DimensionKind,
            definition_point: Point2,
            text_midpoint: Point2,
            dimension_line_point: Option<Point2>,
            extension_line_origin: Option<Point2>,
            extension_line_end: Option<Point2>,
            secondary_point: Option<Point2>,
            arc_definition_point: Option<Point2>,
            center_point: Option<Point2>,
            text: Option<String>,
            measurement: Option<f64>,
            rotation: f64,
            text_rotation: Option<f64>,
            oblique_angle: Option<f64>,
            layer: impl Into<String>,
        ) -> EntityId {
            let layer = layer.into();
            self.ensure_layer(&layer);
            let id = self.next_id();
            self.entities.push((
                id,
                Entity::Dimension(Dimension {
                    kind,
                    definition_point,
                    text_midpoint,
                    dimension_line_point,
                    extension_line_origin,
                    extension_line_end,
                    secondary_point,
                    arc_definition_point,
                    center_point,
                    text,
                    measurement,
                    rotation,
                    text_rotation,
                    oblique_angle,
                    layer,
                }),
            ));
            id
        }

        pub fn add_leader(
            &mut self,
            vertices: Vec<Point2>,
            layer: impl Into<String>,
            style_name: Option<String>,
            has_arrowhead: bool,
        ) -> EntityId {
            let layer = layer.into();
            self.ensure_layer(&layer);
            let id = self.next_id();
            self.entities.push((
                id,
                Entity::Leader(Leader {
                    layer,
                    style_name,
                    vertices,
                    has_arrowhead,
                }),
            ));
            id
        }

        pub fn add_mleader(
            &mut self,
            leader_lines: Vec<LeaderLine>,
            layer: impl Into<String>,
            style_name: Option<String>,
            content: MLeaderContent,
            text_height: Option<f64>,
            scale: Option<f64>,
            has_dogleg: bool,
            dogleg_length: Option<f64>,
            landing_gap: Option<f64>,
        ) -> EntityId {
            let layer = layer.into();
            self.ensure_layer(&layer);
            let id = self.next_id();
            let mut content = content;
            if let MLeaderContent::Block { block } = &mut content {
                Self::resolve_block_content_name_from_handles(&self.block_handles, block);
            }
            self.entities.push((
                id,
                Entity::MLeader(MLeader {
                    layer,
                    style_name,
                    leader_lines,
                    content,
                    text_height,
                    scale,
                    has_dogleg,
                    dogleg_length,
                    landing_gap,
                }),
            ));
            id
        }

        pub fn add_face3d(
            &mut self,
            vertices: [Point3; 4],
            invisible_edges: [bool; 4],
            layer: impl Into<String>,
        ) -> EntityId {
            let layer = layer.into();
            self.ensure_layer(&layer);
            let id = self.next_id();
            self.entities.push((
                id,
                Entity::Face3D(ThreeDFace {
                    layer,
                    vertices,
                    invisible_edges,
                }),
            ));
            id
        }

        fn resolve_block_content_name_from_handles(
            handles: &HashMap<String, String>,
            block: &mut MLeaderBlockContent,
        ) {
            if block.block_name.is_none() {
                if let Some(handle) = block.block_handle.as_deref() {
                    if let Some(mapped) = handles.get(handle) {
                        block.block_name = Some(mapped.clone());
                    }
                }
            }
        }

        fn update_mleader_block_names(&mut self) {
            let handles = self.block_handles.clone();
            for (_, entity) in &mut self.entities {
                if let Entity::MLeader(mleader) = entity {
                    if let MLeaderContent::Block { block } = &mut mleader.content {
                        Self::resolve_block_content_name_from_handles(&handles, block);
                    }
                }
            }
        }

        pub fn add_raster_image_definition(
            &mut self,
            definition: RasterImageDefinition,
        ) -> Option<RasterImageDefinition> {
            self.image_definitions
                .insert(definition.handle.clone(), definition)
        }

        pub fn raster_image_definition(&self, handle: &str) -> Option<&RasterImageDefinition> {
            self.image_definitions.get(handle)
        }

        pub fn raster_image_definitions(
            &self,
        ) -> impl Iterator<Item = (&String, &RasterImageDefinition)> {
            self.image_definitions.iter()
        }

        pub fn raster_image_definition_mut(
            &mut self,
            handle: &str,
        ) -> Option<&mut RasterImageDefinition> {
            self.image_definitions.get_mut(handle)
        }

        pub fn raster_image_definitions_mut(
            &mut self,
        ) -> impl Iterator<Item = (&String, &mut RasterImageDefinition)> {
            self.image_definitions.iter_mut()
        }

        pub fn set_raster_image_resolved_path(
            &mut self,
            handle: &str,
            resolved_path: Option<String>,
        ) -> bool {
            if let Some(definition) = self.image_definitions.get_mut(handle) {
                definition.resolved_path = resolved_path;
                true
            } else {
                false
            }
        }

        pub fn add_image_def_reactor(
            &mut self,
            reactor: ImageDefReactor,
        ) -> Option<ImageDefReactor> {
            self.image_def_reactors
                .insert(reactor.handle.clone(), reactor)
        }

        pub fn image_def_reactor(&self, handle: &str) -> Option<&ImageDefReactor> {
            self.image_def_reactors.get(handle)
        }

        pub fn image_def_reactors(&self) -> impl Iterator<Item = (&String, &ImageDefReactor)> {
            self.image_def_reactors.iter()
        }

        pub fn image_def_reactor_mut(&mut self, handle: &str) -> Option<&mut ImageDefReactor> {
            self.image_def_reactors.get_mut(handle)
        }

        pub fn image_def_reactors_mut(
            &mut self,
        ) -> impl Iterator<Item = (&String, &mut ImageDefReactor)> {
            self.image_def_reactors.iter_mut()
        }

        pub fn set_image_dictionary(&mut self, dictionary: ImageDictionary) {
            self.image_dictionary = Some(dictionary);
        }

        pub fn clear_image_dictionary(&mut self) {
            self.image_dictionary = None;
        }

        pub fn image_dictionary(&self) -> Option<&ImageDictionary> {
            self.image_dictionary.as_ref()
        }

        pub fn set_raster_image_variables(&mut self, variables: RasterImageVariables) {
            self.raster_image_variables = Some(variables);
        }

        pub fn clear_raster_image_variables(&mut self) {
            self.raster_image_variables = None;
        }

        pub fn raster_image_variables(&self) -> Option<&RasterImageVariables> {
            self.raster_image_variables.as_ref()
        }

        pub fn add_raster_image(
            &mut self,
            layer: impl Into<String>,
            image_def_handle: impl Into<String>,
            insert: Point2,
            u_vector: Vector2,
            v_vector: Vector2,
            image_size: Vector2,
            display_options: RasterImageDisplayOptions,
            clip: Option<RasterImageClip>,
            image_def_reactor_handle: Option<String>,
        ) -> EntityId {
            let layer = layer.into();
            self.ensure_layer(&layer);
            let id = self.next_id();
            self.entities.push((
                id,
                Entity::RasterImage(RasterImage {
                    layer,
                    image_def_handle: image_def_handle.into(),
                    insert,
                    u_vector,
                    v_vector,
                    image_size,
                    display_options,
                    image_def_reactor_handle,
                    clip,
                }),
            ));
            id
        }

        pub fn add_wipeout(
            &mut self,
            layer: impl Into<String>,
            insert: Point2,
            u_vector: Vector2,
            v_vector: Vector2,
            image_size: Vector2,
            display_options: RasterImageDisplayOptions,
            clip: Option<RasterImageClip>,
        ) -> EntityId {
            let layer = layer.into();
            self.ensure_layer(&layer);
            let id = self.next_id();
            self.entities.push((
                id,
                Entity::Wipeout(Wipeout {
                    layer,
                    insert,
                    u_vector,
                    v_vector,
                    image_size,
                    display_options,
                    clip,
                }),
            ));
            id
        }

        pub fn add_entity(&mut self, entity: Entity) -> EntityId {
            match entity {
                Entity::Line(line) => self.add_line(line.start, line.end, line.layer),
                Entity::Circle(circle) => {
                    self.add_circle(circle.center, circle.radius, circle.layer)
                }
                Entity::Arc(arc) => self.add_arc(
                    arc.center,
                    arc.radius,
                    arc.start_angle,
                    arc.end_angle,
                    arc.layer,
                ),
                Entity::Ellipse(ellipse) => self.add_ellipse(
                    ellipse.center,
                    ellipse.major_axis,
                    ellipse.ratio,
                    ellipse.start_parameter,
                    ellipse.end_parameter,
                    ellipse.layer,
                ),
                Entity::Polyline(polyline) => self.add_polyline_with_vertices(
                    polyline.vertices,
                    polyline.is_closed,
                    polyline.layer,
                ),
                Entity::Spline(spline) => {
                    let Spline {
                        degree,
                        is_rational,
                        is_closed,
                        is_periodic,
                        control_points,
                        fit_points,
                        knot_values,
                        weights,
                        start_tangent,
                        end_tangent,
                        layer,
                    } = spline;
                    self.add_spline(
                        degree,
                        is_rational,
                        is_closed,
                        is_periodic,
                        control_points,
                        fit_points,
                        knot_values,
                        weights,
                        start_tangent,
                        end_tangent,
                        layer,
                    )
                }
                Entity::Text(text) => self.add_text(
                    text.insert,
                    text.content,
                    text.height,
                    text.rotation,
                    text.layer,
                ),
                Entity::MText(mtext) => self.add_mtext(
                    mtext.insert,
                    mtext.content,
                    mtext.height,
                    mtext.reference_width,
                    mtext.direction,
                    mtext.attachment_point,
                    mtext.drawing_direction,
                    mtext.style,
                    mtext.layer,
                ),
                Entity::BlockReference(reference) => self.add_block_reference(
                    reference.name,
                    reference.insert,
                    reference.scale,
                    reference.rotation,
                    reference.attributes,
                    reference.layer,
                ),
                Entity::Hatch(hatch) => self.add_hatch(
                    hatch.pattern_name,
                    hatch.is_solid,
                    hatch.loops,
                    hatch.gradient,
                    hatch.layer,
                ),
                Entity::Dimension(dimension) => self.add_dimension(
                    dimension.kind,
                    dimension.definition_point,
                    dimension.text_midpoint,
                    dimension.dimension_line_point,
                    dimension.extension_line_origin,
                    dimension.extension_line_end,
                    dimension.secondary_point,
                    dimension.arc_definition_point,
                    dimension.center_point,
                    dimension.text,
                    dimension.measurement,
                    dimension.rotation,
                    dimension.text_rotation,
                    dimension.oblique_angle,
                    dimension.layer,
                ),
                Entity::Leader(leader) => self.add_leader(
                    leader.vertices,
                    leader.layer,
                    leader.style_name,
                    leader.has_arrowhead,
                ),
                Entity::MLeader(mleader) => self.add_mleader(
                    mleader.leader_lines,
                    mleader.layer,
                    mleader.style_name,
                    mleader.content,
                    mleader.text_height,
                    mleader.scale,
                    mleader.has_dogleg,
                    mleader.dogleg_length,
                    mleader.landing_gap,
                ),
                Entity::RasterImage(image) => {
                    let RasterImage {
                        layer,
                        image_def_handle,
                        insert,
                        u_vector,
                        v_vector,
                        image_size,
                        display_options,
                        image_def_reactor_handle,
                        clip,
                    } = image;
                    self.add_raster_image(
                        layer,
                        image_def_handle,
                        insert,
                        u_vector,
                        v_vector,
                        image_size,
                        display_options,
                        clip,
                        image_def_reactor_handle,
                    )
                }
                Entity::Wipeout(wipeout) => {
                    let Wipeout {
                        layer,
                        insert,
                        u_vector,
                        v_vector,
                        image_size,
                        display_options,
                        clip,
                    } = wipeout;
                    self.add_wipeout(
                        layer,
                        insert,
                        u_vector,
                        v_vector,
                        image_size,
                        display_options,
                        clip,
                    )
                }
                Entity::Face3D(face) => {
                    self.add_face3d(face.vertices, face.invisible_edges, face.layer)
                }
            }
        }

        #[inline]
        pub fn layers(&self) -> impl Iterator<Item = &Layer> {
            self.layers.values()
        }

        #[inline]
        pub fn entities(&self) -> impl Iterator<Item = &(EntityId, Entity)> {
            self.entities.iter()
        }

        pub fn add_block_definition(&mut self, definition: BlockDefinition) {
            self.add_block_definition_with_handle(definition, None, None);
        }

        pub fn add_block_definition_with_handle(
            &mut self,
            definition: BlockDefinition,
            block_handle: Option<String>,
            block_record_handle: Option<String>,
        ) {
            let name = definition.name.clone();
            for entity in &definition.entities {
                self.ensure_layer(entity.layer_name());
            }
            for attr in &definition.attributes {
                self.ensure_layer(&attr.layer);
            }
            let mut updated = false;
            if let Some(handle) = block_handle {
                self.block_handles.insert(handle, name.clone());
                updated = true;
            }
            if let Some(record_handle) = block_record_handle {
                self.block_handles.insert(record_handle, name.clone());
                updated = true;
            }
            if updated {
                self.update_mleader_block_names();
            }
            self.blocks.insert(name, definition);
        }

        #[inline]
        pub fn block(&self, name: &str) -> Option<&BlockDefinition> {
            self.blocks.get(name)
        }

        #[inline]
        pub fn block_name_by_handle(&self, handle: &str) -> Option<&str> {
            self.block_handles.get(handle).map(|name| name.as_str())
        }

        #[inline]
        pub fn blocks(&self) -> impl Iterator<Item = &BlockDefinition> {
            self.blocks.values()
        }

        #[inline]
        pub fn entity(&self, id: EntityId) -> Option<&Entity> {
            self.entities.iter().find_map(|(entity_id, entity)| {
                if entity_id.get() == id.get() {
                    Some(entity)
                } else {
                    None
                }
            })
        }

        #[inline]
        pub fn entity_bounds(&self, id: EntityId) -> Option<Bounds2D> {
            self.entity(id).and_then(Entity::bounds)
        }

        pub fn bounds(&self) -> Option<Bounds2D> {
            let mut bounds = Bounds2D::empty();
            let mut has = false;
            for (_, entity) in &self.entities {
                if let Some(entity_bounds) = entity.bounds() {
                    bounds.include_bounds(&entity_bounds);
                    has = true;
                }
            }
            if has { Some(bounds) } else { None }
        }

        #[inline]
        fn next_id(&mut self) -> EntityId {
            let id = self.next_entity_id;
            self.next_entity_id += 1;
            EntityId(id)
        }
    }

    fn normalize_angle(angle: f64) -> f64 {
        let mut result = angle % TAU;
        if result < 0.0 {
            result += TAU;
        }
        result
    }

    fn canonical_interval(start: f64, end: f64) -> (f64, f64) {
        let start = normalize_angle(start);
        let mut end = normalize_angle(end);
        if (end - start).abs() < 1e-9 {
            end = start + TAU;
        } else if end < start {
            end += TAU;
        }
        (start, end)
    }

    fn arc_point(center: Point2, radius: f64, angle: f64) -> Point2 {
        let offset = Vector2::new(radius * angle.cos(), radius * angle.sin());
        center.translate(offset)
    }

    fn arc_bounds(arc: &Arc, bounds: &mut Bounds2D) {
        let radius = arc.radius.abs();
        if radius <= f64::EPSILON {
            bounds.include_point(arc.center);
            return;
        }

        let (start, end) = canonical_interval(arc.start_angle, arc.end_angle);
        bounds.include_point(arc_point(arc.center, radius, start));
        bounds.include_point(arc_point(arc.center, radius, end));

        const QUADRANTS: [f64; 4] = [0.0, FRAC_PI_2, PI, FRAC_PI_2 * 3.0];
        for base in QUADRANTS {
            let mut candidate = base;
            while candidate < start {
                candidate += TAU;
            }
            if candidate <= end {
                bounds.include_point(arc_point(arc.center, radius, candidate));
            }
        }
    }

    fn ellipse_bounds(ellipse: &Ellipse, bounds: &mut Bounds2D) {
        let major_vec = ellipse.major_axis.as_vec2();
        let major_length = major_vec.length();

        if major_length <= f64::EPSILON {
            bounds.include_point(ellipse.center);
            return;
        }
        let minor_length = major_length * ellipse.ratio.abs();
        let major_dir = major_vec / major_length;
        let minor_dir = DVec2::new(-major_dir.y, major_dir.x);
        let minor_vec = minor_dir * minor_length;

        let start = ellipse.start_parameter;
        let mut end = ellipse.end_parameter;
        if (end - start).abs() < 1e-9 {
            end = start + TAU;
        } else if end < start {
            while end < start {
                end += TAU;
            }
        }
        let span = end - start;
        let step_count = ((span / (TAU / 64.0)).ceil() as usize).max(16);
        for i in 0..=step_count {
            let t = start + span * (i as f64 / step_count as f64);
            let offset = major_vec * t.cos() + minor_vec * t.sin();
            let point = ellipse.center.translate(Vector2::from(offset));
            bounds.include_point(point);
        }
    }

    fn include_hatch_edge_bounds(edge: &HatchEdge, bounds: &mut Bounds2D) {
        match edge {
            HatchEdge::Line { start, end } => {
                bounds.include_point(*start);
                bounds.include_point(*end);
            }
            HatchEdge::Arc {
                center,
                radius,
                start_angle,
                end_angle,
                ..
            } => {
                let arc = Arc {
                    center: *center,
                    radius: *radius,
                    start_angle: *start_angle,
                    end_angle: *end_angle,
                    layer: String::new(),
                };
                arc_bounds(&arc, bounds);
            }
            HatchEdge::PolylineSegment { start, end, bulge } => {
                bounds.include_point(*start);
                bounds.include_point(*end);
                polyline_segment_bounds(*start, *end, *bulge, bounds);
            }
            HatchEdge::Ellipse {
                center,
                major_axis,
                minor_ratio,
                start_angle,
                end_angle,
                ..
            } => {
                let ellipse = Ellipse {
                    center: *center,
                    major_axis: *major_axis,
                    ratio: *minor_ratio,
                    start_parameter: *start_angle,
                    end_parameter: *end_angle,
                    layer: String::new(),
                };
                ellipse_bounds(&ellipse, bounds);
            }
            HatchEdge::BoundaryReference { .. } => {
                // 无法推导边界引用范围，交由引用实体处理。
            }
            HatchEdge::Spline {
                control_points,
                fit_points,
                ..
            } => {
                for point in control_points.iter().chain(fit_points.iter()) {
                    bounds.include_point(*point);
                }
            }
        }
    }

    fn polyline_segment_bounds(start: Point2, end: Point2, bulge: f64, bounds: &mut Bounds2D) {
        if bulge.abs() <= 1e-9 {
            return;
        }

        let start_vec = start.as_vec2();
        let end_vec = end.as_vec2();
        let chord = end_vec - start_vec;
        let chord_len = chord.length();
        if chord_len <= f64::EPSILON {
            return;
        }

        let theta = 4.0 * bulge.atan();
        if theta.abs() <= 1e-9 {
            return;
        }

        let half_theta = theta / 2.0;
        let sin_half = half_theta.sin();
        if sin_half.abs() <= 1e-9 {
            return;
        }

        let radius = chord_len / (2.0 * sin_half);
        let midpoint = (start_vec + end_vec) * 0.5;
        let perp = DVec2::new(-chord.y, chord.x);
        if perp.length_squared() <= f64::EPSILON {
            return;
        }
        let perp_dir = perp.normalize();
        let sagitta = bulge * chord_len / 2.0;
        let center_vec = midpoint + perp_dir * sagitta;
        let center = Point2::from_vec(center_vec);

        let start_dir = start_vec - center_vec;
        let start_angle = start_dir.y.atan2(start_dir.x);
        let end_angle = start_angle + theta;

        let arc = Arc {
            center,
            radius: radius.abs(),
            start_angle,
            end_angle,
            layer: String::new(),
        };
        arc_bounds(&arc, bounds);
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::geometry::{Point2, Point3, Vector2};
        use std::f64::consts::{FRAC_PI_2, PI};

        #[test]
        fn document_stores_entities() {
            let mut doc = Document::new();
            let id = doc.add_line(Point2::new(0.0, 0.0), Point2::new(10.0, 0.0), "0");
            let circle_id = doc.add_circle(Point2::new(5.0, 5.0), 2.0, "ANNOT");
            let arc_id = doc.add_arc(Point2::new(5.0, 0.0), 3.5, 0.0, FRAC_PI_2, "GEOM");
            let ellipse_id = doc.add_ellipse(
                Point2::new(15.0, 5.0),
                Vector2::new(4.0, 0.0),
                0.5,
                0.0,
                PI,
                "GEOM",
            );
            let polyline_id = doc.add_polyline(
                [
                    Point2::new(0.0, 0.0),
                    Point2::new(2.0, 2.0),
                    Point2::new(4.0, 0.0),
                ],
                true,
                "SHAPE",
            );
            let text_id = doc.add_text(Point2::new(1.0, 1.0), "Hello", 2.5, PI, "ANNOT");

            assert_eq!(id.get(), 0);
            assert_eq!(circle_id.get(), 1);
            assert_eq!(arc_id.get(), 2);
            assert_eq!(ellipse_id.get(), 3);
            assert_eq!(polyline_id.get(), 4);
            assert_eq!(text_id.get(), 5);
            let layers: Vec<_> = doc.layers().map(|l| l.name.clone()).collect();
            assert!(layers.contains(&"0".to_string()));
            assert!(layers.contains(&"ANNOT".to_string()));
            assert!(layers.contains(&"GEOM".to_string()));
            assert!(layers.contains(&"SHAPE".to_string()));
            assert_eq!(doc.entities().count(), 6);

            match doc.entity(arc_id) {
                Some(Entity::Arc(arc)) => {
                    assert_eq!(arc.layer, "GEOM");
                    assert!((arc.radius - 3.5).abs() < f64::EPSILON);
                }
                other => panic!("unexpected entity lookup result: {other:?}"),
            }

            match doc.entity(ellipse_id) {
                Some(Entity::Ellipse(ellipse)) => {
                    assert_eq!(ellipse.layer, "GEOM");
                    assert!((ellipse.ratio - 0.5).abs() < f64::EPSILON);
                    let axis = ellipse.major_axis.as_vec2();
                    assert!((axis.x - 4.0).abs() < f64::EPSILON);
                    assert!(axis.y.abs() < f64::EPSILON);
                }
                _ => panic!("expected ellipse entity"),
            }

            match doc.entity(text_id) {
                Some(Entity::Text(text)) => {
                    assert_eq!(text.content, "Hello");
                }
                _ => panic!("expected text entity"),
            }

            let mtext_id = doc.add_mtext(
                Point2::new(2.0, 3.0),
                "Multi-line",
                1.5,
                None,
                Vector2::new(1.0, 0.0),
                5,
                1,
                Some("Standard".to_string()),
                "ANNOT",
            );

            match doc.entity(mtext_id) {
                Some(Entity::MText(mtext)) => {
                    assert_eq!(mtext.content, "Multi-line");
                    assert_eq!(mtext.style.as_deref(), Some("Standard"));
                }
                _ => panic!("expected mtext entity"),
            }

            let definition = BlockDefinition {
                name: "BlockA".to_string(),
                base_point: Point2::new(0.0, 0.0),
                entities: vec![],
                attributes: vec![AttributeDefinition {
                    tag: "ID".to_string(),
                    prompt: Some("Prompt".to_string()),
                    default_text: "100".to_string(),
                    insert: Point2::new(0.0, 0.0),
                    height: 1.0,
                    rotation: 0.0,
                    width_factor: 1.0,
                    oblique: 0.0,
                    style: Some("Standard".to_string()),
                    alignment: None,
                    horizontal_align: 0,
                    vertical_align: 0,
                    line_spacing_factor: 1.0,
                    line_spacing_style: 0,
                    is_invisible: false,
                    is_constant: false,
                    is_verify: false,
                    is_preset: false,
                    lock_position: false,
                    layer: "ATTR".to_string(),
                }],
            };
            doc.add_block_definition(definition);
            let block_def = doc.block("BlockA").expect("block definition missing");
            assert_eq!(block_def.attributes.len(), 1);

            let attribute = Attribute {
                tag: "ID".to_string(),
                text: "42".to_string(),
                insert: Point2::new(10.0, 10.0),
                height: 1.0,
                rotation: 0.0,
                width_factor: 1.0,
                oblique: 0.0,
                style: Some("Standard".to_string()),
                prompt: Some("Prompt".to_string()),
                alignment: None,
                horizontal_align: 0,
                vertical_align: 0,
                line_spacing_factor: 1.0,
                line_spacing_style: 0,
                is_invisible: false,
                is_constant: false,
                is_verify: false,
                is_preset: false,
                lock_position: false,
                layer: "0".to_string(),
            };
            let insert_id = doc.add_block_reference(
                "BlockA",
                Point2::new(10.0, 10.0),
                Vector2::new(1.0, 1.0),
                0.0,
                vec![attribute],
                "0",
            );
            match doc.entity(insert_id) {
                Some(Entity::BlockReference(block)) => {
                    assert_eq!(block.attributes.len(), 1);
                    let attr = &block.attributes[0];
                    assert_eq!(attr.tag, "ID");
                    assert_eq!(attr.style.as_deref(), Some("Standard"));
                    assert!(!attr.is_invisible);
                    assert_eq!(attr.line_spacing_style, 0);
                    assert!((attr.line_spacing_factor - 1.0).abs() < f64::EPSILON);
                }
                _ => panic!("expected block reference entity"),
            }

            let bounds = doc.bounds().expect("document bounds should exist");
            assert!((bounds.min().x() - 0.0).abs() < 1e-9);
            assert!((bounds.min().y() - 0.0).abs() < 1e-9);
            assert!((bounds.max().x() - 19.0).abs() < 1e-9);
            assert!((bounds.max().y() - 10.0).abs() < 1e-9);
        }

        #[test]
        fn three_d_face_normal_is_cross_product() {
            let face = ThreeDFace {
                layer: "3D".to_string(),
                vertices: [
                    Point3::new(0.0, 0.0, 0.0),
                    Point3::new(10.0, 0.0, 0.0),
                    Point3::new(0.0, 5.0, 0.0),
                    Point3::new(0.0, 5.0, 0.0),
                ],
                invisible_edges: [false; 4],
            };
            let normal = face.normal().expect("should compute normal").as_vec3();
            assert!((normal.x).abs() < 1e-9);
            assert!((normal.y).abs() < 1e-9);
            assert!((normal.z - 50.0).abs() < 1e-9);
            assert!((face.average_height()).abs() < 1e-9);
        }

        #[test]
        fn three_d_face_normal_none_for_degenerate_face() {
            let face = ThreeDFace {
                layer: "3D".to_string(),
                vertices: [
                    Point3::new(0.0, 0.0, 0.0),
                    Point3::new(1.0, 1.0, 1.0),
                    Point3::new(2.0, 2.0, 2.0),
                    Point3::new(2.0, 2.0, 2.0),
                ],
                invisible_edges: [false; 4],
            };
            assert!(face.normal().is_none());
        }
    }
}
