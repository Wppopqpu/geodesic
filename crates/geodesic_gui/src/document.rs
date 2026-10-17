//! Native project document file operations for the desktop adapter.

use std::{
    fs,
    path::{Path, PathBuf},
};

use geodesic_io::Project;

pub(crate) const PROJECT_EXTENSION: &str = "geodesic.lisp";
pub(crate) const DEFAULT_PROJECT_FILE_NAME: &str = "untitled.geodesic.lisp";

pub(crate) fn native_project_path(path: &Path) -> PathBuf {
    if path
        .extension()
        .is_some_and(|extension| extension == "lisp")
    {
        path.to_owned()
    } else {
        path.with_extension(PROJECT_EXTENSION)
    }
}

pub(crate) fn load_project(path: &Path) -> Result<(Project, String), String> {
    if path.extension().is_some_and(|extension| extension == "ggb") {
        return fs::read(path)
            .map_err(|error| format!("could not read {}: {error}", path.display()))
            .and_then(|bytes| {
                geodesic_io::import_ggb(&bytes)
                    .map(|(project, report)| {
                        let message = format!(
                            "imported {} object(s); {} compatibility placeholder(s)",
                            report.converted,
                            report.placeholders.len()
                        );
                        (project, message)
                    })
                    .map_err(|error| format!("could not import {}: {error}", path.display()))
            });
    }
    fs::read_to_string(path)
        .map_err(|error| format!("could not read {}: {error}", path.display()))
        .and_then(|source| {
            geodesic_io::load(&source)
                .map(|project| (project, format!("opened {}", path.display())))
                .map_err(|error| format!("could not open {}: {error}", path.display()))
        })
}

pub(crate) fn save_project(path: &Path, project: &Project) -> Result<(), String> {
    fs::write(path, geodesic_io::save(project))
        .map_err(|error| format!("could not save {}: {error}", path.display()))
}

pub(crate) fn export_svg(path: &Path, project: &Project) -> Result<(), String> {
    geodesic_svg::render_project(project)
        .map_err(|error| format!("could not export SVG: {error}"))
        .and_then(|svg| {
            fs::write(path, svg)
                .map_err(|error| format!("could not write {}: {error}", path.display()))
        })
}

#[cfg(test)]
mod tests {
    use super::{PROJECT_EXTENSION, export_svg, load_project, native_project_path, save_project};
    use geodesic_io::Project;
    use std::{fs, path::PathBuf};

    #[test]
    fn native_projects_use_the_compound_lisp_extension() {
        assert_eq!(
            native_project_path(&PathBuf::from("diagram")),
            PathBuf::from("diagram.geodesic.lisp")
        );
        assert_eq!(
            native_project_path(&PathBuf::from("diagram.geodesic.lisp")),
            PathBuf::from("diagram.geodesic.lisp")
        );
        assert_eq!(PROJECT_EXTENSION, "geodesic.lisp");
    }

    #[test]
    fn native_files_round_trip_and_export_svg() {
        let directory = PathBuf::from(format!(
            "/tmp/opencode/geodesic-document-{}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).expect("test directory");
        let project_path = directory.join("diagram.geodesic.lisp");
        let svg_path = directory.join("diagram.svg");
        let project = Project::default();

        save_project(&project_path, &project).expect("save");
        let (loaded, _) = load_project(&project_path).expect("load");
        assert_eq!(loaded, project);
        export_svg(&svg_path, &project).expect("export");
        assert!(
            fs::read_to_string(&svg_path)
                .expect("svg")
                .starts_with("<svg")
        );

        fs::remove_dir_all(directory).expect("clean test directory");
    }
}
