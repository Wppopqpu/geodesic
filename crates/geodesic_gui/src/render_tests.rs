//! Opt-in GPU regression capture: requires a display and a working GPU adapter.

use bevy::{
    app::AppExit,
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
    window::WindowResolution,
    winit::WinitPlugin,
};
use geodesic_core::{ComplexCoordinate, GeoCommand, Label, ProjectiveLine, ProjectivePoint};

use crate::{canvas::draw_canvas, state::GuiState};

#[test]
#[ignore = "requires a display/GPU; saves a reviewable canvas screenshot"]
#[allow(
    clippy::too_many_lines,
    reason = "One visual fixture intentionally declares every overlapping render case together."
)]
fn capture_canvas_paths() {
    let dpi = std::env::var("GEODESIC_TEST_DPI")
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(1.0);
    let scale = std::env::var("GEODESIC_TEST_ZOOM")
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(0.02);
    let mut state = GuiState::default();
    for (name, x, y, size, color, opacity) in [
        ("selected", -100.0, 50.0, 0.15, "#1565c0", 1.0),
        ("red", 100.0, 50.0, 0.27, "#ff0000", 0.5),
        ("dependent_size", 0.0, -80.0, 0.12, "#00aa00", 1.0),
    ] {
        let label = Label::parse(name).expect("label");
        state
            .project
            .construction
            .apply(GeoCommand::AddPoint {
                label: label.clone(),
                point: ProjectivePoint::real_affine(x * f64::from(scale), y * f64::from(scale)),
            })
            .expect("point");
        state.project.object_styles.insert(
            label,
            geodesic_io::StyleBinding::Concrete(geodesic_io::ObjectStyle {
                size,
                color: color.to_owned(),
                opacity,
                ..geodesic_io::ObjectStyle::default()
            }),
        );
    }
    for (name, coefficients, color, opacity, size) in [
        (
            "diagonal",
            [1.0.into(), (-1.0).into(), 0.0.into()],
            "#1565c0",
            1.0,
            0.15,
        ),
        (
            "horizontal",
            [0.0.into(), 1.0.into(), (-50.0 * f64::from(scale)).into()],
            "#ff0000",
            0.5,
            0.39,
        ),
        (
            "phase_real",
            [
                ComplexCoordinate::new(0.0, 1.0),
                0.0.into(),
                ComplexCoordinate::new(0.0, -150.0 * f64::from(scale)),
            ],
            "#00aa00",
            1.0,
            0.06,
        ),
        (
            "hidden_complex",
            [1.0.into(), ComplexCoordinate::new(0.0, 1.0), 0.0.into()],
            "#ff00ff",
            1.0,
            0.39,
        ),
    ] {
        let label = Label::parse(name).expect("label");
        let [a, b, c] = coefficients;
        state
            .project
            .construction
            .apply(GeoCommand::AddLine {
                label: label.clone(),
                line: ProjectiveLine::new(a, b, c).expect("line"),
            })
            .expect("add line");
        state.project.object_styles.insert(
            label,
            geodesic_io::StyleBinding::Concrete(geodesic_io::ObjectStyle {
                size,
                color: color.to_owned(),
                opacity,
                ..geodesic_io::ObjectStyle::default()
            }),
        );
    }
    for (label, command, color) in [
        (
            "segment_demo",
            GeoCommand::AddSegment {
                label: Label::parse("segment_demo").expect("label"),
                start: Label::parse("selected").expect("label"),
                end: Label::parse("dependent_size").expect("label"),
            },
            "#ff9800",
        ),
        (
            "ray_demo",
            GeoCommand::AddRay {
                label: Label::parse("ray_demo").expect("label"),
                origin: Label::parse("dependent_size").expect("label"),
                through: Label::parse("selected").expect("label"),
            },
            "#00695c",
        ),
        (
            "vector_demo",
            GeoCommand::AddVector {
                label: Label::parse("vector_demo").expect("label"),
                start: Label::parse("red").expect("label"),
                end: Label::parse("dependent_size").expect("label"),
            },
            "#6a1b9a",
        ),
    ] {
        state
            .project
            .construction
            .apply(command)
            .expect("bounded object");
        state.project.object_styles.insert(
            Label::parse(label).expect("label"),
            geodesic_io::StyleBinding::Concrete(geodesic_io::ObjectStyle {
                color: color.to_owned(),
                size: 0.18,
                opacity: 1.0,
                ..geodesic_io::ObjectStyle::default()
            }),
        );
    }
    state.selected = Some(Label::parse("selected").expect("label"));
    run_capture(state, dpi, scale);
}

fn run_capture(state: GuiState, dpi: f32, scale: f32) {
    let mut app = App::new();
    let winit = WinitPlugin {
        run_on_any_thread: true,
    };
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Geodesic path regression".to_owned(),
                    resolution: WindowResolution::new(640, 400).with_scale_factor_override(dpi),
                    ..default()
                }),
                ..default()
            })
            .set(winit),
    )
    .insert_resource(ClearColor(Color::WHITE))
    .insert_resource(state)
    .add_systems(Startup, move |mut commands: Commands| {
        commands.spawn((
            Camera2d,
            Msaa::Sample4,
            Projection::Orthographic(OrthographicProjection {
                scale,
                ..OrthographicProjection::default_2d()
            }),
        ));
    })
    .add_systems(Update, (draw_canvas, capture_after_warmup).chain());
    assert_eq!(app.run(), AppExit::Success);
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are owned wrappers."
)]
fn capture_after_warmup(
    mut commands: Commands,
    mut frames: Local<u32>,
    mut exit: MessageWriter<AppExit>,
) {
    *frames += 1;
    if *frames == 30 {
        let path = std::env::var("GEODESIC_TEST_SCREENSHOT")
            .unwrap_or_else(|_| "/tmp/opencode/geodesic-paths.png".to_owned());
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path))
            .observe(
                |capture: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
                    assert!(capture.event().width() >= 640);
                    assert!(capture.event().height() >= 400);
                    verify_pixels(&capture.event().image);
                    exit.write(AppExit::Success);
                },
            );
    }
    if *frames == 600 {
        exit.write(AppExit::error());
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "Test positions are bounded positive screenshot coordinates."
)]
fn verify_pixels(image: &Image) {
    let dpi = std::env::var("GEODESIC_TEST_DPI")
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(1.0);
    let pixel = |x: f32, y: f32| {
        image
            .get_color_at(
                (image.width() as f32 / 2.0 + x * dpi) as u32,
                (image.height() as f32 / 2.0 - y * dpi) as u32,
            )
            .expect("pixel")
            .to_srgba()
    };
    let is_blue = |color: Srgba| {
        color.red < 0.1
            && (color.green - 101.0 / 255.0).abs() < 0.01
            && (color.blue - 192.0 / 255.0).abs() < 0.01
    };
    // Filled disk covers the red line, and retains its five-logical-pixel radius.
    assert!(is_blue(pixel(-100.0, 50.0)));
    assert!(is_blue(pixel(-100.0, 53.0)));
    assert!(!is_blue(pixel(-100.0, 57.0)));
    // One translucent stroke and its overlap with one translucent disk.
    let line = pixel(25.0, 50.0);
    let overlap = pixel(100.0, 50.0);
    assert!(line.red > 0.9 && line.green > 0.6 && line.green < 0.85);
    assert!(overlap.green < line.green - 0.1);
    // Complex-scaled real line is present; the genuinely complex line is absent.
    let green = pixel(150.0, 20.0);
    assert!(green.green > green.red + 0.05);
    for y in 0..image.height() {
        for x in 0..image.width() {
            let color = image.get_color_at(x, y).expect("pixel").to_srgba();
            assert!(!(color.red > 0.9 && color.blue > 0.9 && color.green < 0.3));
        }
    }
}
