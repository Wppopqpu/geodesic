use bevy::prelude::*;
use bevy_egui::EguiPlugin;

use crate::canvas::{canvas_input, draw_canvas};
use crate::chrome::application_chrome;
use crate::state::{GuiState, SIDE_PANEL_WIDTH};

pub(crate) fn run() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.97, 0.97, 0.98)))
        .insert_resource(GuiState::default())
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin {
            enable_multipass_for_primary_context: false,
        })
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (draw_canvas, application_chrome, canvas_input).chain(),
        )
        .run();
}

fn setup(mut commands: Commands, mut state: ResMut<GuiState>) {
    state.algebra_width = SIDE_PANEL_WIDTH;
    state.style_width = SIDE_PANEL_WIDTH;
    let mut projection = OrthographicProjection::default_2d();
    projection.scale = 0.02;
    commands.spawn((Camera2d, Projection::Orthographic(projection)));
}
