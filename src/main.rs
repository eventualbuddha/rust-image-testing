use std::env;
use std::path::{Path, PathBuf};
use std::time::Instant;

use geometry::Segment;
use image::imageops::resize;
use image::imageops::FilterType::Lanczos3;
use image::{DynamicImage, GrayImage, Rgb};
use imageproc::contours::Contour;
use imageproc::drawing::{draw_cross_mut, draw_filled_rect_mut, draw_line_segment_mut};
use imageproc::rect::Rect;
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};
use types::{BallotCardGeometry, BallotPaperSize, Size};

use crate::geometry::{segment_distance, segment_with_length};
use crate::timing_marks::{
    find_partial_timing_marks_from_candidate_rects, find_timing_mark_shapes,
};

mod geometry;
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
    let min_x = contour.points.iter().map(|p| p.x).min().unwrap();
    let max_x = contour.points.iter().map(|p| p.x).max().unwrap();
    let min_y = contour.points.iter().map(|p| p.y).min().unwrap();
    let max_y = contour.points.iter().map(|p| p.y).max().unwrap();
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
            .unwrap()
        })
        .sum::<u32>();
    let rectangular_score = error_value as f32 / contour.points.len() as f32;
    rectangular_score < 1.0
}

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

fn find_best_line_through_items(rects: &Vec<Rect>, angle: f32, tolerance: f32) -> Vec<Rect> {
    let best_rects: Vec<&Rect> = rects
        .par_iter()
        .fold_with(vec![], |best_rects, rect| {
            let mut best_rects = best_rects;

            for other_rect in rects.iter() {
                let rect_center = geometry::center_of_rect(rect);
                let other_rect_center = geometry::center_of_rect(other_rect);
                let line_angle = (other_rect_center.y - rect_center.y)
                    .atan2(other_rect_center.x - rect_center.x);

                if geometry::angle_diff(line_angle, angle) > tolerance {
                    continue;
                }

                let rects_intsersecting_line = rects
                    .iter()
                    .filter(|r| {
                        geometry::rect_intersects_line(
                            r,
                            &Segment::new(rect_center, other_rect_center),
                        )
                    })
                    .collect::<Vec<&Rect>>();

                if rects_intsersecting_line.len() > best_rects.len() {
                    best_rects = rects_intsersecting_line;
                }
            }

            best_rects
        })
        .reduce_with(|best_rects, other_best_rects| {
            if other_best_rects.len() > best_rects.len() {
                other_best_rects
            } else {
                best_rects
            }
        })
        .unwrap();

    return best_rects.iter().map(|r| **r).collect();
}

fn debug_image_path(base: &Path, label: &str) -> PathBuf {
    let mut result = PathBuf::from(base);
    result.set_file_name(format!(
        "{}_debug_{}.png",
        base.file_stem().unwrap().to_str().unwrap(),
        label
    ));
    result
}

fn process_image(image_path: &Path) {
    let img = image::open(&image_path);

    if img.is_err() {
        eprintln!("Error opening image: {}", image_path.to_str().unwrap());
        return;
    }

    let img = img.unwrap().into_luma8();
    let geometry = get_scanned_ballot_card_geometry(img.dimensions());

    if geometry.is_none() {
        println!(
            "Could not find ballot card geometry for image of size {:?}",
            img.dimensions()
        );
        return;
    }

    let geometry = geometry.unwrap();
    let img = size_image_to_fit(
        &img,
        geometry.canvas_size.width,
        geometry.canvas_size.height,
    );

    let mut contour_rects_debug_image = DynamicImage::ImageLuma8(img.clone()).into_rgb8();
    let contour_rects = find_timing_mark_shapes(&geometry, &img);

    for (i, rect) in contour_rects.iter().enumerate() {
        draw_filled_rect_mut(
            &mut contour_rects_debug_image,
            *rect,
            RAINBOW[i % RAINBOW.len()],
        );
    }

    let contour_rects_debug_image_path = debug_image_path(image_path, "contour_rects");
    contour_rects_debug_image
        .save(&contour_rects_debug_image_path)
        .unwrap();
    println!(
        "wrote contour rects debug image: {:?}",
        contour_rects_debug_image_path
    );

    let partial_timing_marks =
        find_partial_timing_marks_from_candidate_rects(&geometry, &contour_rects);

    if partial_timing_marks.is_none() {
        return;
    }
    let partial_timing_marks = partial_timing_marks.unwrap();

    let mut find_best_fit_line_debug_image = DynamicImage::ImageLuma8(img).into_rgb8();
    draw_line_segment_mut(
        &mut find_best_fit_line_debug_image,
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
        &mut find_best_fit_line_debug_image,
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
        &mut find_best_fit_line_debug_image,
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
        &mut find_best_fit_line_debug_image,
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
        draw_filled_rect_mut(&mut find_best_fit_line_debug_image, *rect, Rgb([0, 255, 0]));
    }
    for rect in &partial_timing_marks.bottom_rects {
        draw_filled_rect_mut(&mut find_best_fit_line_debug_image, *rect, Rgb([0, 0, 255]));
    }
    for rect in &partial_timing_marks.left_rects {
        draw_filled_rect_mut(&mut find_best_fit_line_debug_image, *rect, Rgb([255, 0, 0]));
    }
    for rect in &partial_timing_marks.right_rects {
        draw_filled_rect_mut(
            &mut find_best_fit_line_debug_image,
            *rect,
            Rgb([0, 255, 255]),
        );
    }

    if let Some(top_left_corner) = partial_timing_marks.top_left_rect {
        draw_filled_rect_mut(
            &mut find_best_fit_line_debug_image,
            top_left_corner,
            Rgb([255, 0, 255]),
        );
    }

    if let Some(top_right_corner) = partial_timing_marks.top_right_rect {
        draw_filled_rect_mut(
            &mut find_best_fit_line_debug_image,
            top_right_corner,
            Rgb([255, 0, 255]),
        );
    }

    if let Some(bottom_left_corner) = partial_timing_marks.bottom_left_rect {
        draw_filled_rect_mut(
            &mut find_best_fit_line_debug_image,
            bottom_left_corner,
            Rgb([255, 0, 255]),
        );
    }

    if let Some(bottom_right_corner) = partial_timing_marks.bottom_right_rect {
        draw_filled_rect_mut(
            &mut find_best_fit_line_debug_image,
            bottom_right_corner,
            Rgb([255, 0, 255]),
        );
    }

    draw_cross_mut(
        &mut find_best_fit_line_debug_image,
        Rgb([255, 255, 255]),
        partial_timing_marks.top_left_corner.x.round() as i32,
        partial_timing_marks.top_left_corner.y.round() as i32,
    );

    draw_cross_mut(
        &mut find_best_fit_line_debug_image,
        Rgb([255, 255, 255]),
        partial_timing_marks.top_right_corner.x.round() as i32,
        partial_timing_marks.top_right_corner.y.round() as i32,
    );

    draw_cross_mut(
        &mut find_best_fit_line_debug_image,
        Rgb([255, 255, 255]),
        partial_timing_marks.bottom_left_corner.x.round() as i32,
        partial_timing_marks.bottom_left_corner.y.round() as i32,
    );

    draw_cross_mut(
        &mut find_best_fit_line_debug_image,
        Rgb([255, 255, 255]),
        partial_timing_marks.bottom_right_corner.x.round() as i32,
        partial_timing_marks.bottom_right_corner.y.round() as i32,
    );

    let top_line_distance = segment_distance(
        &Segment::new(
            partial_timing_marks.top_left_corner,
            partial_timing_marks.top_right_corner,
        )
    );
    let _top_line_distance_per_segment =
        top_line_distance / ((geometry.grid_size.width - 1) as f32);
    let bottom_line_distance = segment_distance(&Segment::new(
        partial_timing_marks.bottom_left_corner,
        partial_timing_marks.bottom_right_corner,
    ));
    let _bottom_line_distance_per_segment =
        bottom_line_distance / ((geometry.grid_size.width - 1) as f32);
    for i in 0..geometry.grid_size.width {
        let expected_top_timing_mark_center = segment_with_length(
            &Segment::new(partial_timing_marks.top_left_corner, partial_timing_marks.top_right_corner),
            top_line_distance * (i as f32),
        )
        .end;

        draw_cross_mut(
            &mut find_best_fit_line_debug_image,
            Rgb([0, 127, 0]),
            expected_top_timing_mark_center.x.round() as i32,
            expected_top_timing_mark_center.y.round() as i32,
        );

        let expected_bottom_timing_mark_center = segment_with_length(
            &Segment::new(partial_timing_marks.bottom_left_corner, partial_timing_marks.bottom_right_corner),
            bottom_line_distance * (i as f32),
        )
        .end;

        draw_cross_mut(
            &mut find_best_fit_line_debug_image,
            Rgb([0, 0, 127]),
            expected_bottom_timing_mark_center.x.round() as i32,
            expected_bottom_timing_mark_center.y.round() as i32,
        );
    }

    let left_line_distance = segment_distance(&Segment::new(
        partial_timing_marks.top_left_corner, partial_timing_marks.bottom_left_corner,
    ));
    let left_line_distance_per_segment =
        left_line_distance / ((geometry.grid_size.height - 1) as f32);
    let right_line_distance = segment_distance(&Segment::new(
        partial_timing_marks.top_right_corner, partial_timing_marks.bottom_right_corner,
    ));
    let right_line_distance_per_segment =
        right_line_distance / ((geometry.grid_size.height - 1) as f32);
    for i in 0..geometry.grid_size.height {
        let expected_left_timing_mark_center = segment_with_length(
            &Segment::new(partial_timing_marks.top_left_corner, partial_timing_marks.bottom_left_corner),
            left_line_distance_per_segment * (i as f32),
        )
        .end;

        draw_cross_mut(
            &mut find_best_fit_line_debug_image,
            Rgb([127, 0, 0]),
            expected_left_timing_mark_center.x.round() as i32,
            expected_left_timing_mark_center.y.round() as i32,
        );

        let expected_right_timing_mark_center = segment_with_length(
            &Segment::new(partial_timing_marks.top_right_corner, partial_timing_marks.bottom_right_corner),
            right_line_distance_per_segment * (i as f32),
        )
        .end;

        draw_cross_mut(
            &mut find_best_fit_line_debug_image,
            Rgb([0, 127, 127]),
            expected_right_timing_mark_center.x.round() as i32,
            expected_right_timing_mark_center.y.round() as i32,
        );
    }

    let find_best_fit_line_debug_image_path = debug_image_path(image_path, "find_best_fit_line");
    find_best_fit_line_debug_image
        .save(&find_best_fit_line_debug_image_path)
        .unwrap();
    println!(
        "wrote find_best_fit_line debug image: {:?}",
        find_best_fit_line_debug_image_path
    );
}

fn main() {
    // get command line arguments
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} <image>", args[0]);
        return;
    }

    let start = Instant::now();
    args[1..].par_iter().for_each(|image_path| {
        process_image(Path::new(image_path));
    });
    println!("total time: {:?}", start.elapsed());
}
