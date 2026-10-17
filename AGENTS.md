# Geodesic Agent Guide

## Project Mission

Geodesic is a non-commercial Rust rewrite inspired by GeoGebra Classic 6.
It is a desktop-first dynamic geometry application with a strict separation
between its geometric core and its adapters:

- Bevy renders the geometric canvas.
- `bevy_egui` provides application chrome, inspectors, toolbars, dialogs,
  theming, and the command bar.
- The core is reusable without a GUI by the CLI, SVG renderer, and AI adapters.
- Project documents use an S-expression format and can contain Steel scripts.
- The kernel supports complex projective geometry while the initial canvas
  renders only the real affine chart.

Do not treat this as a line-by-line Java port. Preserve user-observable
geometric semantics where they are explicitly in scope, but design idiomatic
Rust boundaries and types.

The `geogebra-src/` directory contains the GeoGebra source code. Consult it as
an implementation reference when developing corresponding behavior, while
respecting the licensing constraints below.

## Product and License Constraints

- This project is distributed only for non-commercial use while it reuses
  GeoGebra UI assets.
- GeoGebra UI images, icons, logos, styles, language files, and documentation
  are subject to CC BY-NC-SA 4.0 or later. Preserve provenance for every reused
  asset and include attribution, a copyright notice, a GeoGebra website link,
  and `Made with GeoGebra®` in the product attribution surface.
- Do not add GeoGebra assets or language files without recording their source
  path and license in an asset manifest.
- Do not imply that GeoGebra endorses this project. Do not use GeoGebra
  trademarks beyond the license and attribution requirements.
- GeoGebra source code is EUPL 1.2. Do not copy source code into this project
  without checking the resulting licensing obligations. Reimplement behavior
  from documented semantics and tests where possible.

## Domain Glossary

Use these terms consistently in code, docs, CLI output, and user-visible
errors.

| Term | Meaning |
| --- | --- |
| `Project` | The complete persisted geometric document: construction, document settings, canvas settings, style presets, and scripts. |
| `GeometricObject` | A named object in a `Project`, such as a point or a line. |
| `Label` | The unique, user-visible `snake_case` identifier of a `GeometricObject`. A label is its canonical public identity. |
| `Construction` | The ordered dependency graph and command history which derive dependent objects from inputs. |
| `FreeObject` | A geometric object created from literal input and independently movable when its domain permits it. |
| `DependentObject` | A geometric object derived from other objects or expressions. |
| `ProjectivePoint` | A point represented internally by homogeneous coordinates. |
| `ProjectiveLine` | A line represented by homogeneous coefficients and evaluated through projective incidence. |
| `RealAffineChart` | The initial renderable chart of the real locus, displayed as ordinary 2D Euclidean coordinates. |
| `ComplexObject` | A valid object over complex coordinates which is not renderable in the real affine chart. |
| `StylePreset` | A named reusable set of point or line visual properties stored in the project. |
| `CompatibilityPlaceholder` | A read-only representation of a `.ggb` object unsupported by the current native model. |
| `Script` | A Steel/Scheme program attached to a project event and restricted to registered application primitives. |
| `GeoCommand` | A validated command that changes a `Construction` or a project-level setting. |
| `GeometrySnapshot` | An immutable evaluated view of a project used by renderers and adapters. |

Avoid overloaded terms such as "entity", "node", or "element" when one of
the domain terms above is more precise.

## Confirmed Product Decisions

### Scope and platforms

- Target GeoGebra Classic 6 as a long-term compatibility reference, not as an
  all-at-once first release.
- The first vertical slice is point-and-line projective geometry only.
- The first GUI target is native desktop: Linux, macOS, and Windows.
- Web/WASM is a later adapter; do not make first-release choices that require
  it, but keep the core platform-neutral.
- The project is one process with strict crate boundaries. It is not initially
  a local or remote geometry server.

### Initial geometric capability

The first deliverable includes:

- free and dependent points;
- real projective lines;
- incidence and line intersection;
- parallel and perpendicular constructions in the real affine chart;
- selection, dragging, pan, zoom, undo, and redo;
- point and line labels;
- direct styles and named style presets;
- the Lisp project format, command bar, SVG export, CLI JSON mode, and MCP
  adapter.

Circles, arcs, polygons, conics, functions, CAS, spreadsheets, 3D geometry,
and broad Classic 6 tool parity are explicitly later milestones.

### Numeric and projective model

- Follow GeoGebra's numeric-first approach: use `f64` for ordinary geometric
  evaluation, with configurable tolerances initially aligned to GeoGebra's
  `STANDARD_PRECISION = 1e-8` and `MIN_PRECISION = 1e-5`.
- Keep comparison and degeneracy policy centralized. Never scatter ad-hoc
  epsilon tests through algorithms or rendering code.
- Represent 2D projective points with homogeneous complex coordinates and
  projective lines with homogeneous complex coefficients.
- Render only the real affine chart in the initial canvas. Do not claim that a
  2D canvas fully visualizes complex projective space or all of the real
  projective plane topology.
- CLI, MCP, Steel scripts, and the Ex command bar may create complex objects.
  They appear in the algebra/inspector surface but remain hidden from the
  canvas unless they evaluate to a renderable real affine object.

### Identity and naming

- A `Label` is the unique canonical identity of every public geometric object.
  Do not introduce public UUID references as an alternative identity system.
- Reject duplicate labels and invalid renames.
- Use Rust-like public naming everywhere:
  - types and enum variants: `PascalCase`;
  - functions, fields, labels, commands, and Steel primitives: `snake_case`;
  - CLI subcommands: `kebab-case`.
- Prefer explicit namespaces in Steel, for example
  `(geo::line_through point_a point_b)`.

### Project format and scripting

- The project format is a versioned S-expression document. Do not select or
  introduce a filename extension until it is explicitly decided.
- Data sections are parsed and schema-validated before evaluation. Project
  loading must not accidentally execute arbitrary data forms.
- Use Steel as the embedded Scheme runtime. Pin a compatible `0.8.x` release
  behind a local adapter crate so pre-1.0 Steel API changes do not leak into
  the rest of the workspace.
- Use a data parser/serializer such as `lexpr` for project data and Steel only
  for explicit script sections. Keep persistence independent from the VM.
- Scripts execute only inside the application and receive only registered
  geometry/UI primitives. Do not expose file, network, process, FFI, or dynamic
  module-loading capabilities.
- Scripts run with GeoGebra-like incremental semantics: each valid command
  commits immediately; a later failure stops that script and does not roll back
  earlier committed commands.
- Bound execution with instruction/step, recursion, allocation, and event-loop
  recursion limits. Always report script source locations and the command that
  failed.

### Compatibility

- `.ggb` support is read-only in the first release.
- Convert supported point-and-line objects into native objects.
- Represent unsupported objects as `CompatibilityPlaceholder` values with their
  label, source type, definition summary, and known dependencies.
- Emit a structured compatibility report in the GUI, CLI JSON output, and MCP.
  Never silently discard unsupported content.
- Ignore imported GGBScript entirely in the first release. Do not execute,
  preserve, or attempt best-effort translation of it.
- `.ggb` export is a later milestone.

### Visual design and themes

- Use Bevy for the geometry canvas and `bevy_egui` for application UI.
- UI themes are user preferences. Provide system, light, and dark themes first.
- Canvas settings are project data: background, grid, axes, and object styles.
  Opening a project must not override the user's toolbar and panel theme.
- Points support shape, size, fill, stroke, stroke width, opacity, and label.
- Lines support color, width, solid/dashed/dotted patterns, opacity, endpoint
  or arrow options, and label.
- A `StylePreset` may be referenced and locally overridden by an object.
- SVG output must preserve supported point and line style semantics.

### Vim-style command bar

- Implement an Ex-style command bar, not full Vim modal editing.
- `:` focuses the command bar when no text editor owns keyboard focus.
- The command bar supports history, completion, source-aware errors, and
  commands such as `:save`, `:export svg`, `:theme dark`, `:select point_a`,
  and `:line point_a point_b`.
- Text inputs and script editors retain conventional non-modal editing behavior.

### Automation and headless use

- The CLI is the canonical automation protocol. Human scripts, CI, and AI use
  the same deterministic operations.
- Support both human-readable output and line-delimited or request/response JSON
  output. Emit data to stdout and diagnostics to stderr.
- Implement MCP/JSON-RPC as an adapter over the CLI/core command surface, not
  as an independent geometry implementation.
- Initial headless rendering guarantees SVG only. It must operate from a core
  `GeometrySnapshot` and never launch Bevy.
- Expected first commands include `validate`, `import`, `render-svg`, and a
  structured `eval`/command endpoint. Define exact flags and JSON schemas before
  making them public.

## Intended Workspace Boundaries

Keep workspace crates dependency-directional. The following names describe the
intended responsibilities; adjust only with a documented reason.

```text
geodesic_core  <- domain model, construction graph, evaluation, tolerances
geodesic_lisp  <- project schema bridge and Steel adapter/primitives
geodesic_io    <- project persistence and read-only .ggb import/reporting
geodesic_svg   <- deterministic SVG rendering from GeometrySnapshot
geodesic_cli   <- clap commands and JSON automation protocol
geodesic_mcp   <- MCP/JSON-RPC adapter over core/CLI command semantics
geodesic_gui   <- Bevy canvas and bevy_egui desktop application
```

Rules:

- `geodesic_core` must not depend on Bevy, egui, Steel, clap, MCP, filesystem
  UI state, or platform APIs.
- Rendering consumes immutable snapshots; it does not mutate constructions.
- UI input, CLI requests, MCP requests, and scripts produce validated
  `GeoCommand` values and use one application service to apply them.
- Do not expose `Arc<Mutex<_>>`, Bevy `Entity`, Steel value types, or renderer
  types from the core public API.
- Keep public APIs deliberate and re-exported through crate roots. A public item
  is a compatibility promise.

## Implementation Order

1. Create the workspace and domain-only `geodesic_core`; define the glossary as
   Rust types, label validation, error taxonomy, projective coordinate types,
   construction graph, and command application boundary.
2. Add evaluation, tolerance policy, dependency invalidation, transactions/undo
   units, and unit/property tests for point-line incidence and degeneracy.
3. Define the versioned S-expression schema and implement validated load/save in
   `geodesic_io`; add fixtures and round-trip tests before GUI work.
4. Add the Steel adapter with only registered application primitives, resource
   limits, event dispatch, incremental command semantics, and script tests.
5. Implement deterministic `geodesic_svg` and the CLI. Prove headless SVG output
   with golden tests.
6. Implement point-line `.ggb` import plus structured compatibility placeholders
   and reports.
7. Implement the Bevy/egui desktop adapter: canvas, interaction, style editing,
   algebra/inspector handling for complex objects, themes, and Ex command bar.
8. Add MCP/JSON-RPC over the already-tested command API.
9. Add later geometry families only after their domain model, file-schema impact,
   import behavior, rendering behavior, CLI behavior, and tests are specified.

## Quality Gates

- Every construction algorithm has tests for ordinary, parallel, coincident,
  infinite, undefined, and numerically near-degenerate cases.
- Test that only real affine objects are sent to the initial canvas renderer.
- Test that complex objects remain available to algebra, CLI, MCP, and scripts.
- Test project schema validation, format migration, deterministic serialization,
  and load-save-load equivalence.
- Test SVG output with golden fixtures for points, lines, styles, presets, grid,
  and hidden complex objects.
- Test CLI JSON schemas and exit codes. Data goes to stdout; failures and logs go
  to stderr.
- Test each `.ggb` fixture for converted objects, placeholders, and report
  contents. Assert that imported GGBScript does not execute.
- Test command-bar parsing, label ambiguity/conflicts, history, and completion.
- Test script command ordering, partial commit on failure, loop limits, and undo
  behavior.
- Run `cargo fmt --check`, `cargo clippy --workspace --all-targets`, `cargo test
  --workspace`, and relevant integration/golden tests before claiming a phase is
  complete.

## Agent Rules

- Read this file before changing the workspace or public behavior.
- Preserve the domain glossary. If a new term or invariant is needed, define it
  here before spreading an ambiguous name through code.
- Do not broaden compatibility, scripting capabilities, export formats, or
  geometry scope silently. Propose a new milestone and tests first.
- Do not add a dependency solely for convenience without documenting why an
  existing workspace boundary or standard-library solution is insufficient.
- Avoid panics in project loading, import, command handling, scripting, CLI, and
  rendering paths; return domain-specific errors with useful context.
- Keep dependencies and platform-specific code at adapter edges.
- Do not modify or redistribute GeoGebra assets without maintaining the asset
  manifest and required attribution.
