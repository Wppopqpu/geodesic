use steel::{
    SteelErr,
    steel_vm::{engine::Engine, register_fn::RegisterFn},
};

use crate::ScriptError;

pub(super) fn validate_with_steel(source: &str) -> Result<(), ScriptError> {
    fn geo_point(_: String, _: f64, _: f64) {}
    fn geo_point_complex(_: String, _: f64, _: f64, _: f64, _: f64, _: f64, _: f64) {}
    fn geo_relation(_: String, _: String, _: String) {}
    fn geo_move_line(_: String, _: f64, _: f64) {}
    let mut engine = Engine::new();
    engine.register_fn("geo::point", geo_point);
    engine.register_fn("geo::point_complex", geo_point_complex);
    engine.register_fn("geo::line_through", geo_relation);
    engine.register_fn("geo::intersection", geo_relation);
    engine.register_fn("geo::parallel_through", geo_relation);
    engine.register_fn("geo::perpendicular_through", geo_relation);
    engine.register_fn("geo::move_line", geo_move_line);
    engine
        .compile_and_run_raw_program(source.to_owned())
        .map(|_| ())
        .map_err(|error| steel_error(&error))
}

fn steel_error(error: &SteelErr) -> ScriptError {
    ScriptError::Steel(error.to_string())
}
