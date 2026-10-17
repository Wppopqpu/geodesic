use bevy::prelude::*;
use bevy_egui::{EguiPlugin, EguiPrimaryContextPass, EguiStartupSet};

use crate::canvas::{canvas_input, draw_canvas};
use crate::chrome::application_chrome;
use crate::state::{GuiState, SIDE_PANEL_WIDTH, ToolbarIcons};

pub(crate) fn run() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.97, 0.97, 0.98)))
        .insert_resource(GuiState::default())
        .add_plugins(DefaultPlugins.set(AssetPlugin {
            file_path: "../../assets".into(),
            ..default()
        }))
        .add_plugins(EguiPlugin::default())
        .add_systems(
            PreStartup,
            setup_camera.before(EguiStartupSet::InitContexts),
        )
        .add_systems(Startup, setup_ui_resources)
        .add_systems(Update, (canvas_input, draw_canvas).chain())
        .add_systems(EguiPrimaryContextPass, application_chrome)
        .run();
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy systems receive resource wrappers by value."
)]
fn setup_camera(mut commands: Commands) {
    let mut projection = OrthographicProjection::default_2d();
    projection.scale = 0.02;
    commands.spawn((
        Camera2d,
        Msaa::Sample4,
        Projection::Orthographic(projection),
    ));
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy systems receive resource wrappers by value."
)]
fn setup_ui_resources(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut state: ResMut<GuiState>,
) {
    state.algebra_width = SIDE_PANEL_WIDTH;
    state.style_width = SIDE_PANEL_WIDTH;
    commands.insert_resource(ToolbarIcons {
        move_tool: asset_server.load("geogebra-toolbar/mode_move.png"),
        point: asset_server.load("geogebra-toolbar/mode_point.png"),
        segment: asset_server.load("geogebra-toolbar/mode_segment.png"),
        ray: asset_server.load("geogebra-toolbar/mode_ray.png"),
        vector: asset_server.load("geogebra-toolbar/mode_vector.png"),
        midpoint: asset_server.load("geogebra-toolbar/mode_midpoint.png"),
        perpendicular_bisector: asset_server
            .load("geogebra-toolbar/mode_perpendicularbisector.png"),
        angle_bisector: asset_server.load("geogebra-toolbar/mode_anglebisector.png"),
        line: asset_server.load("geogebra-toolbar/mode_join.png"),
        intersection: asset_server.load("geogebra-toolbar/mode_intersect.png"),
        parallel: asset_server.load("geogebra-toolbar/mode_parallelline.png"),
        perpendicular: asset_server.load("geogebra-toolbar/mode_perpendicularline.png"),
        point_on_locus: asset_server.load("geogebra-toolbar/mode_pointonobject.png"),
    });
}
