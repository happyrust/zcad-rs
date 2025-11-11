use std::collections::HashMap;
use std::convert::TryFrom;
use std::fs;
use std::path::Path;

use thiserror::Error;
use zcad_core::{
    document::{
        Arc, Attribute, AttributeDefinition, BlockDefinition, BlockReference, Circle, ClipMode,
        Dimension, DimensionKind, Document, Ellipse, Entity, Hatch, HatchEdge, HatchGradient,
        HatchLoop, ImageDefReactor, ImageDictionary, ImageDictionaryEntry, Leader, LeaderLine,
        Line, MLeader, MLeaderBlockContent, MLeaderContent, MText, Polyline, PolylineVertex,
        RasterImage, RasterImageClip, RasterImageDefinition, RasterImageDisplayOptions,
        RasterImageVariables, Spline, Text, ThreeDFace, Wipeout,
    },
    geometry::{Point2, Point3, Vector2},
};

#[derive(Debug, Error)]
pub enum IoError {
    #[error("unsupported feature: {0}")]
    UnsupportedFeature(String),
    #[error("failed to read file {path:?}: {source}")]
    ReadError {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to write file {path:?}: {source}")]
    WriteError {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid document structure: {0}")]
    InvalidDocument(String),
}

pub trait DocumentLoader {
    fn load(&self, path: &Path) -> Result<Document, IoError>;
}

pub trait DocumentSaver {
    fn save(&self, document: &Document, path: &Path) -> Result<(), IoError>;
}

pub struct DxfFacade;

impl DxfFacade {
    pub fn new() -> Self {
        Self
    }
}

impl DocumentLoader for DxfFacade {
    fn load(&self, path: &Path) -> Result<Document, IoError> {
        let data = fs::read_to_string(path).map_err(|source| IoError::ReadError {
            path: path.to_path_buf(),
            source,
        })?;
        let parser = DxfParser::new(&data);
        parser.parse().map_err(|err| match err {
            DxfError::Unsupported { feature } => IoError::UnsupportedFeature(feature),
            DxfError::Invalid { message } => IoError::InvalidDocument(message),
        })
    }
}

impl DocumentSaver for DxfFacade {
    fn save(&self, _document: &Document, path: &Path) -> Result<(), IoError> {
        Err(IoError::UnsupportedFeature(format!(
            "DXF writer for {:?} 尚未实现",
            path
        )))
    }
}

#[derive(Debug)]
enum DxfError {
    Unsupported { feature: String },
    Invalid { message: String },
}

impl DxfError {
    fn unsupported(feature: impl Into<String>) -> Self {
        Self::Unsupported {
            feature: feature.into(),
        }
    }

    fn invalid(message: impl Into<String>) -> Self {
        Self::Invalid {
            message: message.into(),
        }
    }
}

struct DxfParser<'a> {
    reader: DxfReader<'a>,
}

#[derive(Debug)]
struct ParsedDictionary {
    handle: String,
    owner: Option<String>,
    entries: Vec<DictionaryEntry>,
}

#[derive(Debug)]
struct DictionaryEntry {
    name: String,
    handle: String,
}

enum PolyfaceRecord {
    Coordinate(Point3),
    Face { indices: [i32; 4] },
    Ignored,
}

impl<'a> DxfParser<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            reader: DxfReader::new(source),
        }
    }

    fn parse(mut self) -> Result<Document, DxfError> {
        let mut document = Document::new();
        while let Some((code, value)) = self.reader.next_pair()? {
            if code != 0 {
                return Err(DxfError::invalid(format!(
                    "意外的组码 {code}（期望 0 表示 SECTION/EOF）"
                )));
            }
            match value.as_str() {
                "SECTION" => {
                    let (name_code, name) = self
                        .reader
                        .next_pair()?
                        .ok_or_else(|| DxfError::invalid("SECTION 缺少名称（组码 2）"))?;
                    if name_code != 2 {
                        return Err(DxfError::invalid(format!(
                            "SECTION 名称使用了组码 {name_code}（期望 2）"
                        )));
                    }
                    match name.as_str() {
                        "ENTITIES" => self.parse_entities(&mut document)?,
                        "BLOCKS" => self.parse_blocks(&mut document)?,
                        "OBJECTS" => self.parse_objects(&mut document)?,
                        _ => self.skip_section()?,
                    }
                }
                "EOF" => break,
                unexpected => {
                    return Err(DxfError::invalid(format!(
                        "意外的标记 {unexpected}，期望 SECTION 或 EOF"
                    )));
                }
            }
        }
        Ok(document)
    }

    fn skip_section(&mut self) -> Result<(), DxfError> {
        loop {
            match self.reader.next_pair()? {
                Some((0, value)) if value == "ENDSEC" => break,
                Some(_) => continue,
                None => {
                    return Err(DxfError::invalid("SECTION 未找到 ENDSEC 终止标记"));
                }
            }
        }
        Ok(())
    }

    fn parse_entities(&mut self, document: &mut Document) -> Result<(), DxfError> {
        loop {
            let (code, value) = match self.reader.next_pair()? {
                Some(pair) => pair,
                None => return Err(DxfError::invalid("ENTITIES 段提前结束")),
            };
            if code != 0 {
                return Err(DxfError::invalid(format!(
                    "ENTITIES 段遇到组码 {code}（期望 0 表示实体起始）"
                )));
            }

            match value.as_str() {
                "ENDSEC" => break,
                "SEQEND" => {
                    self.skip_entity_body()?;
                }
                "POLYLINE" => {
                    self.parse_polyline_entity(document)?;
                }
                entity => {
                    let parsed = self.parse_entity(entity)?;
                    document.add_entity(parsed);
                }
            }
        }
        Ok(())
    }

    fn parse_blocks(&mut self, document: &mut Document) -> Result<(), DxfError> {
        loop {
            let (code, value) = match self.reader.next_pair()? {
                Some(pair) => pair,
                None => return Err(DxfError::invalid("BLOCKS 段提前结束")),
            };
            if code != 0 {
                return Err(DxfError::invalid(format!(
                    "BLOCKS 段遇到组码 {code}（期望 0 表示实体起始）"
                )));
            }

            match value.as_str() {
                "ENDSEC" => break,
                "BLOCK" => {
                    if let Some((definition, block_handle, record_handle)) =
                        self.parse_block_definition()?
                    {
                        document.add_block_definition_with_handle(
                            definition,
                            block_handle,
                            record_handle,
                        );
                    }
                }
                _ => {
                    // 未预期的条目（例如嵌套记录），直接跳过
                    self.skip_entity_body()?;
                }
            }
        }
        Ok(())
    }

    fn parse_objects(&mut self, document: &mut Document) -> Result<(), DxfError> {
        let mut dictionaries: HashMap<String, ParsedDictionary> = HashMap::new();
        let mut root_entries: HashMap<String, String> = HashMap::new();
        let mut raster_variables_by_handle: HashMap<String, RasterImageVariables> = HashMap::new();
        let mut reactor_by_owner: HashMap<String, String> = HashMap::new();

        loop {
            let (code, value) = match self.reader.next_pair()? {
                Some(pair) => pair,
                None => return Err(DxfError::invalid("OBJECTS 段提前结束")),
            };
            if code != 0 {
                return Err(DxfError::invalid(format!(
                    "OBJECTS 段遇到组码 {code}（期望 0 表示对象起始）"
                )));
            }

            match value.as_str() {
                "ENDSEC" => break,
                "IMAGEDEF" => {
                    let def = self.parse_image_def()?;
                    document.add_raster_image_definition(def);
                }
                "IMAGEDEF_REACTOR" => {
                    let reactor = self.parse_image_def_reactor()?;
                    if let Some(owner) = reactor.owner_handle.as_ref() {
                        reactor_by_owner.insert(owner.clone(), reactor.handle.clone());
                    }
                    document.add_image_def_reactor(reactor);
                }
                "RASTERVARIABLES" => {
                    let (handle, vars) = self.parse_raster_variables()?;
                    raster_variables_by_handle.insert(handle, vars);
                }
                "DICTIONARY" => {
                    let dict = self.parse_dictionary()?;
                    if dict.owner.as_deref() == Some("0") {
                        for entry in &dict.entries {
                            root_entries
                                .entry(entry.name.clone())
                                .or_insert_with(|| entry.handle.clone());
                        }
                    }
                    dictionaries.insert(dict.handle.clone(), dict);
                }
                _ => {
                    self.skip_entity_body()?;
                }
            }
        }

        if let Some(dict_handle) = root_entries.get("ACAD_IMAGE_DICT") {
            if let Some(dict) = dictionaries.get(dict_handle) {
                let mut image_dict = ImageDictionary {
                    handle: Some(dict.handle.clone()),
                    entries: dict
                        .entries
                        .iter()
                        .map(|entry| ImageDictionaryEntry {
                            name: entry.name.clone(),
                            image_def_handle: entry.handle.clone(),
                            reactor_handle: reactor_by_owner.get(&entry.handle).cloned(),
                        })
                        .collect(),
                };
                image_dict.entries.sort_by(|a, b| a.name.cmp(&b.name));
                document.set_image_dictionary(image_dict);
            }
        }

        if let Some(vars_handle) = root_entries.get("ACAD_IMAGE_VARS") {
            if let Some(vars) = raster_variables_by_handle.get(vars_handle) {
                document.set_raster_image_variables(vars.clone());
            }
        }

        Ok(())
    }

    fn parse_block_definition(
        &mut self,
    ) -> Result<Option<(BlockDefinition, Option<String>, Option<String>)>, DxfError> {
        let mut name: Option<String> = None;
        let mut base_x: f64 = 0.0;
        let mut base_y: f64 = 0.0;
        let mut collect_entities = true;
        let mut entities: Vec<Entity> = Vec::new();
        let mut attribute_defs: Vec<AttributeDefinition> = Vec::new();
        let mut block_handle: Option<String> = None;
        let mut record_handle: Option<String> = None;

        loop {
            match self.reader.next_pair()? {
                Some((0, value)) => match value.as_str() {
                    "ENDBLK" => {
                        self.skip_entity_body()?;
                        break;
                    }
                    entity_kind => {
                        if collect_entities {
                            match entity_kind {
                                "ATTDEF" => {
                                    let attr_def = self.parse_attdef()?;
                                    attribute_defs.push(attr_def);
                                }
                                _ => match self.parse_entity(entity_kind) {
                                    Ok(entity) => entities.push(entity),
                                    Err(DxfError::Unsupported { .. }) => {
                                        self.skip_entity_body()?;
                                    }
                                    Err(err) => return Err(err),
                                },
                            }
                        } else {
                            self.skip_entity_body()?;
                        }
                    }
                },
                Some((code, value)) => match code {
                    2 => {
                        let trimmed = value.trim().to_string();
                        collect_entities = !trimmed.starts_with('*');
                        name = Some(trimmed);
                    }
                    10 => base_x = parse_f64(&value, "BLOCK 基点 X")?,
                    20 => base_y = parse_f64(&value, "BLOCK 基点 Y")?,
                    30 | 70 | 71 | 62 | 3 | 1 | 4 | 8 | 100 | 102 => {
                        // 暂时忽略的字段
                    }
                    330 => {
                        let trimmed = value.trim();
                        if !trimmed.is_empty() && record_handle.is_none() {
                            record_handle = Some(trimmed.to_string());
                        }
                    }
                    5 => {
                        let trimmed = value.trim();
                        if !trimmed.is_empty() {
                            block_handle = Some(trimmed.to_string());
                        }
                    }
                    _ => {}
                },
                None => {
                    return Err(DxfError::invalid("BLOCK 定义未找到 ENDBLK 终止标记"));
                }
            }
        }

        let name = match name {
            Some(name) => name,
            None => return Err(DxfError::invalid("BLOCK 缺少名称（组码 2）")),
        };

        if !collect_entities {
            return Ok(None);
        }

        Ok(Some((
            BlockDefinition {
                name,
                base_point: Point2::new(base_x, base_y),
                entities,
                attributes: attribute_defs,
            },
            block_handle,
            record_handle,
        )))
    }

    fn parse_entity(&mut self, kind: &str) -> Result<Entity, DxfError> {
        match kind {
            "LINE" => self.parse_line(),
            "CIRCLE" => self.parse_circle(),
            "ARC" => self.parse_arc(),
            "ELLIPSE" => self.parse_ellipse(),
            "LWPOLYLINE" => self.parse_lwpolyline(),
            "TEXT" => self.parse_text(),
            "MTEXT" => self.parse_mtext(),
            "INSERT" => self.parse_insert(),
            "HATCH" => self.parse_hatch(),
            "DIMENSION" => self.parse_dimension(),
            "SPLINE" => self.parse_spline(),
            "LEADER" => self.parse_leader(),
            "MULTILEADER" => self.parse_mleader(),
            "IMAGE" => self.parse_image(),
            "WIPEOUT" => self.parse_wipeout(),
            "3DFACE" => self.parse_3dface(),
            other => Err(DxfError::unsupported(format!("暂不支持的实体类型 {other}"))),
        }
    }

    fn parse_line(&mut self) -> Result<Entity, DxfError> {
        let mut layer = None;
        let mut start_x = None;
        let mut start_y = None;
        let mut end_x = None;
        let mut end_y = None;
        loop {
            match self.reader.next_pair()? {
                Some((0, value)) => {
                    self.reader.put_back((0, value));
                    break;
                }
                Some((code, value)) => match code {
                    8 => layer = Some(value.trim().to_string()),
                    10 => {
                        if start_x.is_some() {
                            return Err(DxfError::invalid("LINE 遇到重复的起点 X（组码 10）"));
                        }
                        start_x = Some(parse_f64(&value, "LINE 起点 X")?);
                    }
                    20 => {
                        if start_y.is_some() {
                            return Err(DxfError::invalid("LINE 遇到重复的起点 Y（组码 20）"));
                        }
                        start_y = Some(parse_f64(&value, "LINE 起点 Y")?);
                    }
                    11 => {
                        if end_x.is_some() {
                            return Err(DxfError::invalid("LINE 遇到重复的终点 X（组码 11）"));
                        }
                        end_x = Some(parse_f64(&value, "LINE 终点 X")?);
                    }
                    21 => {
                        if end_y.is_some() {
                            return Err(DxfError::invalid("LINE 遇到重复的终点 Y（组码 21）"));
                        }
                        end_y = Some(parse_f64(&value, "LINE 终点 Y")?);
                    }
                    30 | 31 => {} // 忽略 Z 坐标
                    _ => {}
                },
                None => return Err(DxfError::invalid("LINE 未正确结束")),
            }
        }

        let layer = layer.unwrap_or_else(|| "0".to_string());
        let sx = start_x.ok_or_else(|| DxfError::invalid("LINE 缺少起点 X（组码 10）"))?;
        let sy = start_y.ok_or_else(|| DxfError::invalid("LINE 缺少起点 Y（组码 20）"))?;
        let ex = end_x.ok_or_else(|| DxfError::invalid("LINE 缺少终点 X（组码 11）"))?;
        let ey = end_y.ok_or_else(|| DxfError::invalid("LINE 缺少终点 Y（组码 21）"))?;

        Ok(Entity::Line(Line {
            start: Point2::new(sx, sy),
            end: Point2::new(ex, ey),
            layer,
        }))
    }

    fn parse_circle(&mut self) -> Result<Entity, DxfError> {
        let mut layer = None;
        let mut center_x = None;
        let mut center_y = None;
        let mut radius = None;
        loop {
            match self.reader.next_pair()? {
                Some((0, value)) => {
                    self.reader.put_back((0, value));
                    break;
                }
                Some((code, value)) => match code {
                    8 => layer = Some(value.trim().to_string()),
                    10 => {
                        if center_x.is_some() {
                            return Err(DxfError::invalid("CIRCLE 遇到重复的圆心 X（组码 10）"));
                        }
                        center_x = Some(parse_f64(&value, "CIRCLE 圆心 X")?);
                    }
                    20 => {
                        if center_y.is_some() {
                            return Err(DxfError::invalid("CIRCLE 遇到重复的圆心 Y（组码 20）"));
                        }
                        center_y = Some(parse_f64(&value, "CIRCLE 圆心 Y")?);
                    }
                    40 => {
                        if radius.is_some() {
                            return Err(DxfError::invalid("CIRCLE 遇到重复的半径（组码 40）"));
                        }
                        radius = Some(parse_f64(&value, "CIRCLE 半径")?);
                    }
                    30 => {}
                    _ => {}
                },
                None => return Err(DxfError::invalid("CIRCLE 未正确结束")),
            }
        }

        let layer = layer.unwrap_or_else(|| "0".to_string());
        let cx = center_x.ok_or_else(|| DxfError::invalid("CIRCLE 缺少圆心 X（组码 10）"))?;
        let cy = center_y.ok_or_else(|| DxfError::invalid("CIRCLE 缺少圆心 Y（组码 20）"))?;
        let radius = radius.ok_or_else(|| DxfError::invalid("CIRCLE 缺少半径（组码 40）"))?;

        Ok(Entity::Circle(Circle {
            center: Point2::new(cx, cy),
            radius,
            layer,
        }))
    }

    fn parse_arc(&mut self) -> Result<Entity, DxfError> {
        let mut layer = None;
        let mut center_x = None;
        let mut center_y = None;
        let mut radius = None;
        let mut start_angle = None;
        let mut end_angle = None;
        loop {
            match self.reader.next_pair()? {
                Some((0, value)) => {
                    self.reader.put_back((0, value));
                    break;
                }
                Some((code, value)) => match code {
                    8 => layer = Some(value.trim().to_string()),
                    10 => {
                        if center_x.is_some() {
                            return Err(DxfError::invalid("ARC 遇到重复的圆心 X（组码 10）"));
                        }
                        center_x = Some(parse_f64(&value, "ARC 圆心 X")?);
                    }
                    20 => {
                        if center_y.is_some() {
                            return Err(DxfError::invalid("ARC 遇到重复的圆心 Y（组码 20）"));
                        }
                        center_y = Some(parse_f64(&value, "ARC 圆心 Y")?);
                    }
                    40 => {
                        if radius.is_some() {
                            return Err(DxfError::invalid("ARC 遇到重复的半径（组码 40）"));
                        }
                        radius = Some(parse_f64(&value, "ARC 半径")?);
                    }
                    50 => {
                        if start_angle.is_some() {
                            return Err(DxfError::invalid("ARC 遇到重复的起始角（组码 50）"));
                        }
                        start_angle = Some(parse_f64(&value, "ARC 起始角")?.to_radians());
                    }
                    51 => {
                        if end_angle.is_some() {
                            return Err(DxfError::invalid("ARC 遇到重复的终止角（组码 51）"));
                        }
                        end_angle = Some(parse_f64(&value, "ARC 终止角")?.to_radians());
                    }
                    30 => {}
                    _ => {}
                },
                None => return Err(DxfError::invalid("ARC 未正确结束")),
            }
        }

        let layer = layer.unwrap_or_else(|| "0".to_string());
        let cx = center_x.ok_or_else(|| DxfError::invalid("ARC 缺少圆心 X（组码 10）"))?;
        let cy = center_y.ok_or_else(|| DxfError::invalid("ARC 缺少圆心 Y（组码 20）"))?;
        let radius = radius.ok_or_else(|| DxfError::invalid("ARC 缺少半径（组码 40）"))?;
        let start_angle =
            start_angle.ok_or_else(|| DxfError::invalid("ARC 缺少起始角（组码 50）"))?;
        let end_angle = end_angle.ok_or_else(|| DxfError::invalid("ARC 缺少终止角（组码 51）"))?;

        Ok(Entity::Arc(Arc {
            center: Point2::new(cx, cy),
            radius,
            start_angle,
            end_angle,
            layer,
        }))
    }

    fn parse_ellipse(&mut self) -> Result<Entity, DxfError> {
        let mut layer = None;
        let mut center_x = None;
        let mut center_y = None;
        let mut major_x = None;
        let mut major_y = None;
        let mut ratio = None;
        let mut start_parameter = 0.0;
        let mut end_parameter = std::f64::consts::TAU;
        let mut has_start = false;
        let mut has_end = false;

        loop {
            match self.reader.next_pair()? {
                Some((0, value)) => {
                    self.reader.put_back((0, value));
                    break;
                }
                Some((code, value)) => match code {
                    8 => layer = Some(value.trim().to_string()),
                    10 => {
                        if center_x.is_some() {
                            return Err(DxfError::invalid("ELLIPSE 遇到重复的圆心 X（组码 10）"));
                        }
                        center_x = Some(parse_f64(&value, "ELLIPSE 圆心 X")?);
                    }
                    20 => {
                        if center_y.is_some() {
                            return Err(DxfError::invalid("ELLIPSE 遇到重复的圆心 Y（组码 20）"));
                        }
                        center_y = Some(parse_f64(&value, "ELLIPSE 圆心 Y")?);
                    }
                    11 => {
                        if major_x.is_some() {
                            return Err(DxfError::invalid(
                                "ELLIPSE 遇到重复的主轴向量 X（组码 11）",
                            ));
                        }
                        major_x = Some(parse_f64(&value, "ELLIPSE 主轴向量 X")?);
                    }
                    21 => {
                        if major_y.is_some() {
                            return Err(DxfError::invalid(
                                "ELLIPSE 遇到重复的主轴向量 Y（组码 21）",
                            ));
                        }
                        major_y = Some(parse_f64(&value, "ELLIPSE 主轴向量 Y")?);
                    }
                    30 | 31 | 210 | 220 | 230 => {
                        // 当前阶段忽略 Z 分量与法向量。后续如需三维支持再扩展。
                    }
                    40 => {
                        if ratio.is_some() {
                            return Err(DxfError::invalid("ELLIPSE 遇到重复的半径比（组码 40）"));
                        }
                        ratio = Some(parse_f64(&value, "ELLIPSE 半径比")?);
                    }
                    41 => {
                        start_parameter = parse_f64(&value, "ELLIPSE 起始参数")?;
                        has_start = true;
                    }
                    42 => {
                        end_parameter = parse_f64(&value, "ELLIPSE 终止参数")?;
                        has_end = true;
                    }
                    12 | 22 | 32 | 100 | 102 | 370 | 440 | 50 | 51 => {
                        // 目前未使用的字段：第二个轴向量、扩展数据、线宽等
                    }
                    _ => {}
                },
                None => return Err(DxfError::invalid("ELLIPSE 未正确结束")),
            }
        }

        let layer = layer.unwrap_or_else(|| "0".to_string());
        let cx = center_x.ok_or_else(|| DxfError::invalid("ELLIPSE 缺少圆心 X（组码 10）"))?;
        let cy = center_y.ok_or_else(|| DxfError::invalid("ELLIPSE 缺少圆心 Y（组码 20）"))?;
        let major_x =
            major_x.ok_or_else(|| DxfError::invalid("ELLIPSE 缺少主轴向量 X（组码 11）"))?;
        let major_y =
            major_y.ok_or_else(|| DxfError::invalid("ELLIPSE 缺少主轴向量 Y（组码 21）"))?;

        if major_x.abs() < f64::EPSILON && major_y.abs() < f64::EPSILON {
            return Err(DxfError::invalid("ELLIPSE 主轴向量长度为 0，无法创建实体"));
        }

        let ratio = ratio.unwrap_or(1.0);
        if ratio <= 0.0 {
            return Err(DxfError::invalid(format!(
                "ELLIPSE 半径比必须为正数，实际为 {ratio}"
            )));
        }

        if !has_start {
            start_parameter = 0.0;
        }
        if !has_end {
            end_parameter = std::f64::consts::TAU;
        }

        Ok(Entity::Ellipse(Ellipse {
            center: Point2::new(cx, cy),
            major_axis: Vector2::new(major_x, major_y),
            ratio,
            start_parameter,
            end_parameter,
            layer,
        }))
    }

    fn parse_lwpolyline(&mut self) -> Result<Entity, DxfError> {
        let mut layer = None;
        let mut is_closed = false;
        let mut vertices: Vec<PolylineVertex> = Vec::new();
        let mut pending_x: Option<f64> = None;
        let mut pending_y: Option<f64> = None;
        let mut last_vertex_index: Option<usize> = None;
        loop {
            match self.reader.next_pair()? {
                Some((0, value)) => {
                    self.reader.put_back((0, value));
                    break;
                }
                Some((code, value)) => match code {
                    8 => layer = Some(value.trim().to_string()),
                    70 => {
                        let flag = parse_i32(&value, "LWPOLYLINE 标志")?;
                        is_closed = flag & 0x01 == 0x01;
                    }
                    90 => {}
                    10 => {
                        let x = parse_f64(&value, "LWPOLYLINE 顶点 X")?;
                        if let Some(y) = pending_y.take() {
                            let vertex = PolylineVertex::new(Point2::new(x, y));
                            vertices.push(vertex);
                            last_vertex_index = Some(vertices.len() - 1);
                        } else {
                            if pending_x.replace(x).is_some() {
                                return Err(DxfError::invalid(
                                    "LWPOLYLINE 顶点缺少对应的 Y（组码 20）",
                                ));
                            }
                        }
                    }
                    20 => {
                        let y = parse_f64(&value, "LWPOLYLINE 顶点 Y")?;
                        if let Some(x) = pending_x.take() {
                            let vertex = PolylineVertex::new(Point2::new(x, y));
                            vertices.push(vertex);
                            last_vertex_index = Some(vertices.len() - 1);
                        } else {
                            if pending_y.replace(y).is_some() {
                                return Err(DxfError::invalid(
                                    "LWPOLYLINE 顶点缺少对应的 X（组码 10）",
                                ));
                            }
                        }
                    }
                    30 => {}
                    42 => {
                        let bulge = parse_f64(&value, "LWPOLYLINE 顶点 bulge")?;
                        match last_vertex_index {
                            Some(idx) => {
                                if let Some(vertex) = vertices.get_mut(idx) {
                                    vertex.bulge = bulge;
                                } else {
                                    return Err(DxfError::invalid("LWPOLYLINE 内部错误：索引越界"));
                                }
                            }
                            None => {
                                return Err(DxfError::invalid(
                                    "LWPOLYLINE 在定义首个顶点前遇到 bulge（组码 42）",
                                ));
                            }
                        }
                    }
                    _ => {}
                },
                None => return Err(DxfError::invalid("LWPOLYLINE 未正确结束")),
            }
        }

        if pending_x.is_some() || pending_y.is_some() {
            return Err(DxfError::invalid(
                "LWPOLYLINE 顶点坐标成对出现（组码 10/20），检测到不完整的顶点",
            ));
        }

        if vertices.is_empty() {
            return Err(DxfError::invalid("LWPOLYLINE 未解析到任何顶点"));
        }

        let layer = layer.unwrap_or_else(|| "0".to_string());
        Ok(Entity::Polyline(Polyline {
            vertices,
            is_closed,
            layer,
        }))
    }

    fn parse_polyline_entity(&mut self, document: &mut Document) -> Result<(), DxfError> {
        let mut layer = None;
        let mut flags: Option<i16> = None;
        let mut mesh_rows: Option<i16> = None;
        let mut mesh_cols: Option<i16> = None;
        loop {
            match self.reader.next_pair()? {
                Some((0, value)) => {
                    self.reader.put_back((0, value));
                    break;
                }
                Some((code, value)) => match code {
                    8 => layer = Some(value.trim().to_string()),
                    70 => flags = Some(parse_i16(&value, "POLYLINE 标志（组码 70）")?),
                    71 => mesh_rows = Some(parse_i16(&value, "POLYLINE 网格行数（组码 71）")?),
                    72 => mesh_cols = Some(parse_i16(&value, "POLYLINE 网格列数（组码 72）")?),
                    66 | 73 | 74 | 75 => {
                        // 读但暂不使用
                    }
                    _ => {}
                },
                None => return Err(DxfError::invalid("POLYLINE 未正确结束")),
            }
        }

        let flags = flags.unwrap_or(0);
        let layer = layer.unwrap_or_else(|| "0".to_string());
        if flags & 0x40 != 0 {
            return self.parse_polyface_mesh(document, layer);
        }

        if flags & 0x10 != 0 {
            let rows =
                mesh_rows.ok_or_else(|| DxfError::invalid("POLYLINE Mesh 缺少行数（组码 71）"))?;
            let cols =
                mesh_cols.ok_or_else(|| DxfError::invalid("POLYLINE Mesh 缺少列数（组码 72）"))?;
            let wrap_m = flags & 0x01 != 0;
            let wrap_n = flags & 0x02 != 0;
            return self.parse_polygon_mesh(
                document,
                layer,
                rows as usize,
                cols as usize,
                wrap_m,
                wrap_n,
            );
        }

        self.skip_polyline_sequence()?;
        Err(DxfError::unsupported(
            "POLYLINE Mesh/Polyface 以外的模式暂未实现",
        ))
    }

    fn parse_polyface_mesh(
        &mut self,
        document: &mut Document,
        layer: String,
    ) -> Result<(), DxfError> {
        let mut coordinates: Vec<Point3> = Vec::new();
        loop {
            match self.reader.next_pair()? {
                Some((0, value)) => match value.as_str() {
                    "VERTEX" => match self.parse_polyface_vertex_record()? {
                        PolyfaceRecord::Coordinate(point) => coordinates.push(point),
                        PolyfaceRecord::Face { indices } => {
                            if let Some((vertices, invisible_edges)) =
                                self.build_polyface_face(&coordinates, indices)?
                            {
                                document.add_face3d(vertices, invisible_edges, layer.clone());
                            }
                        }
                        PolyfaceRecord::Ignored => {}
                    },
                    "SEQEND" => break,
                    _ => {
                        self.reader.put_back((0, value));
                        break;
                    }
                },
                Some(_) => {
                    return Err(DxfError::invalid(
                        "POLYLINE 遇到无效的记录，期望 VERTEX/SEQEND",
                    ));
                }
                None => {
                    return Err(DxfError::invalid(
                        "POLYLINE 缺少 SEQEND（组码 0, 值为 SEQEND）",
                    ));
                }
            }
        }
        Ok(())
    }

    fn parse_polygon_mesh(
        &mut self,
        document: &mut Document,
        layer: String,
        rows: usize,
        cols: usize,
        wrap_m: bool,
        wrap_n: bool,
    ) -> Result<(), DxfError> {
        if rows < 2 || cols < 2 {
            return Err(DxfError::invalid(
                "POLYLINE 网格至少需要 2x2 个顶点才能构成面",
            ));
        }

        let mut vertices: Vec<Point3> = Vec::new();
        loop {
            match self.reader.next_pair()? {
                Some((0, value)) => match value.as_str() {
                    "VERTEX" => {
                        if let Some(point) = self.parse_mesh_vertex_record()? {
                            vertices.push(point);
                        }
                    }
                    "SEQEND" => break,
                    _ => {
                        self.reader.put_back((0, value));
                        break;
                    }
                },
                Some(_) => return Err(DxfError::invalid("POLYLINE 网格遇到无效记录")),
                None => {
                    return Err(DxfError::invalid(
                        "POLYLINE 网格缺少 SEQEND（组码 0, 值为 SEQEND）",
                    ));
                }
            }
        }

        let expected = rows * cols;
        if vertices.len() < expected {
            return Err(DxfError::invalid(format!(
                "POLYLINE 网格顶点不足：期望至少 {expected} 个，实际为 {}",
                vertices.len()
            )));
        }

        let row_iterations = if wrap_m { rows } else { rows - 1 };
        let col_iterations = if wrap_n { cols } else { cols - 1 };

        for row in 0..row_iterations {
            let next_row = (row + 1) % rows;
            for col in 0..col_iterations {
                let next_col = (col + 1) % cols;
                let idx = row * cols + col;
                let idx_right = row * cols + next_col;
                let idx_down = next_row * cols + col;
                let idx_diag = next_row * cols + next_col;
                let face_vertices = [
                    vertices[idx],
                    vertices[idx_down],
                    vertices[idx_diag],
                    vertices[idx_right],
                ];
                document.add_face3d(face_vertices, [false; 4], layer.clone());
            }
        }

        Ok(())
    }

    fn parse_polyface_vertex_record(&mut self) -> Result<PolyfaceRecord, DxfError> {
        let mut x = None;
        let mut y = None;
        let mut z = None;
        let mut flags: i16 = 0;
        let mut indices = [0i32; 4];

        loop {
            match self.reader.next_pair()? {
                Some((0, value)) => {
                    self.reader.put_back((0, value));
                    break;
                }
                Some((code, value)) => match code {
                    8 => {}
                    10 => x = Some(parse_f64(&value, "POLYFACE 顶点 X（组码 10）")?),
                    20 => y = Some(parse_f64(&value, "POLYFACE 顶点 Y（组码 20）")?),
                    30 => z = Some(parse_f64(&value, "POLYFACE 顶点 Z（组码 30）")?),
                    70 => flags = parse_i16(&value, "VERTEX 标志（组码 70）")?,
                    71 => indices[0] = parse_i32(&value, "POLYFACE 面顶点 1（组码 71）")?,
                    72 => indices[1] = parse_i32(&value, "POLYFACE 面顶点 2（组码 72）")?,
                    73 => indices[2] = parse_i32(&value, "POLYFACE 面顶点 3（组码 73）")?,
                    74 => indices[3] = parse_i32(&value, "POLYFACE 面顶点 4（组码 74）")?,
                    _ => {}
                },
                None => return Err(DxfError::invalid("VERTEX 未正确结束")),
            }
        }

        if flags & 0x80 != 0 && flags & 0x40 != 0 {
            let point = Point3::new(
                x.ok_or_else(|| DxfError::invalid("POLYFACE 顶点缺少 X（组码 10）"))?,
                y.ok_or_else(|| DxfError::invalid("POLYFACE 顶点缺少 Y（组码 20）"))?,
                z.unwrap_or(0.0),
            );
            Ok(PolyfaceRecord::Coordinate(point))
        } else if flags & 0x80 != 0 {
            Ok(PolyfaceRecord::Face { indices })
        } else {
            Ok(PolyfaceRecord::Ignored)
        }
    }

    fn build_polyface_face(
        &self,
        coordinates: &[Point3],
        indices: [i32; 4],
    ) -> Result<Option<([Point3; 4], [bool; 4])>, DxfError> {
        if coordinates.is_empty() {
            return Ok(None);
        }
        let mut vertices = [coordinates[0]; 4];
        let mut invisible = [false; 4];
        let mut last_vertex = coordinates[0];

        for (slot, &index) in indices.iter().enumerate() {
            if index == 0 {
                vertices[slot] = last_vertex;
                invisible[slot] = true;
                continue;
            }

            let hidden = index < 0;
            let resolved = self.resolve_polyface_vertex(coordinates, index)?;
            vertices[slot] = resolved;
            invisible[slot] = hidden;
            last_vertex = resolved;
        }

        Ok(Some((vertices, invisible)))
    }

    fn resolve_polyface_vertex(
        &self,
        coordinates: &[Point3],
        index: i32,
    ) -> Result<Point3, DxfError> {
        let idx = index.abs() as usize;
        if idx == 0 || idx > coordinates.len() {
            return Err(DxfError::invalid(format!(
                "POLYFACE 面引用了不存在的顶点索引 {index}"
            )));
        }
        Ok(coordinates[idx - 1])
    }

    fn parse_mesh_vertex_record(&mut self) -> Result<Option<Point3>, DxfError> {
        let mut x = None;
        let mut y = None;
        let mut z = None;
        let mut flags: i16 = 0;

        loop {
            match self.reader.next_pair()? {
                Some((0, value)) => {
                    self.reader.put_back((0, value));
                    break;
                }
                Some((code, value)) => match code {
                    8 => {}
                    10 => x = Some(parse_f64(&value, "POLYLINE 网格顶点 X（组码 10）")?),
                    20 => y = Some(parse_f64(&value, "POLYLINE 网格顶点 Y（组码 20）")?),
                    30 => z = Some(parse_f64(&value, "POLYLINE 网格顶点 Z（组码 30）")?),
                    70 => flags = parse_i16(&value, "VERTEX 标志（组码 70）")?,
                    _ => {}
                },
                None => return Err(DxfError::invalid("VERTEX 未正确结束")),
            }
        }

        if flags & 0x80 != 0 {
            return Ok(None);
        }

        let x = x.ok_or_else(|| DxfError::invalid("POLYLINE 网格顶点缺少 X（组码 10）"))?;
        let y = y.ok_or_else(|| DxfError::invalid("POLYLINE 网格顶点缺少 Y（组码 20）"))?;
        Ok(Some(Point3::new(x, y, z.unwrap_or(0.0))))
    }

    fn skip_polyline_sequence(&mut self) -> Result<(), DxfError> {
        loop {
            match self.reader.next_pair()? {
                Some((0, value)) => match value.as_str() {
                    "VERTEX" => self.skip_entity_body()?,
                    "SEQEND" => break,
                    _ => {
                        self.reader.put_back((0, value));
                        break;
                    }
                },
                Some(_) => continue,
                None => break,
            }
        }
        Ok(())
    }

    fn parse_spline(&mut self) -> Result<Entity, DxfError> {
        let mut layer = None;
        let mut flags: i16 = 0;
        let mut degree: Option<i16> = None;
        let mut knot_values: Vec<f64> = Vec::new();
        let mut weights: Vec<f64> = Vec::new();
        let mut control_points: Vec<Point2> = Vec::new();
        let mut fit_points: Vec<Point2> = Vec::new();
        let mut pending_control_x: Option<f64> = None;
        let mut pending_fit_x: Option<f64> = None;
        let mut pending_start_tangent_x: Option<f64> = None;
        let mut pending_end_tangent_x: Option<f64> = None;
        let mut start_tangent: Option<Vector2> = None;
        let mut end_tangent: Option<Vector2> = None;

        loop {
            match self.reader.next_pair()? {
                Some((0, value)) => {
                    self.reader.put_back((0, value));
                    break;
                }
                Some((code, value)) => match code {
                    8 => layer = Some(value.trim().to_string()),
                    70 => {
                        flags = parse_i16(&value, "SPLINE 类型标志（组码 70）")?;
                    }
                    71 => {
                        degree = Some(parse_i16(&value, "SPLINE 阶数（组码 71）")?);
                    }
                    72 | 73 | 74 => {
                        // 节点/控制点/拟合点计数，仅用于校验，当前忽略
                        let _ = parse_i32(&value, "SPLINE 计数信息")?;
                    }
                    40 => {
                        knot_values.push(parse_f64(&value, "SPLINE 节点值（组码 40）")?);
                    }
                    41 => {
                        weights.push(parse_f64(&value, "SPLINE 权重（组码 41）")?);
                    }
                    10 => {
                        if pending_control_x
                            .replace(parse_f64(&value, "SPLINE 控制点 X（组码 10）")?)
                            .is_some()
                        {
                            return Err(DxfError::invalid(
                                "SPLINE 控制点 X（组码 10）在未提供 Y 之前重复出现",
                            ));
                        }
                    }
                    20 => {
                        let y = parse_f64(&value, "SPLINE 控制点 Y（组码 20）")?;
                        let x = pending_control_x.take().ok_or_else(|| {
                            DxfError::invalid("SPLINE 控制点 Y（组码 20）缺少对应的 X")
                        })?;
                        control_points.push(Point2::new(x, y));
                    }
                    11 => {
                        if pending_fit_x
                            .replace(parse_f64(&value, "SPLINE 拟合点 X（组码 11）")?)
                            .is_some()
                        {
                            return Err(DxfError::invalid(
                                "SPLINE 拟合点 X（组码 11）在未提供 Y 之前重复出现",
                            ));
                        }
                    }
                    21 => {
                        let y = parse_f64(&value, "SPLINE 拟合点 Y（组码 21）")?;
                        let x = pending_fit_x.take().ok_or_else(|| {
                            DxfError::invalid("SPLINE 拟合点 Y（组码 21）缺少对应的 X")
                        })?;
                        fit_points.push(Point2::new(x, y));
                    }
                    12 => {
                        if pending_start_tangent_x
                            .replace(parse_f64(&value, "SPLINE 起始切向量 X（组码 12）")?)
                            .is_some()
                        {
                            return Err(DxfError::invalid(
                                "SPLINE 起始切向量 X（组码 12）重复出现",
                            ));
                        }
                    }
                    22 => {
                        let y = parse_f64(&value, "SPLINE 起始切向量 Y（组码 22）")?;
                        let x = pending_start_tangent_x.take().ok_or_else(|| {
                            DxfError::invalid("SPLINE 起始切向量 Y（组码 22）缺少对应的 X")
                        })?;
                        start_tangent = Some(Vector2::new(x, y));
                    }
                    13 => {
                        if pending_end_tangent_x
                            .replace(parse_f64(&value, "SPLINE 终止切向量 X（组码 13）")?)
                            .is_some()
                        {
                            return Err(DxfError::invalid(
                                "SPLINE 终止切向量 X（组码 13）重复出现",
                            ));
                        }
                    }
                    23 => {
                        let y = parse_f64(&value, "SPLINE 终止切向量 Y（组码 23）")?;
                        let x = pending_end_tangent_x.take().ok_or_else(|| {
                            DxfError::invalid("SPLINE 终止切向量 Y（组码 23）缺少对应的 X")
                        })?;
                        end_tangent = Some(Vector2::new(x, y));
                    }
                    30 | 31 | 32 | 33 => {
                        // 忽略 Z 坐标与三维向量分量
                    }
                    210 | 220 | 230 | 42 | 43 | 44 | 45 | 46 | 47 | 48 | 49 | 420 | 421 | 422
                    | 430 => {
                        // 暂无处理：法向量、拟合公差、颜色等信息
                    }
                    _ => {}
                },
                None => return Err(DxfError::invalid("SPLINE 未正确结束")),
            }
        }

        if let Some(x) = pending_control_x.take() {
            return Err(DxfError::invalid(format!(
                "SPLINE 控制点 X={x} 缺少对应的 Y（组码 20）"
            )));
        }
        if let Some(x) = pending_fit_x.take() {
            return Err(DxfError::invalid(format!(
                "SPLINE 拟合点 X={x} 缺少对应的 Y（组码 21）"
            )));
        }
        if let Some(x) = pending_start_tangent_x.take() {
            return Err(DxfError::invalid(format!(
                "SPLINE 起始切向量 X={x} 缺少对应的 Y（组码 22）"
            )));
        }
        if let Some(x) = pending_end_tangent_x.take() {
            return Err(DxfError::invalid(format!(
                "SPLINE 终止切向量 X={x} 缺少对应的 Y（组码 23）"
            )));
        }

        let layer = layer.unwrap_or_else(|| "0".to_string());
        let degree = degree.ok_or_else(|| DxfError::invalid("SPLINE 缺少阶数（组码 71）"))? as i32;
        let is_closed = flags & 0x01 != 0;
        let is_periodic = flags & 0x02 != 0;
        let is_rational = flags & 0x04 != 0;

        Ok(Entity::Spline(Spline {
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
        }))
    }

    fn parse_text(&mut self) -> Result<Entity, DxfError> {
        let mut layer = None;
        let mut insert_x = None;
        let mut insert_y = None;
        let mut height = None;
        let mut rotation_deg = 0.0;
        let mut text: Option<String> = None;
        loop {
            match self.reader.next_pair()? {
                Some((0, value)) => {
                    self.reader.put_back((0, value));
                    break;
                }
                Some((code, value)) => match code {
                    8 => layer = Some(value.trim().to_string()),
                    10 => {
                        if insert_x.is_some() {
                            return Err(DxfError::invalid("TEXT 遇到重复的插入点 X（组码 10）"));
                        }
                        insert_x = Some(parse_f64(&value, "TEXT 插入点 X")?);
                    }
                    20 => {
                        if insert_y.is_some() {
                            return Err(DxfError::invalid("TEXT 遇到重复的插入点 Y（组码 20）"));
                        }
                        insert_y = Some(parse_f64(&value, "TEXT 插入点 Y")?);
                    }
                    30 => {}
                    40 => {
                        if height.is_some() {
                            return Err(DxfError::invalid("TEXT 遇到重复的文字高度（组码 40）"));
                        }
                        height = Some(parse_f64(&value, "TEXT 高度")?);
                    }
                    50 => {
                        rotation_deg = parse_f64(&value, "TEXT 旋转角")?;
                    }
                    1 => {
                        let entry = value;
                        match text {
                            Some(ref mut existing) => {
                                existing.push('\n');
                                existing.push_str(&entry);
                            }
                            None => text = Some(entry),
                        }
                    }
                    7 | 72 | 73 | 100 | 11 | 21 => {
                        // 目前忽略：文字样式、对齐信息等
                    }
                    _ => {}
                },
                None => return Err(DxfError::invalid("TEXT 未正确结束")),
            }
        }

        let layer = layer.unwrap_or_else(|| "0".to_string());
        let ix = insert_x.ok_or_else(|| DxfError::invalid("TEXT 缺少插入点 X（组码 10）"))?;
        let iy = insert_y.ok_or_else(|| DxfError::invalid("TEXT 缺少插入点 Y（组码 20）"))?;
        let height = height.ok_or_else(|| DxfError::invalid("TEXT 缺少文字高度（组码 40）"))?;
        let content = text.ok_or_else(|| DxfError::invalid("TEXT 缺少文本内容（组码 1）"))?;

        Ok(Entity::Text(Text {
            insert: Point2::new(ix, iy),
            content,
            height,
            rotation: rotation_deg.to_radians(),
            layer,
        }))
    }

    fn parse_mtext(&mut self) -> Result<Entity, DxfError> {
        let mut layer = None;
        let mut insert_x = None;
        let mut insert_y = None;
        let mut height = None;
        let mut reference_width: Option<f64> = None;
        let mut direction_x: Option<f64> = None;
        let mut direction_y: Option<f64> = None;
        let mut rotation_deg: Option<f64> = None;
        let mut attachment_point: i16 = 1;
        let mut drawing_direction: i16 = 1;
        let mut style: Option<String> = None;
        let mut fragments: Vec<String> = Vec::new();

        loop {
            match self.reader.next_pair()? {
                Some((0, value)) => {
                    self.reader.put_back((0, value));
                    break;
                }
                Some((code, value)) => match code {
                    8 => layer = Some(value.trim().to_string()),
                    10 => {
                        if insert_x.is_some() {
                            return Err(DxfError::invalid("MTEXT 遇到重复的插入点 X（组码 10）"));
                        }
                        insert_x = Some(parse_f64(&value, "MTEXT 插入点 X")?);
                    }
                    20 => {
                        if insert_y.is_some() {
                            return Err(DxfError::invalid("MTEXT 遇到重复的插入点 Y（组码 20）"));
                        }
                        insert_y = Some(parse_f64(&value, "MTEXT 插入点 Y")?);
                    }
                    30 => {}
                    40 => {
                        if height.is_some() {
                            return Err(DxfError::invalid("MTEXT 遇到重复的文本高度（组码 40）"));
                        }
                        height = Some(parse_f64(&value, "MTEXT 高度")?);
                    }
                    41 => {
                        let width = parse_f64(&value, "MTEXT 参考宽度")?;
                        reference_width = if width.abs() < f64::EPSILON {
                            None
                        } else {
                            Some(width)
                        };
                    }
                    11 => {
                        direction_x = Some(parse_f64(&value, "MTEXT 方向向量 X")?);
                    }
                    21 => {
                        direction_y = Some(parse_f64(&value, "MTEXT 方向向量 Y")?);
                    }
                    31 => {}
                    50 => {
                        rotation_deg = Some(parse_f64(&value, "MTEXT 旋转角")?);
                    }
                    71 => {
                        attachment_point = parse_i16(&value, "MTEXT 附着点 (组码 71)")?;
                    }
                    72 => {
                        drawing_direction = parse_i16(&value, "MTEXT 书写方向 (组码 72)")?;
                    }
                    7 => {
                        style = Some(value.trim().to_string());
                    }
                    1 | 3 => {
                        fragments.push(value);
                    }
                    100 | 101 | 102 | 210 | 220 | 230 | 73 | 44 => {
                        // 这些字段当前暂未映射，保留跳过
                    }
                    _ => {}
                },
                None => return Err(DxfError::invalid("MTEXT 未正确结束")),
            }
        }

        let layer = layer.unwrap_or_else(|| "0".to_string());
        let ix = insert_x.ok_or_else(|| DxfError::invalid("MTEXT 缺少插入点 X（组码 10）"))?;
        let iy = insert_y.ok_or_else(|| DxfError::invalid("MTEXT 缺少插入点 Y（组码 20）"))?;
        let height = height.ok_or_else(|| DxfError::invalid("MTEXT 缺少文本高度（组码 40）"))?;
        if fragments.is_empty() {
            return Err(DxfError::invalid("MTEXT 缺少内容（组码 1/3）"));
        }

        let decoded_text = fragments
            .into_iter()
            .map(|frag| decode_mtext_content(&frag))
            .collect::<String>();

        let direction = match (direction_x, direction_y) {
            (Some(x), Some(y)) => {
                if (x.abs() < f64::EPSILON) && (y.abs() < f64::EPSILON) {
                    Vector2::new(1.0, 0.0)
                } else {
                    Vector2::new(x, y)
                }
            }
            _ => {
                if let Some(rot) = rotation_deg {
                    let rad = rot.to_radians();
                    Vector2::new(rad.cos(), rad.sin())
                } else {
                    Vector2::new(1.0, 0.0)
                }
            }
        };

        Ok(Entity::MText(MText {
            insert: Point2::new(ix, iy),
            content: decoded_text,
            height,
            reference_width,
            direction,
            attachment_point,
            drawing_direction,
            style,
            layer,
        }))
    }

    fn parse_insert(&mut self) -> Result<Entity, DxfError> {
        let mut layer = None;
        let mut name = None;
        let mut insert_x = None;
        let mut insert_y = None;
        let mut scale_x: Option<f64> = None;
        let mut scale_y: Option<f64> = None;
        let mut rotation_deg: f64 = 0.0;

        loop {
            match self.reader.next_pair()? {
                Some((0, value)) => {
                    self.reader.put_back((0, value));
                    break;
                }
                Some((code, value)) => match code {
                    8 => layer = Some(value.trim().to_string()),
                    2 => {
                        if name.is_some() {
                            return Err(DxfError::invalid("INSERT 遇到重复的块名（组码 2）"));
                        }
                        name = Some(value.trim().to_string());
                    }
                    10 => {
                        if insert_x.is_some() {
                            return Err(DxfError::invalid("INSERT 遇到重复的插入点 X（组码 10）"));
                        }
                        insert_x = Some(parse_f64(&value, "INSERT 插入点 X")?);
                    }
                    20 => {
                        if insert_y.is_some() {
                            return Err(DxfError::invalid("INSERT 遇到重复的插入点 Y（组码 20）"));
                        }
                        insert_y = Some(parse_f64(&value, "INSERT 插入点 Y")?);
                    }
                    30 => {}
                    41 => {
                        scale_x = Some(parse_f64(&value, "INSERT 缩放 X")?);
                    }
                    42 => {
                        scale_y = Some(parse_f64(&value, "INSERT 缩放 Y")?);
                    }
                    50 => {
                        rotation_deg = parse_f64(&value, "INSERT 旋转角")?;
                    }
                    66 => {
                        // 指示存在属性，解析流程会自动尝试读取
                    }
                    43 | 210 | 220 | 230 | 70 | 71 | 100 | 102 | 0 => {
                        // 忽略目前未用到的字段
                    }
                    _ => {}
                },
                None => return Err(DxfError::invalid("INSERT 未正确结束")),
            }
        }

        let layer = layer.unwrap_or_else(|| "0".to_string());
        let name = name.ok_or_else(|| DxfError::invalid("INSERT 缺少块名（组码 2）"))?;
        let ix = insert_x.ok_or_else(|| DxfError::invalid("INSERT 缺少插入点 X（组码 10）"))?;
        let iy = insert_y.ok_or_else(|| DxfError::invalid("INSERT 缺少插入点 Y（组码 20）"))?;
        let sx = scale_x.unwrap_or(1.0);
        let sy = scale_y.unwrap_or_else(|| scale_x.unwrap_or(1.0));

        let mut attributes: Vec<Attribute> = Vec::new();
        loop {
            match self.reader.next_pair()? {
                Some((0, value)) => match value.as_str() {
                    "ATTRIB" => {
                        let attr = self.parse_attrib()?;
                        attributes.push(attr);
                    }
                    "SEQEND" => {
                        self.skip_entity_body()?;
                        break;
                    }
                    _ => {
                        self.reader.put_back((0, value));
                        break;
                    }
                },
                Some((code, value)) => {
                    return Err(DxfError::invalid(format!(
                        "INSERT 属性段出现意外组码 {code} 值 {value}"
                    )));
                }
                None => break,
            }
        }

        Ok(Entity::BlockReference(BlockReference {
            name,
            insert: Point2::new(ix, iy),
            scale: Vector2::new(sx, sy),
            rotation: rotation_deg.to_radians(),
            attributes,
            layer,
        }))
    }

    fn parse_hatch(&mut self) -> Result<Entity, DxfError> {
        #[derive(Debug, Clone)]
        struct PolyVertex {
            point: Point2,
            bulge: f64,
        }

        struct PartialLoop {
            _flags: i32,
            is_polyline: bool,
            has_bulge: bool,
            is_closed: bool,
            expected_vertices: Option<usize>,
            poly_vertices: Vec<PolyVertex>,
            boundary_handles: Vec<String>,
            edges: Vec<HatchEdge>,
            pending_vertex_x: Option<f64>,
        }

        impl PartialLoop {
            fn new(flags: i32) -> Self {
                let is_polyline = (flags & 0x02) != 0;
                Self {
                    _flags: flags,
                    is_polyline,
                    has_bulge: false,
                    is_closed: false,
                    expected_vertices: None,
                    poly_vertices: Vec::new(),
                    boundary_handles: Vec::new(),
                    edges: Vec::new(),
                    pending_vertex_x: None,
                }
            }

            fn finalize_edge_builder(
                &mut self,
                builder: Option<EdgeBuilder>,
            ) -> Result<(), DxfError> {
                if let Some(edge_builder) = builder {
                    self.edges.push(edge_builder.finish()?);
                }
                Ok(())
            }

            fn finalize(mut self) -> Result<HatchLoop, DxfError> {
                if self.is_polyline {
                    if let Some(expected) = self.expected_vertices {
                        if expected != self.poly_vertices.len() {
                            return Err(DxfError::invalid(format!(
                                "HATCH 多段线环路声明的顶点数量 {expected} 与实际数量 {} 不符",
                                self.poly_vertices.len()
                            )));
                        }
                    }
                    self.convert_polyline_vertices_to_edges();
                }
                if let Some(pending) = self.pending_vertex_x {
                    return Err(DxfError::invalid(format!(
                        "HATCH 顶点 X={pending} 缺少对应的 Y 坐标"
                    )));
                }
                Ok(HatchLoop {
                    is_polyline: self.is_polyline,
                    is_closed: self.is_closed,
                    edges: self.edges,
                    boundary_handles: self.boundary_handles,
                })
            }

            fn convert_polyline_vertices_to_edges(&mut self) {
                if self.poly_vertices.len() < 2 {
                    return;
                }
                let len = self.poly_vertices.len();
                for i in 0..len - 1 {
                    let current = &self.poly_vertices[i];
                    let next = &self.poly_vertices[i + 1];
                    self.edges.push(HatchEdge::PolylineSegment {
                        start: current.point,
                        end: next.point,
                        bulge: if self.has_bulge { current.bulge } else { 0.0 },
                    });
                }
                if self.is_closed {
                    let first = &self.poly_vertices[0];
                    let last = &self.poly_vertices[len - 1];
                    self.edges.push(HatchEdge::PolylineSegment {
                        start: last.point,
                        end: first.point,
                        bulge: if self.has_bulge { last.bulge } else { 0.0 },
                    });
                }
            }
        }

        enum EdgeBuilder {
            Line {
                start: Option<Point2>,
                end: Option<Point2>,
            },
            Arc {
                center: Option<Point2>,
                radius: Option<f64>,
                start_angle: Option<f64>,
                end_angle: Option<f64>,
                is_counter_clockwise: bool,
            },
            Ellipse {
                center: Option<Point2>,
                major_axis: Option<Vector2>,
                minor_ratio: Option<f64>,
                start_angle: Option<f64>,
                end_angle: Option<f64>,
                is_counter_clockwise: bool,
            },
            Spline(SplineBuilder),
        }

        impl EdgeBuilder {
            fn new(edge_type: i32) -> Result<Self, DxfError> {
                match edge_type {
                    1 => Ok(Self::Line {
                        start: None,
                        end: None,
                    }),
                    2 => Ok(Self::Arc {
                        center: None,
                        radius: None,
                        start_angle: None,
                        end_angle: None,
                        is_counter_clockwise: true,
                    }),
                    3 => Ok(Self::Ellipse {
                        center: None,
                        major_axis: None,
                        minor_ratio: None,
                        start_angle: None,
                        end_angle: None,
                        is_counter_clockwise: true,
                    }),
                    4 => Ok(Self::Spline(SplineBuilder::default())),
                    other => Err(DxfError::unsupported(format!(
                        "HATCH 不支持的边界类型 {other}"
                    ))),
                }
            }

            fn finish(self) -> Result<HatchEdge, DxfError> {
                match self {
                    EdgeBuilder::Line { start, end } => {
                        let start =
                            start.ok_or_else(|| DxfError::invalid("HATCH 直线边缺少起点"))?;
                        let end = end.ok_or_else(|| DxfError::invalid("HATCH 直线边缺少终点"))?;
                        Ok(HatchEdge::Line { start, end })
                    }
                    EdgeBuilder::Arc {
                        center,
                        radius,
                        start_angle,
                        end_angle,
                        is_counter_clockwise,
                    } => {
                        let center =
                            center.ok_or_else(|| DxfError::invalid("HATCH 圆弧边缺少圆心"))?;
                        let radius =
                            radius.ok_or_else(|| DxfError::invalid("HATCH 圆弧边缺少半径"))?;
                        let start_angle = start_angle
                            .ok_or_else(|| DxfError::invalid("HATCH 圆弧边缺少起始角"))?;
                        let end_angle =
                            end_angle.ok_or_else(|| DxfError::invalid("HATCH 圆弧边缺少终止角"))?;
                        Ok(HatchEdge::Arc {
                            center,
                            radius,
                            start_angle,
                            end_angle,
                            is_counter_clockwise,
                        })
                    }
                    EdgeBuilder::Ellipse {
                        center,
                        major_axis,
                        minor_ratio,
                        start_angle,
                        end_angle,
                        is_counter_clockwise,
                    } => {
                        let center =
                            center.ok_or_else(|| DxfError::invalid("HATCH 椭圆边缺少圆心"))?;
                        let major_axis = major_axis
                            .ok_or_else(|| DxfError::invalid("HATCH 椭圆边缺少主轴向量"))?;
                        let minor_ratio =
                            minor_ratio.ok_or_else(|| DxfError::invalid("HATCH 椭圆边缺少轴比"))?;
                        let start_angle = start_angle
                            .ok_or_else(|| DxfError::invalid("HATCH 椭圆边缺少起始角"))?;
                        let end_angle =
                            end_angle.ok_or_else(|| DxfError::invalid("HATCH 椭圆边缺少终止角"))?;
                        Ok(HatchEdge::Ellipse {
                            center,
                            major_axis,
                            minor_ratio,
                            start_angle,
                            end_angle,
                            is_counter_clockwise,
                        })
                    }
                    EdgeBuilder::Spline(builder) => builder.finish(),
                }
            }
        }

        #[derive(Default)]
        struct GradientBuilder {
            enabled: bool,
            angle: Option<f64>,
            shift: Option<f64>,
            tint: Option<f64>,
            is_single_color: bool,
            colors: Vec<u32>,
            name: Option<String>,
        }

        #[derive(Default)]
        struct SplineBuilder {
            control_points: Vec<Point2>,
            fit_points: Vec<Point2>,
            knot_values: Vec<f64>,
            degree: Option<i32>,
            is_rational: bool,
            is_periodic: bool,
            pending_control_x: Option<f64>,
            pending_fit_x: Option<f64>,
        }

        impl SplineBuilder {
            fn push_control_x(&mut self, value: f64) -> Result<(), DxfError> {
                if self.pending_control_x.replace(value).is_some() {
                    Err(DxfError::invalid(
                        "HATCH 样条边遇到重复的控制点 X（组码 10）",
                    ))
                } else {
                    Ok(())
                }
            }

            fn push_control_y(&mut self, value: f64) -> Result<(), DxfError> {
                let x = self
                    .pending_control_x
                    .take()
                    .ok_or_else(|| DxfError::invalid("HATCH 样条边缺少控制点 X（组码 10）"))?;
                self.control_points.push(Point2::new(x, value));
                Ok(())
            }

            fn push_fit_x(&mut self, value: f64) -> Result<(), DxfError> {
                if self.pending_fit_x.replace(value).is_some() {
                    Err(DxfError::invalid(
                        "HATCH 样条边遇到重复的拟合点 X（组码 11）",
                    ))
                } else {
                    Ok(())
                }
            }

            fn push_fit_y(&mut self, value: f64) -> Result<(), DxfError> {
                let x = self
                    .pending_fit_x
                    .take()
                    .ok_or_else(|| DxfError::invalid("HATCH 样条边缺少拟合点 X（组码 11）"))?;
                self.fit_points.push(Point2::new(x, value));
                Ok(())
            }

            fn finish(mut self) -> Result<HatchEdge, DxfError> {
                if let Some(x) = self.pending_control_x.take() {
                    return Err(DxfError::invalid(format!(
                        "HATCH 样条边控制点 X={x} 缺少对应的 Y 坐标"
                    )));
                }
                if let Some(x) = self.pending_fit_x.take() {
                    return Err(DxfError::invalid(format!(
                        "HATCH 样条边拟合点 X={x} 缺少对应的 Y 坐标"
                    )));
                }
                if self.control_points.len() < 2 {
                    return Err(DxfError::invalid("HATCH 样条边至少需要两个控制点"));
                }
                Ok(HatchEdge::Spline {
                    control_points: self.control_points,
                    fit_points: self.fit_points,
                    knot_values: self.knot_values,
                    degree: self.degree.unwrap_or(3),
                    is_rational: self.is_rational,
                    is_periodic: self.is_periodic,
                })
            }
        }

        impl GradientBuilder {
            fn finish(self) -> Option<HatchGradient> {
                if !self.enabled {
                    return None;
                }
                Some(HatchGradient {
                    name: self.name.unwrap_or_else(|| "LINEAR".to_string()),
                    angle: self.angle.unwrap_or(0.0),
                    shift: self.shift,
                    tint: self.tint,
                    is_single_color: self.is_single_color,
                    color1: self.colors.get(0).copied(),
                    color2: self.colors.get(1).copied(),
                })
            }
        }

        let mut layer = None;
        let mut pattern_name = "SOLID".to_string();
        let mut is_solid = false;
        let mut loops: Vec<HatchLoop> = Vec::new();
        let mut current_loop: Option<PartialLoop> = None;
        let mut edge_builder: Option<EdgeBuilder> = None;
        let mut gradient_builder = GradientBuilder::default();

        fn finalize_loop(
            current_loop: &mut Option<PartialLoop>,
            loops: &mut Vec<HatchLoop>,
            edge_builder: &mut Option<EdgeBuilder>,
        ) -> Result<(), DxfError> {
            if let Some(mut loop_data) = current_loop.take() {
                loop_data.finalize_edge_builder(edge_builder.take())?;
                loops.push(loop_data.finalize()?);
            }
            Ok(())
        }

        loop {
            match self.reader.next_pair()? {
                Some((0, value)) => {
                    finalize_loop(&mut current_loop, &mut loops, &mut edge_builder)?;
                    self.reader.put_back((0, value));
                    break;
                }
                Some((code, value)) => match code {
                    8 => layer = Some(value.trim().to_string()),
                    2 => pattern_name = value.trim().to_string(),
                    70 => {
                        let flag = parse_i16(&value, "HATCH 旗标（组码 70）")?;
                        is_solid = (flag & 1) != 0;
                    }
                    91 => {
                        // 环路数量，仅作校验参考
                        let _ = parse_i32(&value, "HATCH 环路数量（组码 91）")?;
                    }
                    92 => {
                        finalize_loop(&mut current_loop, &mut loops, &mut edge_builder)?;
                        let flags = parse_i32(&value, "HATCH 环路类型（组码 92）")?;
                        current_loop = Some(PartialLoop::new(flags));
                    }
                    93 => {
                        if let Some(loop_data) = current_loop.as_mut() {
                            loop_data.expected_vertices =
                                Some(parse_i32(&value, "HATCH 边计数（组码 93）")? as usize);
                        }
                    }
                    72 => {
                        if let Some(loop_data) = current_loop.as_mut() {
                            if loop_data.is_polyline && edge_builder.is_none() {
                                loop_data.has_bulge =
                                    parse_i32(&value, "HATCH 多段线 bulge 标记（组码 72）")? != 0;
                            } else {
                                loop_data.finalize_edge_builder(edge_builder.take())?;
                                edge_builder = Some(EdgeBuilder::new(parse_i32(
                                    &value,
                                    "HATCH 边类型（组码 72）",
                                )?)?);
                            }
                        } else {
                            return Err(DxfError::invalid(
                                "HATCH 在缺少环路的情况下出现了边定义（组码 72）",
                            ));
                        }
                    }
                    73 => {
                        if let Some(loop_data) = current_loop.as_mut() {
                            if loop_data.is_polyline && edge_builder.is_none() {
                                loop_data.is_closed =
                                    parse_i32(&value, "HATCH 多段线闭合标记（组码 73）")? != 0;
                            } else if let Some(builder) = edge_builder.as_mut() {
                                match builder {
                                    EdgeBuilder::Arc {
                                        is_counter_clockwise,
                                        ..
                                    }
                                    | EdgeBuilder::Ellipse {
                                        is_counter_clockwise,
                                        ..
                                    } => {
                                        *is_counter_clockwise =
                                            parse_i32(&value, "HATCH 边方向标记（组码 73）")? != 0;
                                    }
                                    EdgeBuilder::Spline(spline) => {
                                        spline.is_rational =
                                            parse_i32(&value, "HATCH 样条有理标记（组码 73）")?
                                                != 0;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    74 => {
                        if let Some(EdgeBuilder::Spline(spline)) = edge_builder.as_mut() {
                            spline.is_periodic =
                                parse_i32(&value, "HATCH 样条周期标记（组码 74）")? != 0;
                        }
                    }
                    75 => {
                        if let Some(EdgeBuilder::Spline(spline)) = edge_builder.as_mut() {
                            spline.degree = Some(parse_i32(&value, "HATCH 样条阶数（组码 75）")?);
                        }
                    }
                    97 => {
                        // 引用对象数量，保留检查但实际以 330 条目为准
                        let _ = parse_i32(&value, "HATCH 边界引用数量（组码 97）")?;
                    }
                    330 => {
                        if let Some(loop_data) = current_loop.as_mut() {
                            loop_data.boundary_handles.push(value.trim().to_string());
                        }
                    }
                    10 => {
                        if let Some(loop_data) = current_loop.as_mut() {
                            if loop_data.is_polyline && edge_builder.is_none() {
                                if loop_data.pending_vertex_x.is_some() {
                                    return Err(DxfError::invalid(
                                        "HATCH 多段线遇到重复的顶点 X（组码 10）",
                                    ));
                                }
                                loop_data.pending_vertex_x =
                                    Some(parse_f64(&value, "HATCH 顶点 X（组码 10）")?);
                            } else if let Some(builder) = edge_builder.as_mut() {
                                match builder {
                                    EdgeBuilder::Line { start, .. } => {
                                        let x = parse_f64(&value, "HATCH 直线起点 X（组码 10）")?;
                                        start.get_or_insert(Point2::new(0.0, 0.0)).0.x = x;
                                    }
                                    EdgeBuilder::Arc { center, .. } => {
                                        let x = parse_f64(&value, "HATCH 圆弧圆心 X（组码 10）")?;
                                        center.get_or_insert(Point2::new(0.0, 0.0)).0.x = x;
                                    }
                                    EdgeBuilder::Ellipse { center, .. } => {
                                        let x = parse_f64(&value, "HATCH 椭圆圆心 X（组码 10）")?;
                                        center.get_or_insert(Point2::new(0.0, 0.0)).0.x = x;
                                    }
                                    EdgeBuilder::Spline(spline) => {
                                        spline.push_control_x(parse_f64(
                                            &value,
                                            "HATCH 样条控制点 X（组码 10）",
                                        )?)?;
                                    }
                                }
                            }
                        }
                    }
                    20 => {
                        if let Some(loop_data) = current_loop.as_mut() {
                            if loop_data.is_polyline && edge_builder.is_none() {
                                let y = parse_f64(&value, "HATCH 顶点 Y（组码 20）")?;
                                let x = loop_data.pending_vertex_x.take().ok_or_else(|| {
                                    DxfError::invalid(
                                        "HATCH 顶点 Y 前未读取到对应的 X 值（组码 20）",
                                    )
                                })?;
                                loop_data.poly_vertices.push(PolyVertex {
                                    point: Point2::new(x, y),
                                    bulge: 0.0,
                                });
                            } else if let Some(builder) = edge_builder.as_mut() {
                                match builder {
                                    EdgeBuilder::Line { start, .. } => {
                                        let y = parse_f64(&value, "HATCH 直线起点 Y（组码 20）")?;
                                        start.get_or_insert(Point2::new(0.0, 0.0)).0.y = y;
                                    }
                                    EdgeBuilder::Arc { center, .. } => {
                                        let y = parse_f64(&value, "HATCH 圆弧圆心 Y（组码 20）")?;
                                        center.get_or_insert(Point2::new(0.0, 0.0)).0.y = y;
                                    }
                                    EdgeBuilder::Ellipse { center, .. } => {
                                        let y = parse_f64(&value, "HATCH 椭圆圆心 Y（组码 20）")?;
                                        center.get_or_insert(Point2::new(0.0, 0.0)).0.y = y;
                                    }
                                    EdgeBuilder::Spline(spline) => {
                                        spline.push_control_y(parse_f64(
                                            &value,
                                            "HATCH 样条控制点 Y（组码 20）",
                                        )?)?;
                                    }
                                }
                            }
                        }
                    }
                    11 => {
                        if let Some(builder) = edge_builder.as_mut() {
                            match builder {
                                EdgeBuilder::Line { end, .. } => {
                                    let x = parse_f64(&value, "HATCH 直线终点 X（组码 11）")?;
                                    end.get_or_insert(Point2::new(0.0, 0.0)).0.x = x;
                                }
                                EdgeBuilder::Ellipse { major_axis, .. } => {
                                    let x = parse_f64(&value, "HATCH 椭圆主轴向量 X（组码 11）")?;
                                    major_axis.get_or_insert(Vector2::new(0.0, 0.0)).0.x = x;
                                }
                                EdgeBuilder::Spline(spline) => {
                                    spline.push_fit_x(parse_f64(
                                        &value,
                                        "HATCH 样条拟合点 X（组码 11）",
                                    )?)?;
                                }
                                _ => {}
                            }
                        }
                    }
                    21 => {
                        if let Some(builder) = edge_builder.as_mut() {
                            match builder {
                                EdgeBuilder::Line { end, .. } => {
                                    let y = parse_f64(&value, "HATCH 直线终点 Y（组码 21）")?;
                                    end.get_or_insert(Point2::new(0.0, 0.0)).0.y = y;
                                }
                                EdgeBuilder::Ellipse { major_axis, .. } => {
                                    let y = parse_f64(&value, "HATCH 椭圆主轴向量 Y（组码 21）")?;
                                    major_axis.get_or_insert(Vector2::new(0.0, 0.0)).0.y = y;
                                }
                                EdgeBuilder::Spline(spline) => {
                                    spline.push_fit_y(parse_f64(
                                        &value,
                                        "HATCH 样条拟合点 Y（组码 21）",
                                    )?)?;
                                }
                                _ => {}
                            }
                        }
                    }
                    40 => {
                        if let Some(builder) = edge_builder.as_mut() {
                            match builder {
                                EdgeBuilder::Arc { radius, .. } => {
                                    *radius = Some(parse_f64(&value, "HATCH 圆弧半径（组码 40）")?);
                                }
                                EdgeBuilder::Ellipse { minor_ratio, .. } => {
                                    *minor_ratio =
                                        Some(parse_f64(&value, "HATCH 椭圆轴比（组码 40）")?);
                                }
                                EdgeBuilder::Spline(spline) => {
                                    spline
                                        .knot_values
                                        .push(parse_f64(&value, "HATCH 样条数据（组码 40）")?);
                                }
                                _ => {}
                            }
                        }
                    }
                    41 => {
                        if let Some(EdgeBuilder::Spline(spline)) = edge_builder.as_mut() {
                            spline
                                .knot_values
                                .push(parse_f64(&value, "HATCH 样条数据（组码 41）")?);
                        }
                        // 其它情况下忽略（阴影模糊等参数）
                    }
                    42 => {
                        if let Some(loop_data) = current_loop.as_mut() {
                            if loop_data.is_polyline && !loop_data.poly_vertices.is_empty() {
                                let bulge = parse_f64(&value, "HATCH 多段线 bulge（组码 42）")?;
                                if let Some(last) = loop_data.poly_vertices.last_mut() {
                                    last.bulge = bulge;
                                }
                            }
                        }
                        if let Some(EdgeBuilder::Spline(spline)) = edge_builder.as_mut() {
                            spline
                                .knot_values
                                .push(parse_f64(&value, "HATCH 样条节点（组码 42）")?);
                        }
                    }
                    47 => {
                        // 渐变相关缩放，目前忽略
                    }
                    50 => {
                        if let Some(builder) = edge_builder.as_mut() {
                            match builder {
                                EdgeBuilder::Arc { start_angle, .. } => {
                                    *start_angle =
                                        Some(parse_f64(&value, "HATCH 圆弧起始角（组码 50）")?);
                                }
                                EdgeBuilder::Ellipse { start_angle, .. } => {
                                    *start_angle =
                                        Some(parse_f64(&value, "HATCH 椭圆起始角（组码 50）")?);
                                }
                                _ => {}
                            }
                        } else {
                            gradient_builder.angle =
                                Some(parse_f64(&value, "HATCH 渐变角度（组码 50）")?);
                        }
                    }
                    51 => {
                        if let Some(builder) = edge_builder.as_mut() {
                            match builder {
                                EdgeBuilder::Arc { end_angle, .. } => {
                                    *end_angle =
                                        Some(parse_f64(&value, "HATCH 圆弧终止角（组码 51）")?);
                                }
                                EdgeBuilder::Ellipse { end_angle, .. } => {
                                    *end_angle =
                                        Some(parse_f64(&value, "HATCH 椭圆终止角（组码 51）")?);
                                }
                                _ => {}
                            }
                        }
                    }
                    63 => {
                        // ACI 颜色索引，转换为调色板颜色编码
                        let index = parse_i32(&value, "HATCH 渐变颜色索引（组码 63）")?;
                        gradient_builder.colors.push(index as u32);
                    }
                    420 | 421 => {
                        let color_value = parse_u32(&value, "HATCH 渐变颜色（组码 420/421）")?;
                        gradient_builder.colors.push(color_value);
                    }
                    450 => {
                        gradient_builder.enabled =
                            parse_i32(&value, "HATCH 渐变开关（组码 450）")? != 0;
                    }
                    451 => {
                        // 颜色计数（用于校验）
                        let _ = parse_i32(&value, "HATCH 渐变颜色数量（组码 451）")?;
                    }
                    452 => {
                        gradient_builder.angle =
                            Some(parse_f64(&value, "HATCH 渐变角度（组码 452）")?);
                    }
                    453 => {
                        gradient_builder.is_single_color =
                            parse_i32(&value, "HATCH 渐变类型（组码 453）")? != 0;
                    }
                    460 => {
                        gradient_builder.shift =
                            Some(parse_f64(&value, "HATCH 渐变偏移（组码 460）")?);
                    }
                    461 | 462 => {
                        gradient_builder.tint =
                            Some(parse_f64(&value, "HATCH 渐变颜色混合（组码 461/462）")?);
                    }
                    470 => {
                        gradient_builder.name = Some(value.trim().to_string());
                    }
                    _ => {
                        // 其它未支持字段直接忽略
                    }
                },
                None => return Err(DxfError::invalid("HATCH 未正确结束")),
            }
        }

        if loops.is_empty() {
            return Err(DxfError::invalid("HATCH 缺少边界定义"));
        }

        let layer = layer.unwrap_or_else(|| "0".to_string());
        Ok(Entity::Hatch(Hatch {
            pattern_name,
            is_solid,
            loops,
            gradient: gradient_builder.finish(),
            layer,
        }))
    }

    fn parse_dimension(&mut self) -> Result<Entity, DxfError> {
        let mut layer = None;
        let mut flags: i16 = 0;
        let mut definition_x = None;
        let mut definition_y = None;
        let mut text_mid_x = None;
        let mut text_mid_y = None;
        let mut dim_line_x = None;
        let mut dim_line_y = None;
        let mut ext_origin_x = None;
        let mut ext_origin_y = None;
        let mut ext_end_x = None;
        let mut ext_end_y = None;
        let mut secondary_x = None;
        let mut secondary_y = None;
        let mut arc_def_x = None;
        let mut arc_def_y = None;
        let mut center_x = None;
        let mut center_y = None;
        let mut text_override: Option<String> = None;
        let mut measurement: Option<f64> = None;
        let mut rotation_deg: f64 = 0.0;
        let mut text_rotation_deg: Option<f64> = None;
        let mut oblique_angle_deg: Option<f64> = None;

        loop {
            match self.reader.next_pair()? {
                Some((0, value)) => {
                    self.reader.put_back((0, value));
                    break;
                }
                Some((code, value)) => match code {
                    8 => layer = Some(value.trim().to_string()),
                    70 => {
                        flags = parse_i16(&value, "DIMENSION 类型标志（组码 70）")?;
                    }
                    1 => {
                        let entry = value.trim().to_string();
                        if entry == "<>" || entry.is_empty() {
                            text_override = None;
                        } else {
                            text_override = Some(entry);
                        }
                    }
                    10 => {
                        definition_x = Some(parse_f64(&value, "DIMENSION 定义点 X（组码 10）")?);
                    }
                    20 => {
                        definition_y = Some(parse_f64(&value, "DIMENSION 定义点 Y（组码 20）")?);
                    }
                    11 => {
                        text_mid_x = Some(parse_f64(&value, "DIMENSION 文本位置 X（组码 11）")?);
                    }
                    21 => {
                        text_mid_y = Some(parse_f64(&value, "DIMENSION 文本位置 Y（组码 21）")?);
                    }
                    12 => {
                        secondary_x = Some(parse_f64(&value, "DIMENSION 次要点 X（组码 12）")?);
                    }
                    22 => {
                        secondary_y = Some(parse_f64(&value, "DIMENSION 次要点 Y（组码 22）")?);
                    }
                    13 => {
                        dim_line_x = Some(parse_f64(&value, "DIMENSION 尺寸线位置 X（组码 13）")?);
                    }
                    23 => {
                        dim_line_y = Some(parse_f64(&value, "DIMENSION 尺寸线位置 Y（组码 23）")?);
                    }
                    14 => {
                        ext_origin_x = Some(parse_f64(&value, "DIMENSION 引线起点 X（组码 14）")?);
                    }
                    24 => {
                        ext_origin_y = Some(parse_f64(&value, "DIMENSION 引线起点 Y（组码 24）")?);
                    }
                    15 => {
                        ext_end_x = Some(parse_f64(&value, "DIMENSION 引线终点 X（组码 15）")?);
                    }
                    25 => {
                        ext_end_y = Some(parse_f64(&value, "DIMENSION 引线终点 Y（组码 25）")?);
                    }
                    16 => {
                        arc_def_x = Some(parse_f64(&value, "DIMENSION 弧定义点 X（组码 16）")?);
                    }
                    26 => {
                        arc_def_y = Some(parse_f64(&value, "DIMENSION 弧定义点 Y（组码 26）")?);
                    }
                    17 => {
                        center_x = Some(parse_f64(&value, "DIMENSION 圆心 X（组码 17）")?);
                    }
                    27 => {
                        center_y = Some(parse_f64(&value, "DIMENSION 圆心 Y（组码 27）")?);
                    }
                    42 => {
                        measurement = Some(parse_f64(&value, "DIMENSION 测量值（组码 42）")?);
                    }
                    50 => {
                        rotation_deg = parse_f64(&value, "DIMENSION 旋转角（组码 50）")?;
                    }
                    51 => {
                        oblique_angle_deg = Some(parse_f64(&value, "DIMENSION 倾斜角（组码 51）")?);
                    }
                    52 => {
                        text_rotation_deg =
                            Some(parse_f64(&value, "DIMENSION 文本旋转（组码 52）")?);
                    }
                    210 | 220 | 230 | 100 | 101 | 102 | 71 | 72 => {
                        // 暂不使用的字段
                    }
                    _ => {}
                },
                None => return Err(DxfError::invalid("DIMENSION 未正确结束")),
            }
        }

        let layer = layer.unwrap_or_else(|| "0".to_string());
        let dx =
            definition_x.ok_or_else(|| DxfError::invalid("DIMENSION 缺少定义点 X（组码 10）"))?;
        let dy =
            definition_y.ok_or_else(|| DxfError::invalid("DIMENSION 缺少定义点 Y（组码 20）"))?;
        let tx =
            text_mid_x.ok_or_else(|| DxfError::invalid("DIMENSION 缺少文本位置 X（组码 11）"))?;
        let ty =
            text_mid_y.ok_or_else(|| DxfError::invalid("DIMENSION 缺少文本位置 Y（组码 21）"))?;

        let dimension_line_point = match (dim_line_x, dim_line_y) {
            (Some(x), Some(y)) => Some(Point2::new(x, y)),
            _ => None,
        };

        let extension_line_origin = match (ext_origin_x, ext_origin_y) {
            (Some(x), Some(y)) => Some(Point2::new(x, y)),
            _ => None,
        };

        let extension_line_end = match (ext_end_x, ext_end_y) {
            (Some(x), Some(y)) => Some(Point2::new(x, y)),
            _ => None,
        };

        let secondary_point = match (secondary_x, secondary_y) {
            (Some(x), Some(y)) => Some(Point2::new(x, y)),
            _ => None,
        };

        let arc_definition_point = match (arc_def_x, arc_def_y) {
            (Some(x), Some(y)) => Some(Point2::new(x, y)),
            _ => None,
        };

        let center_point = match (center_x, center_y) {
            (Some(x), Some(y)) => Some(Point2::new(x, y)),
            _ => None,
        };

        let kind = Self::dimension_kind_from_flags(flags);

        Ok(Entity::Dimension(Dimension {
            kind,
            definition_point: Point2::new(dx, dy),
            text_midpoint: Point2::new(tx, ty),
            dimension_line_point,
            extension_line_origin,
            extension_line_end,
            secondary_point,
            arc_definition_point,
            center_point,
            text: text_override,
            measurement,
            rotation: rotation_deg.to_radians(),
            text_rotation: text_rotation_deg.map(f64::to_radians),
            oblique_angle: oblique_angle_deg.map(f64::to_radians),
            layer,
        }))
    }

    fn parse_leader(&mut self) -> Result<Entity, DxfError> {
        let mut layer = None;
        let mut style_name: Option<String> = None;
        let mut has_arrowhead = false;
        let mut pending_x: Option<f64> = None;
        let mut vertices: Vec<Point2> = Vec::new();

        loop {
            match self.reader.next_pair()? {
                Some((0, value)) => {
                    self.reader.put_back((0, value));
                    break;
                }
                Some((code, value)) => match code {
                    8 => layer = Some(value.trim().to_string()),
                    3 => {
                        let trimmed = value.trim();
                        if !trimmed.is_empty() {
                            style_name = Some(trimmed.to_string());
                        }
                    }
                    10 => {
                        if pending_x.is_some() {
                            return Err(DxfError::invalid(
                                "LEADER 顶点 X（组码 10）重复出现且缺少对应的组码 20",
                            ));
                        }
                        pending_x = Some(parse_f64(&value, "LEADER 顶点 X（组码 10）")?);
                    }
                    20 => {
                        let x = pending_x.take().ok_or_else(|| {
                            DxfError::invalid("LEADER 顶点 Y（组码 20）出现前缺少组码 10")
                        })?;
                        let y = parse_f64(&value, "LEADER 顶点 Y（组码 20）")?;
                        vertices.push(Point2::new(x, y));
                    }
                    71 => {
                        has_arrowhead =
                            parse_i16(&value, "LEADER 箭头标志（组码 71）")? & 0x01 != 0;
                    }
                    30 | 40 | 41 | 42 | 43 | 44 | 45 | 46 | 47 | 72 | 73 | 74 | 75 | 76 | 77
                    | 210 | 220 | 230 | 211 | 221 | 231 | 212 | 222 | 232 | 213 | 223 | 233
                    | 60 | 290 | 291 | 292 | 293 | 294 | 295 | 296 | 297 | 340 | 341 | 342 => {
                        // 当前实现未使用的参数，均忽略
                    }
                    _ => {}
                },
                None => return Err(DxfError::invalid("LEADER 未正确结束")),
            }
        }

        if pending_x.is_some() {
            return Err(DxfError::invalid(
                "LEADER 读取完毕时缺少最后一个顶点的组码 20",
            ));
        }

        if vertices.is_empty() {
            return Err(DxfError::invalid("LEADER 缺少任意顶点（组码 10/20）"));
        }

        let layer = layer.unwrap_or_else(|| "0".to_string());
        Ok(Entity::Leader(Leader {
            layer,
            style_name,
            vertices,
            has_arrowhead,
        }))
    }

    fn parse_mleader(&mut self) -> Result<Entity, DxfError> {
        let mut layer = None;
        let mut style_name: Option<String> = None;
        let mut text_height: Option<f64> = None;
        let mut scale: Option<f64> = None;
        let mut landing_gap: Option<f64> = None;
        let mut leader_lines: Vec<LeaderLine> = Vec::new();
        let mut current_line: Vec<Point2> = Vec::new();
        let mut pending_vertex_x: Option<f64> = None;
        let mut content_location_pending_x: Option<f64> = None;
        let mut content_location: Option<Point2> = None;
        let mut text_lines: Vec<String> = Vec::new();
        let mut content_type: Option<i16> = None;
        let mut block_handle: Option<String> = None;
        let mut block_scale = [1.0_f64, 1.0, 1.0];
        let mut block_scale_next_code: Option<i32> = None;
        let mut block_rotation: Option<f64> = None;
        let mut block_connection_type: Option<i16> = None;
        let mut block_location_x: Option<f64> = None;
        let mut block_location_y: Option<f64> = None;
        let mut in_leader_section = false;
        let mut dogleg_length: Option<f64> = None;

        loop {
            match self.reader.next_pair()? {
                Some((0, value)) => {
                    self.reader.put_back((0, value));
                    break;
                }
                Some((code, value)) => match code {
                    8 => layer = Some(value.trim().to_string()),
                    3 => {
                        let trimmed = value.trim();
                        if !trimmed.is_empty() {
                            style_name = Some(trimmed.to_string());
                        }
                    }
                    40 => {
                        if in_leader_section {
                            dogleg_length =
                                Some(parse_f64(&value, "MULTILEADER 狗腿长度（组码 40）")?);
                        } else {
                            scale = Some(parse_f64(&value, "MULTILEADER 缩放（组码 40）")?);
                        }
                    }
                    140 => {
                        text_height = Some(parse_f64(&value, "MULTILEADER 文本高度（组码 140）")?);
                    }
                    145 => {
                        landing_gap = Some(parse_f64(&value, "MULTILEADER 落脚间隙（组码 145）")?);
                    }
                    10 => {
                        if block_scale_next_code == Some(10) {
                            block_scale[0] = parse_f64(&value, "MULTILEADER 块缩放 X（组码 10）")?;
                            block_scale_next_code = Some(20);
                            continue;
                        }
                        if pending_vertex_x.is_some() {
                            return Err(DxfError::invalid(
                                "MULTILEADER 顶点 X（组码 10）重复出现且缺少对应的组码 20",
                            ));
                        }
                        pending_vertex_x =
                            Some(parse_f64(&value, "MULTILEADER 引线顶点 X（组码 10）")?);
                    }
                    20 => {
                        if block_scale_next_code == Some(20) {
                            block_scale[1] = parse_f64(&value, "MULTILEADER 块缩放 Y（组码 20）")?;
                            block_scale_next_code = Some(30);
                            continue;
                        }
                        let x = pending_vertex_x.take().ok_or_else(|| {
                            DxfError::invalid("MULTILEADER 引线顶点 Y（组码 20）出现前缺少组码 10")
                        })?;
                        let y = parse_f64(&value, "MULTILEADER 引线顶点 Y（组码 20）")?;
                        current_line.push(Point2::new(x, y));
                    }
                    30 => {
                        if block_scale_next_code == Some(30) {
                            block_scale[2] = parse_f64(&value, "MULTILEADER 块缩放 Z（组码 30）")?;
                            block_scale_next_code = None;
                        }
                    }
                    12 => {
                        content_location_pending_x =
                            Some(parse_f64(&value, "MULTILEADER 内容位置 X（组码 12）")?);
                    }
                    22 => {
                        let x = content_location_pending_x.take().ok_or_else(|| {
                            DxfError::invalid("MULTILEADER 内容位置 Y（组码 22）出现前缺少组码 12")
                        })?;
                        let y = parse_f64(&value, "MULTILEADER 内容位置 Y（组码 22）")?;
                        content_location = Some(Point2::new(x, y));
                    }
                    91 => {
                        if !current_line.is_empty() {
                            let vertices = std::mem::take(&mut current_line);
                            leader_lines.push(LeaderLine { vertices });
                        }
                    }
                    302 | 303 | 304 => {
                        let trimmed = value.trim_end_matches('\r').to_string();
                        match trimmed.as_str() {
                            "LEADER{" => {
                                in_leader_section = true;
                            }
                            "LEADER_LINE{" => {}
                            "}" => {}
                            other => {
                                if !other.is_empty() {
                                    text_lines.push(other.to_string());
                                }
                            }
                        }
                    }
                    305 | 306 | 307 => {
                        let trimmed = value.trim_end_matches('\r');
                        if trimmed == "}" {
                            in_leader_section = false;
                        }
                    }
                    15 => {
                        block_location_x =
                            Some(parse_f64(&value, "MULTILEADER 块内容位置 X（组码 15）")?);
                    }
                    25 => {
                        block_location_y =
                            Some(parse_f64(&value, "MULTILEADER 块内容位置 Y（组码 25）")?);
                    }
                    41 | 42 | 44 | 45 | 46 | 47 | 48 | 49 | 90 | 92 | 93 | 94 | 95 | 96 | 97
                    | 210 | 220 | 230 | 211 | 221 | 231 | 212 | 222 | 232 | 213 | 223 | 233
                    | 260 | 270 | 271 | 272 | 300 | 301 | 340 | 341 | 342 | 345 | 346 | 347
                    | 348 | 349 | 350 | 351 => {
                        // 当前实现暂未处理的字段
                    }
                    172 => {
                        content_type = Some(parse_i16(&value, "MULTILEADER 内容类型（组码 172）")?);
                    }
                    343 => {
                        // 保留 text style handle，当前实现未解析
                    }
                    344 => {
                        let trimmed = value.trim();
                        if !trimmed.is_empty() {
                            block_handle = Some(trimmed.to_string());
                            block_scale_next_code = Some(10);
                        }
                    }
                    43 => {
                        block_rotation = Some(parse_f64(&value, "MULTILEADER 块旋转（组码 43）")?);
                    }
                    176 => {
                        block_connection_type =
                            Some(parse_i16(&value, "MULTILEADER 块连接类型（组码 176）")?);
                    }
                    _ => {}
                },
                None => return Err(DxfError::invalid("MULTILEADER 未正确结束")),
            }
        }

        if pending_vertex_x.is_some() {
            return Err(DxfError::invalid(
                "MULTILEADER 读取完毕时缺少最后一个引线顶点的组码 20",
            ));
        }
        if content_location_pending_x.is_some() {
            return Err(DxfError::invalid(
                "MULTILEADER 内容位置缺少组码 22 对应的 Y 坐标",
            ));
        }

        if !current_line.is_empty() {
            leader_lines.push(LeaderLine {
                vertices: current_line,
            });
        }

        if leader_lines.is_empty() {
            return Err(DxfError::invalid(
                "MULTILEADER 缺少任何引线几何（组码 10/20）",
            ));
        }

        let layer = layer.unwrap_or_else(|| "0".to_string());

        let fallback_location = content_location
            .as_ref()
            .copied()
            .or_else(|| {
                leader_lines
                    .first()
                    .and_then(|line| line.vertices.last().copied())
            })
            .unwrap_or_else(|| Point2::new(0.0, 0.0));
        let has_block_content = matches!(content_type, Some(1))
            || block_handle.is_some()
            || block_location_x.is_some()
            || block_location_y.is_some();
        let has_dogleg = dogleg_length
            .map(|value| value.abs() > f64::EPSILON)
            .unwrap_or(false);

        let content = if has_block_content {
            let location = match (block_location_x, block_location_y) {
                (Some(x), Some(y)) => Point2::new(x, y),
                _ => fallback_location,
            };
            MLeaderContent::Block {
                block: MLeaderBlockContent {
                    block_handle,
                    block_name: None,
                    location,
                    scale: Vector2::new(block_scale[0], block_scale[1]),
                    rotation: block_rotation.unwrap_or(0.0),
                    connection_type: block_connection_type,
                },
            }
        } else if !text_lines.is_empty() {
            let joined = text_lines.join("\n");
            let decoded = decode_inline_text(&joined);
            let location = content_location.unwrap_or(fallback_location);
            MLeaderContent::MText {
                text: decoded,
                location,
            }
        } else {
            MLeaderContent::None
        };

        Ok(Entity::MLeader(MLeader {
            layer,
            style_name,
            leader_lines,
            content,
            text_height,
            scale,
            has_dogleg,
            dogleg_length,
            landing_gap,
        }))
    }

    fn parse_image(&mut self) -> Result<Entity, DxfError> {
        let mut layer = None;
        let mut image_def_handle: Option<String> = None;
        let mut insert_x: Option<f64> = None;
        let mut insert_y: Option<f64> = None;
        let mut u_vec_x: Option<f64> = None;
        let mut u_vec_y: Option<f64> = None;
        let mut v_vec_x: Option<f64> = None;
        let mut v_vec_y: Option<f64> = None;
        let mut width: f64 = 0.0;
        let mut height: f64 = 0.0;
        let mut options = RasterImageDisplayOptions::default();
        let mut clip_vertices: Vec<Point2> = Vec::new();
        let mut pending_clip_x: Option<f64> = None;
        let mut clip_enabled = false;
        let mut clip_boundary_type: Option<i16> = None;
        let mut expected_clip_vertices: Option<i32> = None;
        let mut image_def_reactor_handle: Option<String> = None;
        let mut clip_mode_value = ClipMode::Outside;

        loop {
            match self.reader.next_pair()? {
                Some((0, value)) => {
                    self.reader.put_back((0, value));
                    break;
                }
                Some((code, value)) => match code {
                    8 => layer = Some(value.trim().to_string()),
                    10 => insert_x = Some(parse_f64(&value, "IMAGE 插入点 X（组码 10）")?),
                    20 => insert_y = Some(parse_f64(&value, "IMAGE 插入点 Y（组码 20）")?),
                    11 => u_vec_x = Some(parse_f64(&value, "IMAGE U 向量 X（组码 11）")?),
                    21 => u_vec_y = Some(parse_f64(&value, "IMAGE U 向量 Y（组码 21）")?),
                    12 => v_vec_x = Some(parse_f64(&value, "IMAGE V 向量 X（组码 12）")?),
                    22 => v_vec_y = Some(parse_f64(&value, "IMAGE V 向量 Y（组码 22）")?),
                    13 => width = parse_f64(&value, "IMAGE 显示宽度（组码 13）")?,
                    23 => height = parse_f64(&value, "IMAGE 显示高度（组码 23）")?,
                    70 => {
                        let flags = parse_i16(&value, "IMAGE 显示标志（组码 70）")?;
                        options.show_image = flags & 1 != 0;
                        options.show_border = flags & 2 != 0;
                        options.use_clipping = flags & 4 != 0;
                    }
                    280 => options.brightness = Some(parse_i16(&value, "IMAGE 亮度（组码 280）")?),
                    281 => options.contrast = Some(parse_i16(&value, "IMAGE 对比度（组码 281）")?),
                    282 => options.fade = Some(parse_i16(&value, "IMAGE 渐隐（组码 282）")?),
                    340 => {
                        image_def_handle = Some(value.trim().to_string());
                    }
                    71 => {
                        clip_enabled = parse_i16(&value, "IMAGE 裁剪开关（组码 71）")? != 0;
                    }
                    72 => {
                        clip_boundary_type = Some(parse_i16(&value, "IMAGE 裁剪类型（组码 72）")?);
                    }
                    290 => {
                        clip_mode_value = if parse_i16(&value, "IMAGE 裁剪方向（组码 290）")? != 0
                        {
                            ClipMode::Inside
                        } else {
                            ClipMode::Outside
                        };
                    }
                    76 => {
                        // R2010+ 使用 76 表示裁剪开关
                        clip_enabled = parse_i16(&value, "IMAGE 裁剪开关（组码 76）")? != 0;
                    }
                    90 => {
                        clip_boundary_type = Some(parse_i16(&value, "IMAGE 裁剪类型（组码 90）")?);
                    }
                    91 => {
                        expected_clip_vertices =
                            Some(parse_i16(&value, "IMAGE 裁剪顶点数量（组码 91）")? as i32);
                    }
                    14 => {
                        pending_clip_x = Some(parse_f64(&value, "IMAGE 裁剪点 X（组码 14）")?);
                    }
                    24 => {
                        let y = parse_f64(&value, "IMAGE 裁剪点 Y（组码 24）")?;
                        let x = pending_clip_x.take().unwrap_or(0.0);
                        clip_vertices.push(Point2::new(x, y));
                    }
                    30
                    | 31
                    | 32
                    | 40
                    | 41
                    | 50
                    | 73
                    | 92
                    | 93
                    | 94
                    | 95
                    | 96
                    | 97
                    | 98
                    | 99
                    | 2800..=2999
                    | 420
                    | 421
                    | 422
                    | 423
                    | 424
                    | 425
                    | 426 => {
                        // 当前实现未解析的字段，忽略
                    }
                    360 => {
                        let trimmed = value.trim();
                        if !trimmed.is_empty() {
                            image_def_reactor_handle = Some(trimmed.to_string());
                        }
                    }
                    _ => {}
                },
                None => return Err(DxfError::invalid("IMAGE 未正确结束")),
            }
        }

        let layer = layer.unwrap_or_else(|| "0".to_string());
        let insert = Point2::new(insert_x.unwrap_or(0.0), insert_y.unwrap_or(0.0));
        let u_vector = Vector2::new(u_vec_x.unwrap_or(1.0), u_vec_y.unwrap_or(0.0));
        let v_vector = Vector2::new(v_vec_x.unwrap_or(0.0), v_vec_y.unwrap_or(1.0));
        let image_def_handle = image_def_handle
            .ok_or_else(|| DxfError::invalid("IMAGE 缺少引用的 IMAGEDEF 句柄（组码 340）"))?;

        let clip = Self::build_raster_clip(
            clip_enabled,
            clip_boundary_type,
            expected_clip_vertices,
            clip_vertices,
            clip_mode_value,
        );

        Ok(Entity::RasterImage(RasterImage {
            layer,
            image_def_handle,
            insert,
            u_vector,
            v_vector,
            image_size: Vector2::new(width, height),
            display_options: options,
            image_def_reactor_handle,
            clip,
        }))
    }

    fn parse_wipeout(&mut self) -> Result<Entity, DxfError> {
        let mut layer = None;
        let mut insert_x: Option<f64> = None;
        let mut insert_y: Option<f64> = None;
        let mut u_vec_x: Option<f64> = None;
        let mut u_vec_y: Option<f64> = None;
        let mut v_vec_x: Option<f64> = None;
        let mut v_vec_y: Option<f64> = None;
        let mut width: f64 = 0.0;
        let mut height: f64 = 0.0;
        let mut options = RasterImageDisplayOptions::default();
        let mut clip_vertices: Vec<Point2> = Vec::new();
        let mut pending_clip_x: Option<f64> = None;
        let mut clip_enabled = false;
        let mut clip_boundary_type: Option<i16> = None;
        let mut expected_clip_vertices: Option<i32> = None;
        let mut clip_mode_value = ClipMode::Outside;
        let mut ignore_handle: Option<String> = None;

        loop {
            match self.reader.next_pair()? {
                Some((0, value)) => {
                    self.reader.put_back((0, value));
                    break;
                }
                Some((code, value)) => match code {
                    8 => layer = Some(value.trim().to_string()),
                    10 => insert_x = Some(parse_f64(&value, "WIPEOUT 插入点 X（组码 10）")?),
                    20 => insert_y = Some(parse_f64(&value, "WIPEOUT 插入点 Y（组码 20）")?),
                    11 => u_vec_x = Some(parse_f64(&value, "WIPEOUT U 向量 X（组码 11）")?),
                    21 => u_vec_y = Some(parse_f64(&value, "WIPEOUT U 向量 Y（组码 21）")?),
                    12 => v_vec_x = Some(parse_f64(&value, "WIPEOUT V 向量 X（组码 12）")?),
                    22 => v_vec_y = Some(parse_f64(&value, "WIPEOUT V 向量 Y（组码 22）")?),
                    13 => width = parse_f64(&value, "WIPEOUT 显示宽度（组码 13）")?,
                    23 => height = parse_f64(&value, "WIPEOUT 显示高度（组码 23）")?,
                    70 => {
                        let flags = parse_i16(&value, "WIPEOUT 显示标志（组码 70）")?;
                        options.show_image = flags & 1 != 0;
                        options.show_border = flags & 2 != 0;
                        options.use_clipping = flags & 4 != 0;
                    }
                    280 => {
                        options.brightness = Some(parse_i16(&value, "WIPEOUT 亮度（组码 280）")?)
                    }
                    281 => {
                        options.contrast = Some(parse_i16(&value, "WIPEOUT 对比度（组码 281）")?)
                    }
                    282 => options.fade = Some(parse_i16(&value, "WIPEOUT 渐隐（组码 282）")?),
                    340 => {
                        let trimmed = value.trim();
                        if !trimmed.is_empty() {
                            ignore_handle = Some(trimmed.to_string());
                        }
                    }
                    71 => {
                        clip_enabled = parse_i16(&value, "WIPEOUT 裁剪开关（组码 71）")? != 0;
                    }
                    72 => {
                        clip_boundary_type =
                            Some(parse_i16(&value, "WIPEOUT 裁剪类型（组码 72）")?);
                    }
                    290 => {
                        clip_mode_value = if parse_i16(&value, "WIPEOUT 裁剪方向（组码 290）")? != 0
                        {
                            ClipMode::Inside
                        } else {
                            ClipMode::Outside
                        };
                    }
                    76 => {
                        clip_enabled = parse_i16(&value, "WIPEOUT 裁剪开关（组码 76）")? != 0;
                    }
                    90 => {
                        clip_boundary_type =
                            Some(parse_i16(&value, "WIPEOUT 裁剪类型（组码 90）")?);
                    }
                    91 => {
                        expected_clip_vertices =
                            Some(parse_i16(&value, "WIPEOUT 裁剪顶点数量（组码 91）")? as i32);
                    }
                    14 => {
                        pending_clip_x = Some(parse_f64(&value, "WIPEOUT 裁剪点 X（组码 14）")?);
                    }
                    24 => {
                        let y = parse_f64(&value, "WIPEOUT 裁剪点 Y（组码 24）")?;
                        let x = pending_clip_x.take().unwrap_or(0.0);
                        clip_vertices.push(Point2::new(x, y));
                    }
                    360 => {
                        let trimmed = value.trim();
                        if !trimmed.is_empty() {
                            ignore_handle = Some(trimmed.to_string());
                        }
                    }
                    30
                    | 31
                    | 32
                    | 40
                    | 41
                    | 50
                    | 73
                    | 92
                    | 93
                    | 94
                    | 95
                    | 96
                    | 97
                    | 98
                    | 99
                    | 2800..=2999
                    | 420
                    | 421
                    | 422
                    | 423
                    | 424
                    | 425
                    | 426 => {
                        // 目前不使用的字段，忽略
                    }
                    _ => {}
                },
                None => return Err(DxfError::invalid("WIPEOUT 未正确结束")),
            }
        }

        let layer = layer.unwrap_or_else(|| "0".to_string());
        let insert = Point2::new(insert_x.unwrap_or(0.0), insert_y.unwrap_or(0.0));
        let u_vector = Vector2::new(u_vec_x.unwrap_or(1.0), u_vec_y.unwrap_or(0.0));
        let v_vector = Vector2::new(v_vec_x.unwrap_or(0.0), v_vec_y.unwrap_or(1.0));
        let clip = Self::build_raster_clip(
            clip_enabled,
            clip_boundary_type,
            expected_clip_vertices,
            clip_vertices,
            clip_mode_value,
        );

        Ok(Entity::Wipeout(Wipeout {
            layer,
            insert,
            u_vector,
            v_vector,
            image_size: Vector2::new(width, height),
            display_options: options,
            clip,
        }))
    }

    fn build_raster_clip(
        clip_enabled: bool,
        clip_boundary_type: Option<i16>,
        expected_clip_vertices: Option<i32>,
        clip_vertices: Vec<Point2>,
        clip_mode_value: ClipMode,
    ) -> Option<RasterImageClip> {
        if !clip_enabled {
            return None;
        }
        let polygon_expected = expected_clip_vertices
            .and_then(|count| {
                if count > 0 {
                    Some(count as usize)
                } else {
                    None
                }
            })
            .unwrap_or(0);
        let boundary_type = clip_boundary_type.unwrap_or_else(|| {
            if polygon_expected >= 3 || clip_vertices.len() >= 3 {
                2
            } else {
                1
            }
        });
        let treat_as_polygon =
            boundary_type == 2 || polygon_expected >= 3 || clip_vertices.len() >= 3;
        if treat_as_polygon {
            if clip_vertices.len() >= 3 {
                Some(RasterImageClip::Polygon {
                    vertices: clip_vertices,
                    mode: clip_mode_value,
                })
            } else {
                None
            }
        } else if clip_vertices.len() >= 2 {
            let mut min_x = f64::INFINITY;
            let mut min_y = f64::INFINITY;
            let mut max_x = f64::NEG_INFINITY;
            let mut max_y = f64::NEG_INFINITY;
            for vertex in &clip_vertices {
                min_x = min_x.min(vertex.x());
                min_y = min_y.min(vertex.y());
                max_x = max_x.max(vertex.x());
                max_y = max_y.max(vertex.y());
            }
            Some(RasterImageClip::Rectangle {
                min: Point2::new(min_x, min_y),
                max: Point2::new(max_x, max_y),
                mode: clip_mode_value,
            })
        } else {
            None
        }
    }

    fn parse_3dface(&mut self) -> Result<Entity, DxfError> {
        let mut layer = None;
        let mut vx = [None; 4];
        let mut vy = [None; 4];
        let mut vz = [None; 4];
        let mut invisible_edges: Option<i16> = None;

        loop {
            match self.reader.next_pair()? {
                Some((0, value)) => {
                    self.reader.put_back((0, value));
                    break;
                }
                Some((code, value)) => match code {
                    8 => layer = Some(value.trim().to_string()),
                    10 => assign_coord(&mut vx[0], &value, "3DFACE 顶点 1 X（组码 10）")?,
                    20 => assign_coord(&mut vy[0], &value, "3DFACE 顶点 1 Y（组码 20）")?,
                    30 => assign_coord(&mut vz[0], &value, "3DFACE 顶点 1 Z（组码 30）")?,
                    11 => assign_coord(&mut vx[1], &value, "3DFACE 顶点 2 X（组码 11）")?,
                    21 => assign_coord(&mut vy[1], &value, "3DFACE 顶点 2 Y（组码 21）")?,
                    31 => assign_coord(&mut vz[1], &value, "3DFACE 顶点 2 Z（组码 31）")?,
                    12 => assign_coord(&mut vx[2], &value, "3DFACE 顶点 3 X（组码 12）")?,
                    22 => assign_coord(&mut vy[2], &value, "3DFACE 顶点 3 Y（组码 22）")?,
                    32 => assign_coord(&mut vz[2], &value, "3DFACE 顶点 3 Z（组码 32）")?,
                    13 => assign_coord(&mut vx[3], &value, "3DFACE 顶点 4 X（组码 13）")?,
                    23 => assign_coord(&mut vy[3], &value, "3DFACE 顶点 4 Y（组码 23）")?,
                    33 => assign_coord(&mut vz[3], &value, "3DFACE 顶点 4 Z（组码 33）")?,
                    70 => {
                        if invisible_edges.is_some() {
                            return Err(DxfError::invalid(
                                "3DFACE 遇到重复的隐藏边标记（组码 70）",
                            ));
                        }
                        invisible_edges = Some(parse_i16(&value, "3DFACE 隐藏边标记（组码 70）")?);
                    }
                    39 | 71 | 72 | 73 | 74 | 210 | 220 | 230 => {
                        // 厚度、可见性别名或挤出方向暂未使用
                    }
                    _ => {}
                },
                None => return Err(DxfError::invalid("3DFACE 未正确结束")),
            }
        }

        let layer = layer.unwrap_or_else(|| "0".to_string());

        let v1 = build_face_vertex(1, vx[0], vy[0], vz[0])?
            .ok_or_else(|| DxfError::invalid("3DFACE 缺少第 1 个顶点"))?;
        let v2 = build_face_vertex(2, vx[1], vy[1], vz[1])?
            .ok_or_else(|| DxfError::invalid("3DFACE 缺少第 2 个顶点"))?;
        let v3 = build_face_vertex(3, vx[2], vy[2], vz[2])?
            .ok_or_else(|| DxfError::invalid("3DFACE 缺少第 3 个顶点"))?;
        let v4 = match build_face_vertex(4, vx[3], vy[3], vz[3])? {
            Some(vertex) => vertex,
            None => v3,
        };

        let flags = invisible_edges.unwrap_or(0);
        let invisible = [
            flags & 1 != 0,
            flags & 2 != 0,
            flags & 4 != 0,
            flags & 8 != 0,
        ];

        Ok(Entity::Face3D(ThreeDFace {
            layer,
            vertices: [v1, v2, v3, v4],
            invisible_edges: invisible,
        }))
    }

    fn parse_image_def(&mut self) -> Result<RasterImageDefinition, DxfError> {
        let mut handle: Option<String> = None;
        let mut file_path: Option<String> = None;
        let mut name: Option<String> = None;
        let mut size_x: Option<f64> = None;
        let mut size_y: Option<f64> = None;
        let mut pixel_width: Option<f64> = None;
        let mut pixel_height: Option<f64> = None;

        loop {
            match self.reader.next_pair()? {
                Some((0, value)) => {
                    self.reader.put_back((0, value));
                    break;
                }
                Some((code, value)) => match code {
                    5 => handle = Some(value.trim().to_string()),
                    1 => file_path = Some(value.trim().to_string()),
                    2 => {
                        let trimmed = value.trim();
                        if !trimmed.is_empty() {
                            name = Some(trimmed.to_string());
                        }
                    }
                    10 => size_x = Some(parse_f64(&value, "IMAGEDEF 像素宽度（组码 10）")?),
                    20 => size_y = Some(parse_f64(&value, "IMAGEDEF 像素高度（组码 20）")?),
                    11 => pixel_width = Some(parse_f64(&value, "IMAGEDEF 单像素宽度（组码 11）")?),
                    21 => pixel_height = Some(parse_f64(&value, "IMAGEDEF 单像素高度（组码 21）")?),
                    280 | 281 | 282 | 330 | 340 | 341 | 70 | 71 => {
                        // 暂未使用的字段
                    }
                    _ => {}
                },
                None => return Err(DxfError::invalid("IMAGEDEF 未正确结束")),
            }
        }

        let handle = handle.ok_or_else(|| DxfError::invalid("IMAGEDEF 缺少句柄（组码 5）"))?;
        let file_path =
            file_path.ok_or_else(|| DxfError::invalid("IMAGEDEF 缺少文件路径（组码 1）"))?;

        let image_size_pixels = match (size_x, size_y) {
            (Some(x), Some(y)) => Some(Vector2::new(x, y)),
            _ => None,
        };
        let pixel_size = match (pixel_width, pixel_height) {
            (Some(x), Some(y)) => Some(Vector2::new(x, y)),
            _ => None,
        };

        Ok(RasterImageDefinition {
            handle,
            name,
            file_path,
            image_size_pixels,
            pixel_size,
            resolved_path: None,
        })
    }

    fn parse_dictionary(&mut self) -> Result<ParsedDictionary, DxfError> {
        let mut handle: Option<String> = None;
        let mut owner: Option<String> = None;
        let mut entries: Vec<DictionaryEntry> = Vec::new();
        let mut pending_entry_name: Option<String> = None;
        let mut in_reactors = false;

        loop {
            match self.reader.next_pair()? {
                Some((0, value)) => {
                    self.reader.put_back((0, value));
                    break;
                }
                Some((code, value)) => match code {
                    5 => handle = Some(value.trim().to_string()),
                    3 => {
                        let trimmed = value.trim();
                        if !trimmed.is_empty() {
                            pending_entry_name = Some(trimmed.to_string());
                        }
                    }
                    330 => {
                        if in_reactors {
                            continue;
                        }
                        if pending_entry_name.is_none() {
                            owner = Some(value.trim().to_string());
                        }
                    }
                    350 | 360 => {
                        if let Some(name) = pending_entry_name.take() {
                            let trimmed = value.trim();
                            if !trimmed.is_empty() {
                                entries.push(DictionaryEntry {
                                    name,
                                    handle: trimmed.to_string(),
                                });
                            }
                        }
                    }
                    102 => {
                        let trimmed = value.trim();
                        if trimmed.starts_with('{') {
                            in_reactors = true;
                        } else if trimmed == "}" {
                            in_reactors = false;
                        }
                    }
                    _ => {}
                },
                None => return Err(DxfError::invalid("DICTIONARY 未正确结束")),
            }
        }

        let handle = handle.ok_or_else(|| DxfError::invalid("DICTIONARY 缺少句柄（组码 5）"))?;
        Ok(ParsedDictionary {
            handle,
            owner,
            entries,
        })
    }

    fn parse_raster_variables(&mut self) -> Result<(String, RasterImageVariables), DxfError> {
        let mut handle: Option<String> = None;
        let mut class_version: Option<i32> = None;
        let mut frame: Option<i16> = None;
        let mut quality: Option<i16> = None;
        let mut units: Option<i16> = None;
        let mut in_reactors = false;

        loop {
            match self.reader.next_pair()? {
                Some((0, value)) => {
                    self.reader.put_back((0, value));
                    break;
                }
                Some((code, value)) => match code {
                    5 => handle = Some(value.trim().to_string()),
                    90 => {
                        class_version =
                            Some(parse_i32(&value, "RASTERVARIABLES 类版本（组码 90）")?);
                    }
                    70 => frame = Some(parse_i16(&value, "RASTERVARIABLES 图像边框（组码 70）")?),
                    71 => quality = Some(parse_i16(&value, "RASTERVARIABLES 图像质量（组码 71）")?),
                    72 => units = Some(parse_i16(&value, "RASTERVARIABLES 单位（组码 72）")?),
                    102 => {
                        let trimmed = value.trim();
                        if trimmed.starts_with('{') {
                            in_reactors = true;
                        } else if trimmed == "}" {
                            in_reactors = false;
                        }
                    }
                    330 if !in_reactors => {
                        // 所属字典句柄，暂不使用
                    }
                    _ => {}
                },
                None => return Err(DxfError::invalid("RASTERVARIABLES 未正确结束")),
            }
        }

        let handle =
            handle.ok_or_else(|| DxfError::invalid("RASTERVARIABLES 缺少句柄（组码 5）"))?;
        let mut vars = RasterImageVariables::default();
        vars.handle = Some(handle.clone());
        vars.class_version = class_version;
        vars.frame = frame;
        vars.quality = quality;
        vars.units = units;

        Ok((handle, vars))
    }

    fn parse_image_def_reactor(&mut self) -> Result<ImageDefReactor, DxfError> {
        let mut handle: Option<String> = None;
        let mut owner_handle: Option<String> = None;
        let mut image_handle: Option<String> = None;
        let mut class_version: i32 = 0;
        let mut in_reactors = false;
        let mut in_reactor_subclass = false;

        loop {
            match self.reader.next_pair()? {
                Some((0, value)) => {
                    self.reader.put_back((0, value));
                    break;
                }
                Some((code, value)) => match code {
                    5 => handle = Some(value.trim().to_string()),
                    90 => {
                        class_version = parse_i32(&value, "IMAGEDEF_REACTOR 类版本（组码 90）")?;
                    }
                    100 => {
                        let trimmed = value.trim();
                        in_reactor_subclass = trimmed == "AcDbRasterImageDefReactor";
                    }
                    330 => {
                        if in_reactors {
                            continue;
                        }
                        let trimmed = value.trim();
                        if trimmed.is_empty() {
                            continue;
                        }
                        if in_reactor_subclass {
                            image_handle = Some(trimmed.to_string());
                        } else if owner_handle.is_none() {
                            owner_handle = Some(trimmed.to_string());
                        }
                    }
                    102 => {
                        let trimmed = value.trim();
                        if trimmed.starts_with('{') {
                            in_reactors = true;
                        } else if trimmed == "}" {
                            in_reactors = false;
                        }
                    }
                    _ => {}
                },
                None => return Err(DxfError::invalid("IMAGEDEF_REACTOR 未正确结束")),
            }
        }

        let handle =
            handle.ok_or_else(|| DxfError::invalid("IMAGEDEF_REACTOR 缺少句柄（组码 5）"))?;
        Ok(ImageDefReactor {
            handle,
            class_version,
            owner_handle,
            image_handle,
        })
    }

    fn dimension_kind_from_flags(flags: i16) -> DimensionKind {
        let code = flags & 0x0F;
        match code {
            0 => DimensionKind::Linear,
            1 => DimensionKind::Aligned,
            2 => DimensionKind::Angular,
            3 => DimensionKind::Diameter,
            4 => DimensionKind::Radius,
            5 => DimensionKind::Angular3Point,
            6 => DimensionKind::Ordinate,
            other => DimensionKind::Unknown(other),
        }
    }

    fn parse_attrib(&mut self) -> Result<Attribute, DxfError> {
        let mut layer = None;
        let mut insert_x = None;
        let mut insert_y = None;
        let mut height: Option<f64> = None;
        let mut rotation_deg: f64 = 0.0;
        let mut width_factor: f64 = 1.0;
        let mut oblique_deg: f64 = 0.0;
        let mut text: Option<String> = None;
        let mut tag: Option<String> = None;
        let mut style: Option<String> = None;
        let mut prompt: Option<String> = None;
        let mut align_x: Option<f64> = None;
        let mut align_y: Option<f64> = None;
        let mut horizontal_align: i16 = 0;
        let mut vertical_align: i16 = 0;
        let mut flags: i16 = 0;
        let mut lock_position = false;
        let mut line_spacing_factor: f64 = 1.0;
        let mut line_spacing_style: i16 = 0;

        loop {
            match self.reader.next_pair()? {
                Some((0, value)) => {
                    self.reader.put_back((0, value));
                    break;
                }
                Some((code, value)) => match code {
                    8 => layer = Some(value.trim().to_string()),
                    10 => {
                        if insert_x.is_some() {
                            return Err(DxfError::invalid("ATTRIB 遇到重复的插入点 X（组码 10）"));
                        }
                        insert_x = Some(parse_f64(&value, "ATTRIB 插入点 X")?);
                    }
                    20 => {
                        if insert_y.is_some() {
                            return Err(DxfError::invalid("ATTRIB 遇到重复的插入点 Y（组码 20）"));
                        }
                        insert_y = Some(parse_f64(&value, "ATTRIB 插入点 Y")?);
                    }
                    30 => {}
                    40 => height = Some(parse_f64(&value, "ATTRIB 高度")?),
                    41 => width_factor = parse_f64(&value, "ATTRIB 宽度因子")?,
                    50 => {
                        rotation_deg = parse_f64(&value, "ATTRIB 旋转角")?;
                    }
                    51 => {
                        oblique_deg = parse_f64(&value, "ATTRIB 倾斜角")?;
                    }
                    1 => {
                        let entry = value;
                        match text {
                            Some(ref mut existing) => {
                                existing.push('\n');
                                existing.push_str(&entry);
                            }
                            None => text = Some(entry),
                        }
                    }
                    2 => tag = Some(value.trim().to_string()),
                    3 => prompt = Some(value),
                    7 => style = Some(value.trim().to_string()),
                    11 => align_x = Some(parse_f64(&value, "ATTRIB 对齐点 X")?),
                    21 => align_y = Some(parse_f64(&value, "ATTRIB 对齐点 Y")?),
                    70 => flags = parse_i16(&value, "ATTRIB 标志")?,
                    72 => horizontal_align = parse_i16(&value, "ATTRIB 水平对齐")?,
                    73 => vertical_align = parse_i16(&value, "ATTRIB 垂直对齐")?,
                    74 => line_spacing_style = parse_i16(&value, "ATTRIB 行距样式")?,
                    44 => line_spacing_factor = parse_f64(&value, "ATTRIB 行距因子")?,
                    280 => lock_position = parse_i16(&value, "ATTRIB 锁定标志")? != 0,
                    100 | 101 | 102 | 210 | 220 | 230 | 360 => {
                        // 暂时忽略的属性参数
                    }
                    _ => {}
                },
                None => return Err(DxfError::invalid("ATTRIB 未正确结束")),
            }
        }

        let layer = layer.unwrap_or_else(|| "0".to_string());
        let ix = insert_x.ok_or_else(|| DxfError::invalid("ATTRIB 缺少插入点 X（组码 10）"))?;
        let iy = insert_y.ok_or_else(|| DxfError::invalid("ATTRIB 缺少插入点 Y（组码 20）"))?;
        let text = text.ok_or_else(|| DxfError::invalid("ATTRIB 缺少文本内容（组码 1）"))?;
        let tag = tag.ok_or_else(|| DxfError::invalid("ATTRIB 缺少标记（组码 2）"))?;

        let oblique = oblique_deg.to_radians();
        let alignment = match (align_x, align_y) {
            (Some(x), Some(y)) => Some(Point2::new(x, y)),
            _ => None,
        };
        let decoded_text = decode_inline_text(&text);
        Ok(Attribute {
            tag,
            text: decoded_text,
            insert: Point2::new(ix, iy),
            height: height.unwrap_or(0.0),
            rotation: rotation_deg.to_radians(),
            width_factor,
            oblique,
            style,
            prompt,
            alignment,
            horizontal_align,
            vertical_align,
            line_spacing_factor,
            line_spacing_style,
            is_invisible: flags & 0x01 != 0,
            is_constant: flags & 0x02 != 0,
            is_verify: flags & 0x04 != 0,
            is_preset: flags & 0x08 != 0,
            lock_position,
            layer,
        })
    }

    fn parse_attdef(&mut self) -> Result<AttributeDefinition, DxfError> {
        let mut layer = None;
        let mut insert_x = None;
        let mut insert_y = None;
        let mut height: Option<f64> = None;
        let mut rotation_deg: f64 = 0.0;
        let mut width_factor: f64 = 1.0;
        let mut oblique_deg: f64 = 0.0;
        let mut default_text: Option<String> = None;
        let mut tag: Option<String> = None;
        let mut prompt: Option<String> = None;
        let mut style: Option<String> = None;
        let mut align_x: Option<f64> = None;
        let mut align_y: Option<f64> = None;
        let mut horizontal_align: i16 = 0;
        let mut vertical_align: i16 = 0;
        let mut flags: i16 = 0;
        let mut lock_position = false;
        let mut line_spacing_factor: f64 = 1.0;
        let mut line_spacing_style: i16 = 0;

        loop {
            match self.reader.next_pair()? {
                Some((0, value)) => {
                    self.reader.put_back((0, value));
                    break;
                }
                Some((code, value)) => match code {
                    8 => layer = Some(value.trim().to_string()),
                    10 => {
                        if insert_x.is_some() {
                            return Err(DxfError::invalid("ATTDEF 遇到重复的插入点 X（组码 10）"));
                        }
                        insert_x = Some(parse_f64(&value, "ATTDEF 插入点 X")?);
                    }
                    20 => {
                        if insert_y.is_some() {
                            return Err(DxfError::invalid("ATTDEF 遇到重复的插入点 Y（组码 20）"));
                        }
                        insert_y = Some(parse_f64(&value, "ATTDEF 插入点 Y")?);
                    }
                    30 => {}
                    40 => {
                        if height.is_some() {
                            return Err(DxfError::invalid("ATTDEF 遇到重复的文本高度（组码 40）"));
                        }
                        height = Some(parse_f64(&value, "ATTDEF 高度")?);
                    }
                    41 => width_factor = parse_f64(&value, "ATTDEF 宽度因子")?,
                    50 => rotation_deg = parse_f64(&value, "ATTDEF 旋转角")?,
                    51 => oblique_deg = parse_f64(&value, "ATTDEF 倾斜角")?,
                    1 => {
                        let entry = value;
                        match default_text {
                            Some(ref mut existing) => {
                                existing.push('\n');
                                existing.push_str(&entry);
                            }
                            None => default_text = Some(entry),
                        }
                    }
                    2 => tag = Some(value.trim().to_string()),
                    3 => prompt = Some(value),
                    7 => style = Some(value.trim().to_string()),
                    11 => align_x = Some(parse_f64(&value, "ATTDEF 对齐点 X")?),
                    21 => align_y = Some(parse_f64(&value, "ATTDEF 对齐点 Y")?),
                    70 => flags = parse_i16(&value, "ATTDEF 标志")?,
                    72 => horizontal_align = parse_i16(&value, "ATTDEF 水平对齐")?,
                    73 => vertical_align = parse_i16(&value, "ATTDEF 垂直对齐")?,
                    74 => line_spacing_style = parse_i16(&value, "ATTDEF 行距样式")?,
                    44 => line_spacing_factor = parse_f64(&value, "ATTDEF 行距因子")?,
                    280 => lock_position = parse_i16(&value, "ATTDEF 锁定标志")? != 0,
                    100 | 101 | 102 | 210 | 220 | 230 | 360 | 71 => {
                        // 暂时忽略的字段
                    }
                    _ => {}
                },
                None => return Err(DxfError::invalid("ATTDEF 未正确结束")),
            }
        }

        let layer = layer.unwrap_or_else(|| "0".to_string());
        let ix = insert_x.ok_or_else(|| DxfError::invalid("ATTDEF 缺少插入点 X（组码 10）"))?;
        let iy = insert_y.ok_or_else(|| DxfError::invalid("ATTDEF 缺少插入点 Y（组码 20）"))?;
        let height = height.unwrap_or(0.0);
        let default_raw =
            default_text.ok_or_else(|| DxfError::invalid("ATTDEF 缺少默认文本（组码 1）"))?;
        let tag = tag.ok_or_else(|| DxfError::invalid("ATTDEF 缺少标记（组码 2）"))?;
        let decoded_default = decode_inline_text(&default_raw);
        let alignment = match (align_x, align_y) {
            (Some(x), Some(y)) => Some(Point2::new(x, y)),
            _ => None,
        };

        Ok(AttributeDefinition {
            tag,
            prompt,
            default_text: decoded_default,
            insert: Point2::new(ix, iy),
            height,
            rotation: rotation_deg.to_radians(),
            width_factor,
            oblique: oblique_deg.to_radians(),
            style,
            alignment,
            horizontal_align,
            vertical_align,
            line_spacing_factor,
            line_spacing_style,
            is_invisible: flags & 0x01 != 0,
            is_constant: flags & 0x02 != 0,
            is_verify: flags & 0x04 != 0,
            is_preset: flags & 0x08 != 0,
            lock_position,
            layer,
        })
    }

    fn skip_entity_body(&mut self) -> Result<(), DxfError> {
        loop {
            match self.reader.next_pair()? {
                Some((0, value)) => {
                    self.reader.put_back((0, value));
                    break;
                }
                Some(_) => continue,
                None => break,
            }
        }
        Ok(())
    }
}

struct DxfReader<'a> {
    lines: std::str::Lines<'a>,
    buffer: Option<(i32, String)>,
    line_number: usize,
}

impl<'a> DxfReader<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            lines: source.lines(),
            buffer: None,
            line_number: 0,
        }
    }

    fn next_pair(&mut self) -> Result<Option<(i32, String)>, DxfError> {
        if let Some(pair) = self.buffer.take() {
            return Ok(Some(pair));
        }

        let code_line = match self.lines.next() {
            Some(line) => {
                self.line_number += 1;
                line
            }
            None => return Ok(None),
        };

        let value_line = match self.lines.next() {
            Some(line) => {
                self.line_number += 1;
                line
            }
            None => {
                return Err(DxfError::invalid(format!(
                    "文件在第 {} 行结束，缺少与组码对应的值行",
                    self.line_number
                )));
            }
        };

        let code = code_line.trim().parse::<i32>().map_err(|_| {
            DxfError::invalid(format!(
                "第 {} 行的组码 \"{}\" 无法解析为整数",
                self.line_number - 1,
                code_line.trim()
            ))
        })?;
        let value = value_line.trim_end_matches('\r').to_string();
        Ok(Some((code, value)))
    }

    fn put_back(&mut self, pair: (i32, String)) {
        if self.buffer.is_some() {
            panic!("内部错误：尝试多次回退 DXF pair");
        }
        self.buffer = Some(pair);
    }
}

fn assign_coord(slot: &mut Option<f64>, raw: &str, context: &str) -> Result<(), DxfError> {
    if slot.is_some() {
        return Err(DxfError::invalid(format!("{context} 出现重复值")));
    }
    *slot = Some(parse_f64(raw, context)?);
    Ok(())
}

fn build_face_vertex(
    index: usize,
    x: Option<f64>,
    y: Option<f64>,
    z: Option<f64>,
) -> Result<Option<Point3>, DxfError> {
    match (x, y, z) {
        (None, None, None) => Ok(None),
        (Some(x), Some(y), z) => Ok(Some(Point3::new(x, y, z.unwrap_or(0.0)))),
        _ => Err(DxfError::invalid(format!(
            "3DFACE 顶点 {index} 缺少完整的 XY 坐标"
        ))),
    }
}

fn parse_f64(raw: &str, context: &str) -> Result<f64, DxfError> {
    raw.trim()
        .parse::<f64>()
        .map_err(|_| DxfError::invalid(format!("{context} 解析失败（值：\"{raw}\"）")))
}

fn parse_i32(raw: &str, context: &str) -> Result<i32, DxfError> {
    raw.trim()
        .parse::<i32>()
        .map_err(|_| DxfError::invalid(format!("{context} 解析失败（值：\"{raw}\"）")))
}

fn parse_i16(raw: &str, context: &str) -> Result<i16, DxfError> {
    let value = parse_i32(raw, context)?;
    i16::try_from(value)
        .map_err(|_| DxfError::invalid(format!("{context} 超出 i16 范围（值：{value}）")))
}

fn parse_u32(raw: &str, context: &str) -> Result<u32, DxfError> {
    raw.trim()
        .parse::<u32>()
        .map_err(|_| DxfError::invalid(format!("{context} 解析失败（值：\"{raw}\"）")))
}

fn decode_mtext_content(raw: &str) -> String {
    let mut result = String::new();
    let mut chars = raw.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('P') | Some('p') => result.push('\n'),
                Some('~') => result.push(' '),
                Some('\\') => result.push('\\'),
                Some(other) => {
                    result.push('\\');
                    result.push(other);
                }
                None => result.push('\\'),
            }
        } else {
            result.push(ch);
        }
    }
    result
}

fn decode_inline_text(raw: &str) -> String {
    let mut result = String::new();
    let mut chars = raw.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('P') | Some('p') => result.push('\n'),
                Some('~') => result.push(' '),
                Some('\\') => result.push('\\'),
                Some('S') | Some('s') => {
                    // 跳过堆叠分数段；未来需要可在此扩展为具体格式。
                    while let Some(next) = chars.next() {
                        if next == ';' {
                            break;
                        }
                    }
                }
                Some(other) => {
                    result.push('\\');
                    result.push(other);
                }
                None => result.push('\\'),
            }
        } else {
            result.push(ch);
        }
    }
    result
}
