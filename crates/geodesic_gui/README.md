# Canvas rendering

The canvas uses Lyon path tessellation and Bevy `Mesh2d` rasterization. Filled
markers and wide strokes are triangle meshes, not scanlines or repeated gizmos.
`path_renderer.rs` is an internal GUI boundary with separate path fill and stroke
operations; it accepts line segments and Bezier paths independently of the
geometric domain. Lyon is required because Bevy's primitive meshes do not supply
general path fill/stroke tessellation. It does not enter the core dependency tree.

## Coordinates and classification

- Core extraction removes complex homogeneous scale and phase before testing
  reality. Original complex coordinates remain unchanged. GUI picking and SVG
  use the same extraction as the canvas.
- A normalized imaginary residual at most `1e-8` is accepted as numeric noise;
  residuals between `1e-8` and `1e-5` are numerically uncertain; larger residuals
  are classified non-real. Ill-conditioned affine division is also uncertain.
  These are floating-point policies, not certified error estimates.
- Only renderable values enter paths. Non-real, infinite and uncertain objects
  remain in the algebra panel with a visibility explanation.
- Positions are transformed into viewport-local logical pixels using `f64`
  before conversion to mesh `f32`. Infinite lines are clipped to the viewport.
  Marker radii and stroke widths stay fixed in logical pixels across camera zoom.
- Lyon flattening tolerance is `0.1` physical pixel, adjusted for the window's
  scale factor. This controls path approximation, not upstream numeric error.

## Appearance and ordering

One reusable mesh asset contains the painter-ordered triangles: grid, axes,
lines, then points. Selection halos precede their object. Vertex colors retain
alpha; the material uses alpha blending and the camera uses four-sample MSAA.
There is no overlap between repeated parallel strokes to darken a wide line.

Persisted style sizes retain the existing schema. `0.03` maps to one logical
pixel of marker radius, or half a logical pixel of line width. Unstyled free
points have radius 5 and dependent points radius 4; the point editor offers 1–9,
and line thickness 1–13 (actual width 0.5–6.5 pixels). Opening the editor does not
create a style override. SVG retains its existing view-box style units.

Behavior references, independently reimplemented from local GeoGebra sources:

- `DrawPoint.java`: filled dot, black outline, selection radius `2*r+1`.
- `Drawable.java::updateStrokes`: width `thickness/2`, selected line width
  `2*max(width,1)+2`.
- `GeoElement.java::getSelColor`: object-colored selection tint, alpha 0.2.
  This adapter additionally respects the object's opacity.
- `DrawConic.java` and desktop/web `GGraphics2D` adapters: separate path fill and
  stroke operations, including Bezier curves and independent hyperbola branches.

Future circle/conic adapters must extract their real locus before building paths;
real coefficients alone do not guarantee a real locus. They must clip branches
and supply separate fill and stroke paths so a clipping boundary is never
stroked as part of a curve. Current public geometry remains points and lines.

## Verification

`cargo test --workspace` includes normalized projection cases and mesh-area tests
for filled disks, fractional-width strokes, alpha preservation and Bezier paths.
The opt-in GPU test checks actual pixels for fill, ordering, transparency, fixed
marker size and complex-object visibility, and writes a reviewable screenshot:

```sh
cargo test -p geodesic_gui capture_canvas_paths -- --ignored --nocapture
GEODESIC_TEST_DPI=2 GEODESIC_TEST_ZOOM=0.08 \
GEODESIC_TEST_SCREENSHOT=/tmp/opencode/geodesic-paths-hidpi.png \
cargo test -p geodesic_gui capture_canvas_paths -- --ignored --nocapture
```

A display and working GPU adapter are required. The default screenshot path is
`/tmp/opencode/geodesic-paths.png`; set `GEODESIC_TEST_SCREENSHOT` on other systems.
