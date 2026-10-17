use bevy::prelude::*;
use bevy_egui::EguiPlugin;

use crate::canvas::{canvas_input, draw_canvas};
use crate::chrome::application_chrome;
use crate::state::{GuiState, SIDE_PANEL_WIDTH, ToolbarIcons};

pub(crate) fn run() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.97, 0.97, 0.98)))
        .insert_resource(GuiState::default())
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin::default())
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (application_chrome, canvas_input, draw_canvas).chain(),
        )
        .run();
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy systems receive resource wrappers by value."
)]
fn setup(mut commands: Commands, asset_server: Res<AssetServer>, mut state: ResMut<GuiState>) {
    state.algebra_width = SIDE_PANEL_WIDTH;
    state.style_width = SIDE_PANEL_WIDTH;
    let mut projection = OrthographicProjection::default_2d();
    projection.scale = 0.02;
    commands.spawn((
        Camera2d,
        Msaa::Sample4,
        Projection::Orthographic(projection),
    ));
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
    });
}
