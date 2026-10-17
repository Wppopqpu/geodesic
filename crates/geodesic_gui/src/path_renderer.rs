//! Screen-space path fill/stroke tessellation, independent of geometric objects.
//!
//! Lyon owns curve flattening and triangulation; Bevy owns rasterization and
//! MSAA. All dimensions here are logical pixels. Closed fill paths and open
//! stroke paths remain separate so viewport clipping never invents a border.

use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::mesh::{Indices, PrimitiveTopology},
};
use lyon_tessellation::{
    BuffersBuilder, FillOptions, FillTessellator, FillVertex, StrokeOptions, StrokeTessellator,
    StrokeVertex, VertexBuffers,
    path::{Path, Winding, math::point},
};

/// Maximum flattening deviation in physical pixels (not a geometric error bound).
const PATH_TOLERANCE: f32 = 0.1;

#[derive(Clone, Copy, Default)]
struct Vertex {
    position: [f32; 3],
    color: [f32; 4],
}

/// One painter-ordered batch. Reused across frames, including tessellator storage.
#[derive(Default)]
pub(crate) struct PathPainter {
    fill: FillTessellator,
    stroke: StrokeTessellator,
    geometry: VertexBuffers<Vertex, u32>,
    error: Option<String>,
    pixel_scale: f32,
}

impl PathPainter {
    pub(crate) fn set_pixel_scale(&mut self, scale: f32) {
        self.pixel_scale = scale;
    }

    pub(crate) fn clear(&mut self) {
        self.geometry.vertices.clear();
        self.geometry.indices.clear();
        self.error = None;
    }

    /// A tessellation failure invalidates this frame instead of showing a partial mesh.
    pub(crate) fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub(crate) fn fill_path(&mut self, path: &Path, color: Color) {
        let color = color.to_linear().to_f32_array();
        if let Err(error) = self.fill.tessellate_path(
            path,
            &FillOptions::default().with_tolerance(PATH_TOLERANCE / self.pixel_scale.max(1.0)),
            &mut BuffersBuilder::new(&mut self.geometry, |vertex: FillVertex<'_>| Vertex {
                position: [vertex.position().x, vertex.position().y, 0.0],
                color,
            }),
        ) {
            self.error = Some(format!("Canvas fill tessellation failed: {error:?}"));
        }
    }

    pub(crate) fn stroke_path(&mut self, path: &Path, width: f32, color: Color) {
        let color = color.to_linear().to_f32_array();
        if let Err(error) = self.stroke.tessellate_path(
            path,
            &StrokeOptions::default()
                .with_line_width(width)
                .with_tolerance(PATH_TOLERANCE / self.pixel_scale.max(1.0)),
            &mut BuffersBuilder::new(&mut self.geometry, |vertex: StrokeVertex<'_, '_>| Vertex {
                position: [vertex.position().x, vertex.position().y, 0.0],
                color,
            }),
        ) {
            self.error = Some(format!("Canvas stroke tessellation failed: {error:?}"));
        }
    }

    pub(crate) fn disk(&mut self, center: Vec2, radius: f32, color: Color) {
        let mut builder = Path::builder();
        builder.add_circle(point(center.x, center.y), radius, Winding::Positive);
        self.fill_path(&builder.build(), color);
    }

    pub(crate) fn circle(&mut self, center: Vec2, radius: f32, width: f32, color: Color) {
        let mut builder = Path::builder();
        builder.add_circle(point(center.x, center.y), radius, Winding::Positive);
        self.stroke_path(&builder.build(), width, color);
    }

    pub(crate) fn line(&mut self, start: Vec2, end: Vec2, width: f32, color: Color) {
        let mut builder = Path::builder();
        builder.begin(point(start.x, start.y));
        builder.line_to(point(end.x, end.y));
        builder.end(false);
        self.stroke_path(&builder.build(), width, color);
    }

    pub(crate) fn triangle(&mut self, first: Vec2, second: Vec2, third: Vec2, color: Color) {
        let mut builder = Path::builder();
        builder.begin(point(first.x, first.y));
        builder.line_to(point(second.x, second.y));
        builder.line_to(point(third.x, third.y));
        builder.close();
        self.fill_path(&builder.build(), color);
    }

    pub(crate) fn polygon(&mut self, vertices: &[Vec2], color: Color) {
        let Some((&first, rest)) = vertices.split_first() else {
            return;
        };
        let mut builder = Path::builder();
        builder.begin(point(first.x, first.y));
        for vertex in rest {
            builder.line_to(point(vertex.x, vertex.y));
        }
        builder.close();
        self.fill_path(&builder.build(), color);
    }

    pub(crate) fn mesh(&self) -> Mesh {
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );
        let vertices = if self.error.is_none() {
            &self.geometry.vertices[..]
        } else {
            &[]
        };
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_POSITION,
            vertices.iter().map(|v| v.position).collect::<Vec<_>>(),
        );
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_COLOR,
            vertices.iter().map(|v| v.color).collect::<Vec<_>>(),
        );
        mesh.insert_indices(Indices::U32(if self.error.is_none() {
            self.geometry.indices.clone()
        } else {
            Vec::new()
        }));
        mesh
    }
}

#[derive(Default)]
pub(crate) struct CanvasBatch {
    pub(crate) painter: PathPainter,
    pub(crate) mesh: Option<Handle<Mesh>>,
    pub(crate) entity: Option<Entity>,
    pub(crate) last_error: Option<String>,
}

/// The single canvas mesh is expressed in pixels relative to the camera center.
#[derive(Component)]
pub(crate) struct CanvasMesh;

#[cfg(test)]
mod tests {
    use super::*;

    fn area(painter: &PathPainter) -> f32 {
        let (triangles, remainder) = painter.geometry.indices.as_chunks::<3>();
        assert_eq!(remainder.len(), 0);
        triangles
            .iter()
            .map(|triangle| {
                let [a, b, c] = triangle.map(|index| {
                    let p = painter.geometry.vertices[index as usize].position;
                    Vec2::new(p[0], p[1])
                });
                (b - a).perp_dot(c - a).abs() / 2.0
            })
            .sum()
    }

    #[test]
    fn disk_has_filled_area_and_unmodified_translucent_color() {
        let mut painter = PathPainter::default();
        // A selected point's halo exceeds the point-size slider maximum.
        painter.disk(Vec2::ZERO, 19.0, Color::srgba(1.0, 0.0, 0.0, 0.2));
        let expected = std::f32::consts::PI * 19.0_f32.powi(2);
        assert!((area(&painter) - expected).abs() / expected < 0.02);
        assert!(
            painter
                .geometry
                .vertices
                .iter()
                .all(|vertex| (vertex.color[3] - 0.2).abs() < f32::EPSILON)
        );
        assert!(painter.error().is_none());
    }

    #[test]
    fn fractional_stroke_width_is_not_rounded_or_overdrawn() {
        let mut painter = PathPainter::default();
        painter.line(Vec2::ZERO, Vec2::new(100.0, 0.0), 2.5, Color::WHITE);
        assert!((area(&painter) - 250.0).abs() < 0.001);
        assert!(
            painter
                .geometry
                .vertices
                .iter()
                .all(|vertex| (vertex.position[1].abs() - 1.25).abs() < 0.001)
        );
    }

    #[test]
    fn bezier_curves_use_the_same_stroke_backend() {
        let mut builder = Path::builder();
        builder.begin(point(-50.0, 50.0));
        builder.quadratic_bezier_to(point(0.0, -50.0), point(50.0, 50.0));
        builder.end(false);
        let mut painter = PathPainter::default();
        painter.stroke_path(&builder.build(), 2.5, Color::WHITE);
        assert!(painter.error().is_none());
        assert!(painter.geometry.indices.len() > 6);
        assert!(
            painter
                .geometry
                .vertices
                .iter()
                .all(|vertex| vertex.position.iter().all(|value| value.is_finite()))
        );
        assert!(area(&painter) > 250.0);
    }
}
