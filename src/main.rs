extern crate log;
extern crate pretty_env_logger;

use std::env;
use std::path::{Path, PathBuf};

use clap::{arg, command, Command};
use geometry::Segment;
use image::imageops::resize;
use image::imageops::FilterType::Lanczos3;
use image::{DynamicImage, GrayImage, Rgb, RgbImage};
use imageproc::contours::Contour;
use imageproc::contrast::otsu_level;
use imageproc::drawing::{
    draw_cross_mut, draw_filled_rect_mut, draw_hollow_rect_mut, draw_line_segment_mut,
};
use imageproc::point::Point;
use imageproc::rect::Rect;
use logging_timer::{finish, time, timer};
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};
use timing_marks::PartialTimingMarks;
use types::{BallotCardGeometry, BallotPaperSize, Size};

use crate::geometry::segment_with_length;
use crate::timing_marks::{
    find_complete_timing_marks_from_partial_timing_marks,
    find_partial_timing_marks_from_candidate_rects, find_timing_mark_shapes, load_oval_scan_image,
    score_oval_mark, TimingMarkGrid,
};

mod geometry;
mod image_utils;
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

const RAINBOW: [Rgb<u8>; 7] = [
    Rgb([255, 0, 0]),
    Rgb([255, 127, 0]),
    Rgb([255, 255, 0]),
    Rgb([0, 255, 0]),
    Rgb([0, 0, 255]),
    Rgb([75, 0, 130]),
    Rgb([143, 0, 255]),
];

fn debug_image_path(base: &Path, label: &str) -> PathBuf {
    let mut result = PathBuf::from(base);
    result.set_file_name(format!(
        "{}_debug_{}.png",
        base.file_stem().unwrap_or_default().to_str().unwrap(),
        label
    ));
    result
}

struct ProcessImageOptions {
    debug: bool,
    oval_template: GrayImage,
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

        draw_contour_rects_debug_image_mut(&mut contour_rects_debug_image, &contour_rects);

        let contour_rects_debug_image_path = debug_image_path(image_path, "contour_rects");
        match contour_rects_debug_image.save(&contour_rects_debug_image_path) {
            Ok(_) => {
                println!("DEBUG: {:?}", contour_rects_debug_image_path);
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
        draw_best_fit_line_debug_image_mut(
            &mut find_best_fit_line_debug_image,
            &geometry,
            &partial_timing_marks,
        );

        let find_best_fit_line_debug_image_path =
            debug_image_path(image_path, "find_best_fit_line");
        match find_best_fit_line_debug_image.save(&find_best_fit_line_debug_image_path) {
            Ok(_) => {
                println!("DEBUG: {:?}", find_best_fit_line_debug_image_path);
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
                draw_best_fit_line_debug_image_mut(
                    &mut debug_image,
                    &geometry,
                    &PartialTimingMarks {
                        geometry: complete_timing_marks.geometry,
                        top_left_corner: complete_timing_marks.top_left_corner,
                        top_right_corner: complete_timing_marks.top_right_corner,
                        bottom_left_corner: complete_timing_marks.bottom_left_corner,
                        bottom_right_corner: complete_timing_marks.bottom_right_corner,
                        top_rects: complete_timing_marks.top_rects.clone(),
                        bottom_rects: complete_timing_marks.bottom_rects.clone(),
                        left_rects: complete_timing_marks.left_rects.clone(),
                        right_rects: complete_timing_marks.right_rects.clone(),
                        top_left_rect: Some(complete_timing_marks.top_left_rect),
                        top_right_rect: Some(complete_timing_marks.top_right_rect),
                        bottom_left_rect: Some(complete_timing_marks.bottom_left_rect),
                        bottom_right_rect: Some(complete_timing_marks.bottom_right_rect),
                    },
                );

                let debug_image_path = debug_image_path(image_path, "complete_timing_marks");
                match debug_image.save(&debug_image_path) {
                    Ok(_) => {
                        println!("DEBUG: {:?}", debug_image_path);
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

    let grid = TimingMarkGrid::new(geometry, complete_timing_marks);

    if debug {
        let mut debug_image = DynamicImage::ImageLuma8(img.clone()).into_rgb8();
        draw_timing_mark_grid_debug_image_mut(&mut debug_image, &grid, &geometry);

        let debug_image_path = debug_image_path(image_path, "timing_mark_grid");
        match debug_image.save(&debug_image_path) {
            Ok(_) => {
                println!("DEBUG: {:?}", debug_image_path);
            }
            Err(_) => {
                return Err(ProcessImageError::DebugImageSaveError(
                    debug_image_path.to_path_buf(),
                ))
            }
        }
    }

    if let Some(scored_oval_mark) = score_oval_mark(
        &img,
        &options.oval_template,
        &grid,
        &Point::new(19, 9),
        7,
        otsu_level(&img),
    ) {
        println!("Scored oval mark: {:?}", scored_oval_mark);

        if debug {
            let mut debug_image = DynamicImage::ImageLuma8(img.clone()).into_rgb8();

            // draw rect around the oval mark
            let bounds = scored_oval_mark.bounds;
            draw_hollow_rect_mut(&mut debug_image, bounds, Rgb([u8::MAX, u8::MIN, u8::MIN]));

            let debug_image_path = debug_image_path(image_path, "scored_oval_mark");
            match debug_image.save(&debug_image_path) {
                Ok(_) => {
                    println!("DEBUG: {:?}", debug_image_path);
                }
                Err(_) => {
                    return Err(ProcessImageError::DebugImageSaveError(
                        debug_image_path.to_path_buf(),
                    ))
                }
            }
        }
    }

    return Ok(());
}

fn draw_contour_rects_debug_image_mut(canvas: &mut RgbImage, contour_rects: &Vec<Rect>) {
    for (i, rect) in contour_rects.iter().enumerate() {
        draw_filled_rect_mut(canvas, *rect, RAINBOW[i % RAINBOW.len()]);
    }
}

fn draw_best_fit_line_debug_image_mut(
    canvas: &mut RgbImage,
    geometry: &BallotCardGeometry,
    partial_timing_marks: &PartialTimingMarks,
) {
    draw_line_segment_mut(
        canvas,
        (
            partial_timing_marks.top_left_corner.x,
            partial_timing_marks.top_left_corner.y,
        ),
        (
            partial_timing_marks.top_right_corner.x,
            partial_timing_marks.top_right_corner.y,
        ),
        Rgb([0, 255, 0]),
    );

    draw_line_segment_mut(
        canvas,
        (
            partial_timing_marks.bottom_left_corner.x,
            partial_timing_marks.bottom_left_corner.y,
        ),
        (
            partial_timing_marks.bottom_right_corner.x,
            partial_timing_marks.bottom_right_corner.y,
        ),
        Rgb([0, 0, 255]),
    );

    draw_line_segment_mut(
        canvas,
        (
            partial_timing_marks.top_left_corner.x,
            partial_timing_marks.top_left_corner.y,
        ),
        (
            partial_timing_marks.bottom_left_corner.x,
            partial_timing_marks.bottom_left_corner.y,
        ),
        Rgb([255, 0, 0]),
    );

    draw_line_segment_mut(
        canvas,
        (
            partial_timing_marks.top_right_corner.x,
            partial_timing_marks.top_right_corner.y,
        ),
        (
            partial_timing_marks.bottom_right_corner.x,
            partial_timing_marks.bottom_right_corner.y,
        ),
        Rgb([0, 255, 255]),
    );

    for rect in &partial_timing_marks.top_rects {
        draw_filled_rect_mut(canvas, *rect, Rgb([0, 255, 0]));
    }
    for rect in &partial_timing_marks.bottom_rects {
        draw_filled_rect_mut(canvas, *rect, Rgb([0, 0, 255]));
    }
    for rect in &partial_timing_marks.left_rects {
        draw_filled_rect_mut(canvas, *rect, Rgb([255, 0, 0]));
    }
    for rect in &partial_timing_marks.right_rects {
        draw_filled_rect_mut(canvas, *rect, Rgb([0, 255, 255]));
    }

    if let Some(top_left_corner) = partial_timing_marks.top_left_rect {
        draw_filled_rect_mut(canvas, top_left_corner, Rgb([255, 0, 255]));
    }

    if let Some(top_right_corner) = partial_timing_marks.top_right_rect {
        draw_filled_rect_mut(canvas, top_right_corner, Rgb([255, 0, 255]));
    }

    if let Some(bottom_left_corner) = partial_timing_marks.bottom_left_rect {
        draw_filled_rect_mut(canvas, bottom_left_corner, Rgb([255, 0, 255]));
    }

    if let Some(bottom_right_corner) = partial_timing_marks.bottom_right_rect {
        draw_filled_rect_mut(canvas, bottom_right_corner, Rgb([255, 0, 255]));
    }

    draw_cross_mut(
        canvas,
        Rgb([255, 255, 255]),
        partial_timing_marks.top_left_corner.x.round() as i32,
        partial_timing_marks.top_left_corner.y.round() as i32,
    );

    draw_cross_mut(
        canvas,
        Rgb([255, 255, 255]),
        partial_timing_marks.top_right_corner.x.round() as i32,
        partial_timing_marks.top_right_corner.y.round() as i32,
    );

    draw_cross_mut(
        canvas,
        Rgb([255, 255, 255]),
        partial_timing_marks.bottom_left_corner.x.round() as i32,
        partial_timing_marks.bottom_left_corner.y.round() as i32,
    );

    draw_cross_mut(
        canvas,
        Rgb([255, 255, 255]),
        partial_timing_marks.bottom_right_corner.x.round() as i32,
        partial_timing_marks.bottom_right_corner.y.round() as i32,
    );

    let top_line_distance = Segment::new(
        partial_timing_marks.top_left_corner,
        partial_timing_marks.top_right_corner,
    )
    .length();
    let _top_line_distance_per_segment =
        top_line_distance / ((geometry.grid_size.width - 1) as f32);
    let bottom_line_distance = Segment::new(
        partial_timing_marks.bottom_left_corner,
        partial_timing_marks.bottom_right_corner,
    )
    .length();
    let _bottom_line_distance_per_segment =
        bottom_line_distance / ((geometry.grid_size.width - 1) as f32);
    for i in 0..geometry.grid_size.width {
        let expected_top_timing_mark_center = segment_with_length(
            &Segment::new(
                partial_timing_marks.top_left_corner,
                partial_timing_marks.top_right_corner,
            ),
            top_line_distance * (i as f32),
        )
        .end;

        draw_cross_mut(
            canvas,
            Rgb([0, 127, 0]),
            expected_top_timing_mark_center.x.round() as i32,
            expected_top_timing_mark_center.y.round() as i32,
        );

        let expected_bottom_timing_mark_center = segment_with_length(
            &Segment::new(
                partial_timing_marks.bottom_left_corner,
                partial_timing_marks.bottom_right_corner,
            ),
            bottom_line_distance * (i as f32),
        )
        .end;

        draw_cross_mut(
            canvas,
            Rgb([0, 0, 127]),
            expected_bottom_timing_mark_center.x.round() as i32,
            expected_bottom_timing_mark_center.y.round() as i32,
        );
    }

    let left_line_distance = Segment::new(
        partial_timing_marks.top_left_corner,
        partial_timing_marks.bottom_left_corner,
    )
    .length();
    let left_line_distance_per_segment =
        left_line_distance / ((geometry.grid_size.height - 1) as f32);
    let right_line_distance = Segment::new(
        partial_timing_marks.top_right_corner,
        partial_timing_marks.bottom_right_corner,
    )
    .length();
    let right_line_distance_per_segment =
        right_line_distance / ((geometry.grid_size.height - 1) as f32);
    for i in 0..geometry.grid_size.height {
        let expected_left_timing_mark_center = segment_with_length(
            &Segment::new(
                partial_timing_marks.top_left_corner,
                partial_timing_marks.bottom_left_corner,
            ),
            left_line_distance_per_segment * (i as f32),
        )
        .end;

        draw_cross_mut(
            canvas,
            Rgb([127, 0, 0]),
            expected_left_timing_mark_center.x.round() as i32,
            expected_left_timing_mark_center.y.round() as i32,
        );

        let expected_right_timing_mark_center = segment_with_length(
            &Segment::new(
                partial_timing_marks.top_right_corner,
                partial_timing_marks.bottom_right_corner,
            ),
            right_line_distance_per_segment * (i as f32),
        )
        .end;

        draw_cross_mut(
            canvas,
            Rgb([0, 127, 127]),
            expected_right_timing_mark_center.x.round() as i32,
            expected_right_timing_mark_center.y.round() as i32,
        );
    }
}

fn draw_timing_mark_grid_debug_image_mut(
    canvas: &mut RgbImage,
    timing_mark_grid: &TimingMarkGrid,
    geometry: &BallotCardGeometry,
) {
    for x in 0..geometry.grid_size.width {
        for y in 0..geometry.grid_size.height {
            let point = timing_mark_grid.get(x, y).expect("grid point is defined");
            draw_cross_mut(
                canvas,
                Rgb([255, 0, 255]),
                point.x.round() as i32,
                point.y.round() as i32,
            );
        }
    }
}

fn main() {
    pretty_env_logger::init_custom_env("LOG");

    let matches = cli().get_matches();
    let debug = matches.get_flag("debug");
    let images: Vec<&String> = matches
        .get_many::<String>("image")
        .unwrap_or_default()
        .collect();

    let oval_scan_image = match load_oval_scan_image() {
        Some(image) => image,
        None => {
            panic!("Error loading oval scan image");
        }
    };

    let options = ProcessImageOptions {
        debug,
        oval_template: oval_scan_image,
    };
    let timer = timer!("total");
    images.par_iter().for_each(
        |image_path| match process_image(Path::new(image_path), &options) {
            Ok(_) => {}
            Err(e) => {
                eprintln!("Error processing image {}: {:?}", image_path, e);
            }
        },
    );
    finish!(timer);
}

fn cli() -> Command {
    command!()
        .arg(arg!(-d --debug "Enable debug mode"))
        .arg(arg!([image]... "Images to process"))
}
