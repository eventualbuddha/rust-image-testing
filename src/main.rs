extern crate log;
extern crate pretty_env_logger;

use std::env;
use std::path::{Path, PathBuf};

use clap::{arg, command, Command};
use image::imageops::resize;
use image::imageops::FilterType::Lanczos3;
use image::{DynamicImage, GrayImage};
use imageproc::contours::Contour;
use imageproc::rect::Rect;
use logging_timer::{finish, time, timer};
use types::{BallotCardGeometry, BallotPaperSize, BallotSide, Size};

use crate::election::Election;
use crate::metadata::{decode_metadata_from_timing_marks};
use crate::timing_marks::{
    find_complete_timing_marks_from_partial_timing_marks,
    find_partial_timing_marks_from_candidate_rects, find_timing_mark_shapes, load_oval_scan_image,
    score_oval_marks_from_grid_layout, TimingMarkGrid,
};

mod debug;
mod election;
mod geometry;
mod image_utils;
mod metadata;
mod timing_marks;
mod types;

fn get_scanned_ballot_card_geometry_8pt5x11() -> BallotCardGeometry {
    BallotCardGeometry {
        ballot_paper_size: BallotPaperSize::Letter,
        pixels_per_inch: 200,
        canvas_size: Size {
            width: 1696,
            height: 2200,
        },
        content_area: Rect::at(0, 0).of_size(1696, 2200),
        oval_size: Size {
            width: 40,
            height: 26,
        },
        timing_mark_size: Size {
            width: 37.5,
            height: 12.5,
        },
        grid_size: Size {
            width: 34,
            height: 41,
        },
        front_usable_area: Rect::at(0, 0).of_size(34, 41),
        back_usable_area: Rect::at(0, 0).of_size(34, 41),
    }
}

fn get_scanned_ballot_card_geometry_8pt5x14() -> BallotCardGeometry {
    BallotCardGeometry {
        ballot_paper_size: BallotPaperSize::Legal,
        pixels_per_inch: 200,
        canvas_size: Size {
            width: 1696,
            height: 2800,
        },
        content_area: Rect::at(0, 0).of_size(1696, 2800),
        oval_size: Size {
            width: 40,
            height: 26,
        },
        timing_mark_size: Size {
            width: 37.5,
            height: 12.5,
        },
        grid_size: Size {
            width: 34,
            height: 53,
        },
        front_usable_area: Rect::at(0, 0).of_size(34, 53),
        back_usable_area: Rect::at(0, 0).of_size(34, 53),
    }
}

fn get_scanned_ballot_card_geometry(size: (u32, u32)) -> Option<BallotCardGeometry> {
    let (width, height) = size;
    let aspect_ratio = width as f32 / height as f32;
    let letter_size = get_scanned_ballot_card_geometry_8pt5x11();
    let letter_aspect_ratio =
        letter_size.canvas_size.width as f32 / letter_size.canvas_size.height as f32;
    let legal_size = get_scanned_ballot_card_geometry_8pt5x14();
    let letgal_aspect_ratio =
        legal_size.canvas_size.width as f32 / legal_size.canvas_size.height as f32;

    if (aspect_ratio - letter_aspect_ratio).abs() < 0.01 {
        Some(letter_size)
    } else if (aspect_ratio - letgal_aspect_ratio).abs() < 0.01 {
        Some(legal_size)
    } else {
        None
    }
}

fn get_contour_bounding_rect(contour: &Contour<u32>) -> Rect {
    let min_x = contour.points.iter().map(|p| p.x).min().unwrap_or(0);
    let max_x = contour.points.iter().map(|p| p.x).max().unwrap_or(0);
    let min_y = contour.points.iter().map(|p| p.y).min().unwrap_or(0);
    let max_y = contour.points.iter().map(|p| p.y).max().unwrap_or(0);
    Rect::at(min_x as i32, min_y as i32).of_size(max_x - min_x + 1, max_y - min_y + 1)
}

fn is_contour_rectangular(contour: &Contour<u32>) -> bool {
    let rect = get_contour_bounding_rect(contour);

    let error_value = contour
        .points
        .iter()
        .map(|p| {
            [
                (p.x - rect.left() as u32),
                (p.y - rect.top() as u32),
                (rect.right() as u32 - p.x),
                (rect.bottom() as u32 - p.y),
            ]
            .into_iter()
            .min()
            .unwrap_or(0)
        })
        .sum::<u32>();
    let rectangular_score = error_value as f32 / contour.points.len() as f32;
    rectangular_score < 1.0
}

#[time]
fn size_image_to_fit(img: &GrayImage, max_width: u32, max_height: u32) -> GrayImage {
    let aspect_ratio = img.width() as f32 / img.height() as f32;
    let new_width = if aspect_ratio > 1.0 {
        max_width
    } else {
        (max_height as f32 * aspect_ratio).ceil() as u32
    };
    let new_height = if aspect_ratio > 1.0 {
        (max_width as f32 / aspect_ratio).ceil() as u32
    } else {
        max_height
    };
    resize(img, new_width, new_height, Lanczos3)
}

#[derive(Debug, Clone)]
struct ProcessImageOptions {
    debug: bool,
    oval_template: GrayImage,
    election: Election,
    side: BallotSide,
}

#[derive(Debug)]
enum ProcessImageError {
    ImageOpenError(PathBuf),
    UnexpectedDimensionsError((u32, u32)),
    DebugImageSaveError(PathBuf),
    MissingTimingMarks(Vec<Rect>),
}

#[time]
fn process_image(
    image_path: &Path,
    options: &ProcessImageOptions,
) -> Result<(), ProcessImageError> {
    let debug = options.debug;

    let timer = timer!("image::open()", "path={:?}", image_path);
    let img = match image::open(&image_path) {
        Ok(img) => img.into_luma8(),
        Err(_) => {
            return Err(ProcessImageError::ImageOpenError(image_path.to_path_buf()));
        }
    };
    finish!(timer);

    let geometry = match get_scanned_ballot_card_geometry(img.dimensions()) {
        Some(geometry) => geometry,
        None => {
            return Err(ProcessImageError::UnexpectedDimensionsError(
                img.dimensions(),
            ))
        }
    };

    let img = size_image_to_fit(
        &img,
        geometry.canvas_size.width,
        geometry.canvas_size.height,
    );

    let contour_rects = find_timing_mark_shapes(&geometry, &img);

    if debug {
        let mut contour_rects_debug_image = DynamicImage::ImageLuma8(img.clone()).into_rgb8();

        debug::draw_contour_rects_debug_image_mut(&mut contour_rects_debug_image, &contour_rects);

        let contour_rects_debug_image_path = debug::debug_image_path(image_path, "contour_rects");
        match contour_rects_debug_image.save(&contour_rects_debug_image_path) {
            Ok(_) => {
                eprintln!("DEBUG: {:?}", contour_rects_debug_image_path);
            }
            Err(_) => {
                return Err(ProcessImageError::DebugImageSaveError(
                    contour_rects_debug_image_path.to_path_buf(),
                ))
            }
        }
    }

    let partial_timing_marks =
        match find_partial_timing_marks_from_candidate_rects(&geometry, &contour_rects) {
            Some(partial_timing_marks) => partial_timing_marks,
            None => return Err(ProcessImageError::MissingTimingMarks(contour_rects)),
        };

    if debug {
        let mut find_best_fit_line_debug_image = DynamicImage::ImageLuma8(img.clone()).into_rgb8();
        debug::draw_timing_mark_debug_image_mut(
            &mut find_best_fit_line_debug_image,
            &geometry,
            &partial_timing_marks,
        );

        let find_best_fit_line_debug_image_path =
            debug::debug_image_path(image_path, "find_best_fit_line");
        match find_best_fit_line_debug_image.save(&find_best_fit_line_debug_image_path) {
            Ok(_) => {
                eprintln!("DEBUG: {:?}", find_best_fit_line_debug_image_path);
            }
            Err(_) => {
                return Err(ProcessImageError::DebugImageSaveError(
                    find_best_fit_line_debug_image_path.to_path_buf(),
                ))
            }
        }
    }

    let complete_timing_marks = match find_complete_timing_marks_from_partial_timing_marks(
        &partial_timing_marks,
        &geometry,
    ) {
        None => {
            return Err(ProcessImageError::MissingTimingMarks(contour_rects));
        }
        Some(complete_timing_marks) => {
            if debug {
                let mut debug_image = DynamicImage::ImageLuma8(img.clone()).into_rgb8();
                debug::draw_timing_mark_debug_image_mut(
                    &mut debug_image,
                    &geometry,
                    &complete_timing_marks.clone().into(),
                );

                let debug_image_path = debug::debug_image_path(image_path, "complete_timing_marks");
                match debug_image.save(&debug_image_path) {
                    Ok(_) => {
                        eprintln!("DEBUG: {:?}", debug_image_path);
                    }
                    Err(_) => {
                        return Err(ProcessImageError::DebugImageSaveError(
                            debug_image_path.to_path_buf(),
                        ))
                    }
                }
            }
            complete_timing_marks
        }
    };

    match decode_metadata_from_timing_marks(&partial_timing_marks, &complete_timing_marks) {
        Ok(metadata) => {
            eprintln!("Metadata: {:?}", metadata);
        }
        Err(err) => {
            eprintln!("Metadata error: {:?}", err);
        }
    }

    let grid = TimingMarkGrid::new(geometry, complete_timing_marks);

    if debug {
        let mut debug_image = DynamicImage::ImageLuma8(img.clone()).into_rgb8();
        debug::draw_timing_mark_grid_debug_image_mut(&mut debug_image, &grid, &geometry);

        let debug_image_path = debug::debug_image_path(image_path, "timing_mark_grid");
        match debug_image.save(&debug_image_path) {
            Ok(_) => {
                eprintln!("DEBUG: {:?}", debug_image_path);
            }
            Err(_) => {
                return Err(ProcessImageError::DebugImageSaveError(
                    debug_image_path.to_path_buf(),
                ))
            }
        }
    }

    let scored_oval_marks = score_oval_marks_from_grid_layout(
        &img,
        &options.oval_template,
        &grid,
        options
            .election
            .grid_layouts
            .first()
            .expect("no grid layouts"),
        options.side,
    );

    eprintln!("DEBUG: scored_oval_marks: {:?}", scored_oval_marks);

    if debug {
        let mut debug_image = DynamicImage::ImageLuma8(img.clone()).into_rgb8();

        debug::draw_scored_oval_marks_debug_image_mut(&mut debug_image, &scored_oval_marks);

        let debug_image_path = debug::debug_image_path(image_path, "scored_oval_mark");
        match debug_image.save(&debug_image_path) {
            Ok(_) => {
                eprintln!("DEBUG: {:?}", debug_image_path);
            }
            Err(_) => {
                return Err(ProcessImageError::DebugImageSaveError(
                    debug_image_path.to_path_buf(),
                ))
            }
        }
    }

    return Ok(());
}

fn main() {
    pretty_env_logger::init_custom_env("LOG");

    let matches = cli().get_matches();
    let debug = matches.get_flag("debug");
    let side_a_path = matches
        .get_one::<String>("side_a_path")
        .expect("side A image path");
    let side_b_path = matches
        .get_one::<String>("side_b_path")
        .expect("side B image path");
    let election_definition_path = matches
        .get_one::<String>("election")
        .expect("election path");

    // parse contents of election_definition_path with serde_json
    let election: Election = match serde_json::from_str(
        &std::fs::read_to_string(election_definition_path).expect("election file"),
    ) {
        Ok(election_definition) => election_definition,
        Err(e) => {
            panic!("Error parsing election definition: {}", e);
        }
    };

    println!("Election: {:?}", election);

    let oval_scan_image = match load_oval_scan_image() {
        Some(image) => image,
        None => {
            panic!("Error loading oval scan image");
        }
    };

    let options = ProcessImageOptions {
        debug,
        oval_template: oval_scan_image,
        election,
        side: BallotSide::Front,
    };
    let timer = timer!("total");

    rayon::join(
        || {
            let timer = timer!("side A");
            match process_image(
                Path::new(&side_a_path),
                &ProcessImageOptions {
                    side: BallotSide::Front,
                    ..options.clone()
                },
            ) {
                Ok(_) => {}
                Err(e) => {
                    eprintln!("Error processing image {}: {:?}", side_a_path, e);
                }
            };
            finish!(timer);
        },
        || {
            let timer = timer!("side B");
            match process_image(
                Path::new(&side_b_path),
                &ProcessImageOptions {
                    side: BallotSide::Back,
                    ..options.clone()
                },
            ) {
                Ok(_) => {}
                Err(e) => {
                    eprintln!("Error processing image {}: {:?}", side_b_path, e);
                }
            };
            finish!(timer);
        },
    );
    finish!(timer);
}

fn cli() -> Command {
    command!()
        .arg(arg!(-e --election <PATH> "Path to election.json file").required(true))
        .arg(arg!(-d --debug "Enable debug mode"))
        .arg(arg!(side_a_path: <SIDE_A_IMAGE> "Path to image for side A").required(true))
        .arg(arg!(side_b_path: <SIDE_B_IMAGE> "Path to image for side B").required(true))
}
