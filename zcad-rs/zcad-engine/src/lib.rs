pub mod command;

pub mod errors {
    use thiserror::Error;

    #[derive(Debug, Error)]
    pub enum EngineError {
        #[error("document is not initialized")]
        DocumentNotInitialized,
        #[error("entity with id {0} not found")]
        EntityNotFound(u64),
    }
}

pub mod scene {
    use std::collections::HashSet;

    use tracing::debug;
    use zcad_core::document::{Document, Entity, EntityId};
    use zcad_core::geometry::{Bounds2D, Point2, Vector2};

    use crate::errors::EngineError;

    const DEFAULT_ZOOM: f64 = 1.0;
    const MIN_ZOOM: f64 = 0.01;
    const MAX_ZOOM: f64 = 1_000.0;

    /// 记录视口状态（中心点与缩放），后续可扩展旋转角与裁剪范围。
    #[derive(Debug, Clone, Copy)]
    pub struct ViewportState {
        pub center: Point2,
        pub zoom: f64,
    }

    impl ViewportState {
        #[inline]
        fn clamp_zoom(value: f64) -> f64 {
            value.clamp(MIN_ZOOM, MAX_ZOOM)
        }
    }

    impl Default for ViewportState {
        fn default() -> Self {
            Self {
                center: Point2::new(0.0, 0.0),
                zoom: DEFAULT_ZOOM,
            }
        }
    }

    /// 引擎层负责维护 `Document` 和运行时状态（选中集、视图设置等）。
    #[derive(Debug)]
    pub struct Scene {
        document: Document,
        selected: HashSet<EntityId>,
        viewport: ViewportState,
    }

    #[derive(Debug, Clone, Copy)]
    pub struct DemoEntities {
        pub baseline: EntityId,
        pub circle: EntityId,
        pub arc: EntityId,
        pub polyline: EntityId,
        pub label: EntityId,
    }

    impl Scene {
        pub fn new() -> Self {
            Self {
                document: Document::new(),
                selected: HashSet::new(),
                viewport: ViewportState::default(),
            }
        }

        /// 使用现有文档初始化场景。
        pub fn with_document(document: Document) -> Self {
            let mut scene = Self::new();
            scene.load_document(document);
            scene
        }

        /// 使用默认样例重置场景。调用后会清空文档和选中状态。
        pub fn reset(&mut self) {
            self.document = Document::new();
            self.selected.clear();
            self.viewport = ViewportState::default();
        }

        /// 替换当前文档并重置运行时状态。
        pub fn load_document(&mut self, document: Document) {
            self.document = document;
            self.selected.clear();
            self.viewport = ViewportState::default();

            if let Some(bounds) = self.document.bounds() {
                self.viewport.center = bounds.center();
            }
        }

        /// 返回当前选中实体数量。
        #[inline]
        pub fn selection_len(&self) -> usize {
            self.selected.len()
        }

        /// 判定指定实体是否在选中集中。
        #[inline]
        pub fn is_selected(&self, id: EntityId) -> bool {
            self.selected.contains(&id)
        }

        /// 选中指定实体。若实体不存在则返回错误。
        pub fn select(&mut self, id: EntityId) -> Result<(), EngineError> {
            if self.document.entity(id).is_none() {
                return Err(EngineError::EntityNotFound(id.get()));
            }
            self.selected.insert(id);
            Ok(())
        }

        /// 取消选中指定实体，返回之前是否处于选中状态。
        pub fn deselect(&mut self, id: EntityId) -> bool {
            self.selected.remove(&id)
        }

        /// 切换实体选中状态，返回切换后的状态。
        pub fn toggle_selection(&mut self, id: EntityId) -> Result<bool, EngineError> {
            if self.document.entity(id).is_none() {
                return Err(EngineError::EntityNotFound(id.get()));
            }
            if !self.selected.insert(id) {
                self.selected.remove(&id);
                Ok(false)
            } else {
                Ok(true)
            }
        }

        /// 清空当前选中集。
        #[inline]
        pub fn clear_selection(&mut self) {
            self.selected.clear();
        }

        /// 以迭代形式返回当前选中实体 ID。
        #[inline]
        pub fn selection(&self) -> impl Iterator<Item = EntityId> + '_ {
            self.selected.iter().copied()
        }

        /// 返回当前选中实体的包围盒。
        pub fn selection_bounds(&self) -> Option<Bounds2D> {
            let mut bounds = Bounds2D::empty();
            let mut has = false;
            for id in &self.selected {
                if let Some(entity_bounds) = self.document.entity_bounds(*id) {
                    bounds.include_bounds(&entity_bounds);
                    has = true;
                }
            }
            if has { Some(bounds) } else { None }
        }

        /// 获取当前视口状态。
        #[inline]
        pub fn viewport(&self) -> ViewportState {
            self.viewport
        }

        /// 重置视口到默认状态。
        #[inline]
        pub fn reset_viewport(&mut self) {
            self.viewport = ViewportState::default();
        }

        /// 设置视口中心点。
        #[inline]
        pub fn set_viewport_center(&mut self, center: Point2) {
            self.viewport.center = center;
        }

        /// 平移视口中心。
        pub fn pan_viewport(&mut self, delta: Vector2) {
            self.viewport.center = self.viewport.center.translate(delta);
        }

        /// 设置缩放倍数（自动限制在合法范围内）。
        pub fn set_viewport_zoom(&mut self, zoom: f64) {
            self.viewport.zoom = ViewportState::clamp_zoom(zoom);
        }

        /// 按乘法因子调整缩放。
        pub fn scale_viewport_zoom(&mut self, factor: f64) {
            let current = self.viewport.zoom;
            let target = if factor.is_finite() {
                current * factor
            } else {
                current
            };
            self.set_viewport_zoom(target);
        }

        /// 聚焦当前选中实体，若为空则退化到整个文档范围。
        pub fn focus_on_selection(&mut self) {
            let target = self.selection_bounds().or_else(|| self.document.bounds());
            if let Some(bounds) = target {
                self.viewport.center = bounds.center();
            }
        }

        #[inline]
        pub fn document(&self) -> &Document {
            &self.document
        }

        #[inline]
        pub fn document_mut(&mut self) -> &mut Document {
            &mut self.document
        }

        /// 为 CLI / 快速验证填充一组示例实体，返回关键实体 ID。
        pub fn populate_demo(&mut self) -> DemoEntities {
            use std::f64::consts::{FRAC_PI_2, FRAC_PI_4};

            self.clear_selection();

            let baseline =
                self.document
                    .add_line(Point2::new(0.0, 0.0), Point2::new(100.0, 0.0), "0");
            let circle = self
                .document
                .add_circle(Point2::new(50.0, 25.0), 12.5, "ANNOT");
            let arc = self
                .document
                .add_arc(Point2::new(20.0, 10.0), 7.5, 0.0, FRAC_PI_2, "ANNOT");
            let polyline = self.document.add_polyline(
                [
                    Point2::new(0.0, 10.0),
                    Point2::new(10.0, 20.0),
                    Point2::new(25.0, 5.0),
                ],
                false,
                "SKETCH",
            );
            let label = self.document.add_text(
                Point2::new(5.0, 12.0),
                "Rust 移植示例",
                3.5,
                FRAC_PI_4,
                "ANNOT",
            );

            let ids = DemoEntities {
                baseline,
                circle,
                arc,
                polyline,
                label,
            };

            debug!(
                baseline = ids.baseline.get(),
                circle = ids.circle.get(),
                arc = ids.arc.get(),
                polyline = ids.polyline.get(),
                label = ids.label.get(),
                "已创建演示实体"
            );

            ids
        }

        pub fn entity(&self, id: EntityId) -> Option<&Entity> {
            self.document().entity(id)
        }
    }

    impl Default for Scene {
        fn default() -> Self {
            Self::new()
        }
    }

    #[cfg(test)]
    mod tests {
        use zcad_core::document::Document;

        use super::*;

        #[test]
        fn demo_population_creates_entities() {
            let mut scene = Scene::new();
            let ids = scene.populate_demo();
            assert_eq!(scene.document().entities().count(), 5);
            assert!(scene.entity(ids.arc).is_some());
            assert!(scene.entity(ids.polyline).is_some());
        }

        #[test]
        fn selection_operations_work() {
            let mut scene = Scene::new();
            let ids = scene.populate_demo();

            assert_eq!(scene.selection_len(), 0);
            assert!(!scene.is_selected(ids.circle));

            scene.select(ids.circle).expect("select circle");
            assert!(scene.is_selected(ids.circle));
            assert_eq!(scene.selection_len(), 1);

            // toggle should remove when already selected
            let now_selected = scene
                .toggle_selection(ids.circle)
                .expect("toggle existing selection");
            assert!(!now_selected);
            assert!(!scene.is_selected(ids.circle));

            // toggle again selects
            let now_selected = scene.toggle_selection(ids.circle).expect("toggle again");
            assert!(now_selected);
            assert!(scene.is_selected(ids.circle));

            // deselect returns true only if it was selected
            assert!(scene.deselect(ids.circle));
            assert!(!scene.deselect(ids.circle));
            assert_eq!(scene.selection_len(), 0);

            // selecting missing entity results in error
            let missing = EntityId::new(9_999);
            let err = scene.select(missing).unwrap_err();
            assert!(matches!(err, EngineError::EntityNotFound(_)));
        }

        #[test]
        fn viewport_state_clamps_zoom() {
            let mut scene = Scene::new();
            let default = scene.viewport();
            assert!((default.zoom - 1.0).abs() < f64::EPSILON);
            assert!((default.center.x()).abs() < f64::EPSILON);

            scene.set_viewport_center(Point2::new(10.0, -5.0));
            assert_eq!(scene.viewport().center.x(), 10.0);
            assert_eq!(scene.viewport().center.y(), -5.0);

            scene.pan_viewport(Vector2::new(5.0, 5.0));
            assert_eq!(scene.viewport().center.x(), 15.0);
            assert_eq!(scene.viewport().center.y(), 0.0);

            scene.set_viewport_zoom(0.0001);
            assert!((scene.viewport().zoom - MIN_ZOOM).abs() < f64::EPSILON);

            scene.set_viewport_zoom(10_000.0);
            assert!((scene.viewport().zoom - MAX_ZOOM).abs() < f64::EPSILON);

            scene.set_viewport_zoom(2.0);
            scene.scale_viewport_zoom(0.5);
            assert!((scene.viewport().zoom - 1.0).abs() < f64::EPSILON);

            scene.reset_viewport();
            let reset = scene.viewport();
            assert!((reset.zoom - 1.0).abs() < f64::EPSILON);
            assert!((reset.center.x()).abs() < f64::EPSILON);
            assert!((reset.center.y()).abs() < f64::EPSILON);
        }

        #[test]
        fn focus_on_selection_recenters_viewport() {
            let mut scene = Scene::new();
            let ids = scene.populate_demo();
            scene.select(ids.circle).unwrap();
            scene.select(ids.label).unwrap();

            scene.focus_on_selection();
            let viewport = scene.viewport();
            assert!((viewport.center.x() - 33.75).abs() < 1e-9);
            assert!((viewport.center.y() - 24.75).abs() < 1e-9);

            scene.clear_selection();
            scene.focus_on_selection();
            let viewport_all = scene.viewport();
            assert!((viewport_all.center.x() - 50.0).abs() < 1e-9);
            assert!((viewport_all.center.y() - 18.75).abs() < 1e-9);
        }

        #[test]
        fn load_document_resets_state_and_recenters_viewport() {
            let mut scene = Scene::new();
            let ids = scene.populate_demo();
            scene.select(ids.circle).unwrap();
            scene.set_viewport_center(Point2::new(999.0, 999.0));
            scene.set_viewport_zoom(42.0);

            let mut document = Document::new();
            document.add_line(Point2::new(-10.0, -10.0), Point2::new(0.0, 10.0), "GEOM");
            document.add_circle(Point2::new(10.0, 0.0), 5.0, "GEOM");

            scene.load_document(document);

            assert_eq!(scene.selection_len(), 0);
            assert!(scene.selection().next().is_none());
            assert_eq!(scene.document().entities().count(), 2);

            let viewport = scene.viewport();
            let expected_center = scene
                .document()
                .bounds()
                .expect("document should have bounds")
                .center();
            assert!((viewport.zoom - 1.0).abs() < f64::EPSILON);
            assert!((viewport.center.x() - expected_center.x()).abs() < 1e-9);
            assert!((viewport.center.y() - expected_center.y()).abs() < 1e-9);
        }
    }
}
