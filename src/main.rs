use std::env;
use std::f32::consts::PI;
use std::path::Path;
use std::time::Instant;

use image::imageops::resize;
use image::imageops::FilterType::Lanczos3;
use image::{DynamicImage, GrayImage, Rgb};
use imageproc::contours::{find_contours_with_threshold, BorderType, Contour};
use imageproc::drawing::{draw_cross_mut, draw_filled_rect_mut, draw_line_segment_mut};
use imageproc::{contrast::otsu_level, rect::Rect};
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};
use types::{BallotCardGeometry, BallotPaperSize, Size};

use crate::timing_marks::rect_could_be_timing_mark;

mod geometry;
mod timing_marks;
mod types;

fn get_scanned_ballot_card_geometry_8pt5x11() -> BallotCardGeometry {
    return BallotCardGeometry {
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
    };
}

fn get_scanned_ballot_card_geometry_8pt5x14() -> BallotCardGeometry {
    return BallotCardGeometry {
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
    };
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
    return rectangular_score < 1.0;
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
    return resize(img, new_width, new_height, Lanczos3);
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
                    .filter(|r| geometry::rect_intersects_line(r, &(rect_center, other_rect_center)))
                    .collect::<Vec<&Rect>>();

                if rects_intsersecting_line.len() > best_rects.len() {
                    best_rects = rects_intsersecting_line;
                }
            }

            return best_rects;
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

    // let rects = vertical_scan_for_timing_marks(&img, &geometry);
    // let mut vertical_scan_debug_image = DynamicImage::ImageLuma8(img.clone()).into_rgb8();

    // for (i, rect) in rects.iter().enumerate() {
    //     if rect_could_be_timing_mark(&geometry, rect) {
    //         draw_filled_rect_mut(
    //             &mut vertical_scan_debug_image,
    //             *rect,
    //             RAINBOW[i % RAINBOW.len()],
    //         );
    //     }
    // }

    // vertical_scan_debug_image
    //     .save("vertical_scan_debug_image.png")
    //     .unwrap();

    let threshold = otsu_level(&img);
    let mut contour_image = DynamicImage::ImageLuma8(img.clone()).into_rgb8();
    let start = Instant::now();
    let contours = find_contours_with_threshold(&img, threshold);
    for (i, contour) in contours.iter().enumerate() {
        if contour.border_type == BorderType::Hole {
            let contour_bounds = get_contour_bounding_rect(&contour);
            if timing_marks::rect_could_be_timing_mark(&geometry, &contour_bounds)
                && is_contour_rectangular(&contour)
                && contours.iter().all(|c| c.parent != Some(i))
            {
                contour.points.iter().for_each(|point| {
                    contour_image.put_pixel(point.x, point.y, Rgb([255u8, 0u8, 0u8]));
                });
            }
        }
    }
    println!("contour time: {:?}", start.elapsed());
    contour_image
        .save(format!(
            "{}_contours.png",
            image_path.file_stem().unwrap().to_str().unwrap()
        ))
        .unwrap();

    let start = Instant::now();
    let contour_rects = contours
        .iter()
        .enumerate()
        .flat_map(|(i, contour)| {
            if contour.border_type == BorderType::Hole {
                let contour_bounds = get_contour_bounding_rect(&contour);
                if rect_could_be_timing_mark(&geometry, &contour_bounds)
                    && is_contour_rectangular(&contour)
                    && contours.iter().all(|c| c.parent != Some(i))
                {
                    return Some(contour_bounds);
                }
            }
            None
        })
        .collect::<Vec<Rect>>();

    let mut contour_rects_debug_image = DynamicImage::ImageLuma8(img.clone()).into_rgb8();

    for (i, rect) in contour_rects.iter().enumerate() {
        draw_filled_rect_mut(
            &mut contour_rects_debug_image,
            *rect,
            RAINBOW[i % RAINBOW.len()],
        );
    }

    contour_rects_debug_image
        .save(format!(
            "{}_contour_rects.png",
            image_path.file_stem().unwrap().to_str().unwrap()
        ))
        .unwrap();

    let mut find_best_fit_line_debug_image = DynamicImage::ImageLuma8(img.clone()).into_rgb8();
    let half_height = (geometry.canvas_size.height / 2) as i32;
    let top_half_rects = contour_rects
        .iter()
        .filter(|r| r.top() < half_height)
        .map(|r| *r)
        .collect::<Vec<Rect>>();
    let bottom_half_rects = contour_rects
        .iter()
        .filter(|r| r.top() >= half_height)
        .map(|r| *r)
        .collect::<Vec<Rect>>();
    let left_half_rects = contour_rects
        .iter()
        .filter(|r| r.left() < half_height)
        .map(|r| *r)
        .collect::<Vec<Rect>>();
    let right_half_rects = contour_rects
        .iter()
        .filter(|r| r.left() >= half_height)
        .map(|r| *r)
        .collect::<Vec<Rect>>();
    let mut top_line = find_best_line_through_items(&top_half_rects, 0.0, 5.0_f32.to_radians());
    let mut bottom_line =
        find_best_line_through_items(&bottom_half_rects, 0.0, 5.0_f32.to_radians());
    let mut left_line =
        find_best_line_through_items(&left_half_rects, PI / 2.0, 5.0_f32.to_radians());
    let mut right_line =
        find_best_line_through_items(&right_half_rects, PI / 2.0, 5.0_f32.to_radians());

    top_line.sort_by(|a, b| a.left().partial_cmp(&b.left()).unwrap());
    bottom_line.sort_by(|a, b| a.left().partial_cmp(&b.left()).unwrap());
    left_line.sort_by(|a, b| a.top().partial_cmp(&b.top()).unwrap());
    right_line.sort_by(|a, b| a.top().partial_cmp(&b.top()).unwrap());

    let mut all_distances = vec![];
    all_distances.append(&mut geometry::distances_between_rects(&top_line));
    all_distances.append(&mut geometry::distances_between_rects(&bottom_line));
    all_distances.append(&mut geometry::distances_between_rects(&left_line));
    all_distances.append(&mut geometry::distances_between_rects(&right_line));
    all_distances.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median_distance = all_distances[all_distances.len() / 2];

    println!("median distance: {}", median_distance);
    println!("find best fit line time: {:?}", start.elapsed());

    let top_start_rect_center = geometry::center_of_rect(top_line.first().unwrap());
    let top_last_rect_center = geometry::center_of_rect(top_line.last().unwrap());
    draw_line_segment_mut(
        &mut find_best_fit_line_debug_image,
        (top_start_rect_center.x, top_start_rect_center.y),
        (top_last_rect_center.x, top_last_rect_center.y),
        Rgb([0, 255, 0]),
    );

    let bottom_start_rect_center = geometry::center_of_rect(bottom_line.first().unwrap());
    let bottom_last_rect_center = geometry::center_of_rect(bottom_line.last().unwrap());
    draw_line_segment_mut(
        &mut find_best_fit_line_debug_image,
        (bottom_start_rect_center.x, bottom_start_rect_center.y),
        (bottom_last_rect_center.x, bottom_last_rect_center.y),
        Rgb([0, 0, 255]),
    );

    let left_start_rect_center = geometry::center_of_rect(left_line.first().unwrap());
    let left_last_rect_center = geometry::center_of_rect(left_line.last().unwrap());
    draw_line_segment_mut(
        &mut find_best_fit_line_debug_image,
        (left_start_rect_center.x, left_start_rect_center.y),
        (left_last_rect_center.x, left_last_rect_center.y),
        Rgb([255, 0, 0]),
    );

    let right_start_rect_center = geometry::center_of_rect(right_line.first().unwrap());
    let right_last_rect_center = geometry::center_of_rect(right_line.last().unwrap());
    draw_line_segment_mut(
        &mut find_best_fit_line_debug_image,
        (right_start_rect_center.x, right_start_rect_center.y),
        (right_last_rect_center.x, right_last_rect_center.y),
        Rgb([0, 255, 255]),
    );

    for rect in &top_line {
        draw_filled_rect_mut(&mut find_best_fit_line_debug_image, *rect, Rgb([0, 255, 0]));
    }
    for rect in &bottom_line {
        draw_filled_rect_mut(&mut find_best_fit_line_debug_image, *rect, Rgb([0, 0, 255]));
    }
    for rect in &left_line {
        draw_filled_rect_mut(&mut find_best_fit_line_debug_image, *rect, Rgb([255, 0, 0]));
    }
    for rect in &right_line {
        draw_filled_rect_mut(
            &mut find_best_fit_line_debug_image,
            *rect,
            Rgb([0, 255, 255]),
        );
    }

    let top_left_corner = if top_line.first() == left_line.first() {
        top_line.first()
    } else {
        None
    };
    let top_right_corner = if top_line.last() == right_line.first() {
        top_line.last()
    } else {
        None
    };
    let bottom_left_corner = if bottom_line.first() == left_line.last() {
        bottom_line.first()
    } else {
        None
    };
    let bottom_right_corner = if bottom_line.last() == right_line.last() {
        bottom_line.last()
    } else {
        None
    };

    if let Some(top_left_corner) = top_left_corner {
        draw_filled_rect_mut(
            &mut find_best_fit_line_debug_image,
            *top_left_corner,
            Rgb([255, 0, 255]),
        );
    }

    if let Some(top_right_corner) = top_right_corner {
        draw_filled_rect_mut(
            &mut find_best_fit_line_debug_image,
            *top_right_corner,
            Rgb([255, 0, 255]),
        );
    }

    if let Some(bottom_left_corner) = bottom_left_corner {
        draw_filled_rect_mut(
            &mut find_best_fit_line_debug_image,
            *bottom_left_corner,
            Rgb([255, 0, 255]),
        );
    }

    if let Some(bottom_right_corner) = bottom_right_corner {
        draw_filled_rect_mut(
            &mut find_best_fit_line_debug_image,
            *bottom_right_corner,
            Rgb([255, 0, 255]),
        );
    }

    let top_left_intersection = geometry::intersection_of_lines(
        &(top_start_rect_center, top_last_rect_center),
        &(left_start_rect_center, left_last_rect_center),
        false,
    )
    .unwrap();
    draw_cross_mut(
        &mut find_best_fit_line_debug_image,
        Rgb([255, 255, 255]),
        top_left_intersection.x.round() as i32,
        top_left_intersection.y.round() as i32,
    );

    let top_right_intersection = geometry::intersection_of_lines(
        &(top_start_rect_center, top_last_rect_center),
        &(right_start_rect_center, right_last_rect_center),
        false,
    )
    .unwrap();
    draw_cross_mut(
        &mut find_best_fit_line_debug_image,
        Rgb([255, 255, 255]),
        top_right_intersection.x.round() as i32,
        top_right_intersection.y.round() as i32,
    );

    if let Some(bottom_left_intersection) = geometry::intersection_of_lines(
        &(bottom_start_rect_center, bottom_last_rect_center),
        &(left_start_rect_center, left_last_rect_center),
        false,
    ) {
        draw_cross_mut(
            &mut find_best_fit_line_debug_image,
            Rgb([255, 255, 255]),
            bottom_left_intersection.x.round() as i32,
            bottom_left_intersection.y.round() as i32,
        );
    }

    if let Some(bottom_right_intersection) = geometry::intersection_of_lines(
        &(bottom_start_rect_center, bottom_last_rect_center),
        &(right_start_rect_center, right_last_rect_center),
        false,
    ) {
        draw_cross_mut(
            &mut find_best_fit_line_debug_image,
            Rgb([255, 255, 255]),
            bottom_right_intersection.x.round() as i32,
            bottom_right_intersection.y.round() as i32,
        );
    }

    find_best_fit_line_debug_image
        .save(image_path.with_file_name(format!(
            "{}_debug_find_best_fit_line.png",
            image_path.file_stem().unwrap().to_str().unwrap()
        )))
        .unwrap();
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
