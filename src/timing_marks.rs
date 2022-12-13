use std::f32::consts::PI;

use image::GrayImage;
use imageproc::{
    contours::{find_contours_with_threshold, BorderType},
    contrast::otsu_level,
    point::Point,
    rect::Rect,
};
use logging_timer::time;

use crate::{
    geometry::{center_of_rect, find_best_line_through_items, intersection_of_lines, Segment},
    get_contour_bounding_rect, is_contour_rectangular,
    types::BallotCardGeometry,
};

/// Represents partial timing marks found in a ballot card.
pub struct PartialTimingMarks {
    pub geometry: BallotCardGeometry,
    pub top_left_corner: Point<f32>,
    pub top_right_corner: Point<f32>,
    pub bottom_left_corner: Point<f32>,
    pub bottom_right_corner: Point<f32>,
    pub top_rects: Vec<Rect>,
    pub bottom_rects: Vec<Rect>,
    pub left_rects: Vec<Rect>,
    pub right_rects: Vec<Rect>,
    pub top_left_rect: Option<Rect>,
    pub top_right_rect: Option<Rect>,
    pub bottom_left_rect: Option<Rect>,
    pub bottom_right_rect: Option<Rect>,
}

#[time]
pub fn find_timing_mark_shapes(
    geometry: &BallotCardGeometry,
    img: &GrayImage,
) -> Vec<Rect> {
    let threshold = otsu_level(img);
    let contours = find_contours_with_threshold(img, threshold);
    let contour_rects = contours
        .iter()
        .enumerate()
        .filter_map(|(i, contour)| {
            if contour.border_type == BorderType::Hole {
                let contour_bounds = get_contour_bounding_rect(contour);
                if rect_could_be_timing_mark(geometry, &contour_bounds)
                    && is_contour_rectangular(contour)
                    && contours.iter().all(|c| c.parent != Some(i))
                {
                    return Some(contour_bounds);
                }
            }
            None
        })
        .collect::<Vec<Rect>>();

    contour_rects
}

#[time]
pub fn find_partial_timing_marks_from_candidate_rects(
    geometry: &BallotCardGeometry,
    rects: &[Rect],
) -> Option<PartialTimingMarks> {
    let half_height = (geometry.canvas_size.height / 2) as i32;
    let top_half_rects = rects
        .iter()
        .filter(|r| r.top() < half_height)
        .copied()
        .collect::<Vec<Rect>>();
    let bottom_half_rects = rects
        .iter()
        .filter(|r| r.top() >= half_height)
        .copied()
        .collect::<Vec<Rect>>();
    let left_half_rects = rects
        .iter()
        .filter(|r| r.left() < half_height)
        .copied()
        .collect::<Vec<Rect>>();
    let right_half_rects = rects
        .iter()
        .filter(|r| r.left() >= half_height)
        .copied()
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
    all_distances.append(&mut distances_between_rects(&top_line));
    all_distances.append(&mut distances_between_rects(&bottom_line));
    all_distances.append(&mut distances_between_rects(&left_line));
    all_distances.append(&mut distances_between_rects(&right_line));
    all_distances.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let top_start_rect_center = center_of_rect(top_line.first().unwrap());
    let top_last_rect_center = center_of_rect(top_line.last().unwrap());
    // draw_line_segment_mut(
    //     &mut find_best_fit_line_debug_image,
    //     (top_start_rect_center.x, top_start_rect_center.y),
    //     (top_last_rect_center.x, top_last_rect_center.y),
    //     Rgb([0, 255, 0]),
    // );

    let bottom_start_rect_center = center_of_rect(bottom_line.first().unwrap());
    let bottom_last_rect_center = center_of_rect(bottom_line.last().unwrap());
    // draw_line_segment_mut(
    //     &mut find_best_fit_line_debug_image,
    //     (bottom_start_rect_center.x, bottom_start_rect_center.y),
    //     (bottom_last_rect_center.x, bottom_last_rect_center.y),
    //     Rgb([0, 0, 255]),
    // );

    let left_start_rect_center = center_of_rect(left_line.first().unwrap());
    let left_last_rect_center = center_of_rect(left_line.last().unwrap());
    // draw_line_segment_mut(
    //     &mut find_best_fit_line_debug_image,
    //     (left_start_rect_center.x, left_start_rect_center.y),
    //     (left_last_rect_center.x, left_last_rect_center.y),
    //     Rgb([255, 0, 0]),
    // );

    let right_start_rect_center = center_of_rect(right_line.first().unwrap());
    let right_last_rect_center = center_of_rect(right_line.last().unwrap());
    // draw_line_segment_mut(
    //     &mut find_best_fit_line_debug_image,
    //     (right_start_rect_center.x, right_start_rect_center.y),
    //     (right_last_rect_center.x, right_last_rect_center.y),
    //     Rgb([0, 255, 255]),
    // );

    // for rect in &top_line {
    //     draw_filled_rect_mut(&mut find_best_fit_line_debug_image, *rect, Rgb([0, 255, 0]));
    // }
    // for rect in &bottom_line {
    //     draw_filled_rect_mut(&mut find_best_fit_line_debug_image, *rect, Rgb([0, 0, 255]));
    // }
    // for rect in &left_line {
    //     draw_filled_rect_mut(&mut find_best_fit_line_debug_image, *rect, Rgb([255, 0, 0]));
    // }
    // for rect in &right_line {
    //     draw_filled_rect_mut(
    //         &mut find_best_fit_line_debug_image,
    //         *rect,
    //         Rgb([0, 255, 255]),
    //     );
    // }

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

    // if let Some(top_left_corner) = top_left_corner {
    //     draw_filled_rect_mut(
    //         &mut find_best_fit_line_debug_image,
    //         *top_left_corner,
    //         Rgb([255, 0, 255]),
    //     );
    // }

    // if let Some(top_right_corner) = top_right_corner {
    //     draw_filled_rect_mut(
    //         &mut find_best_fit_line_debug_image,
    //         *top_right_corner,
    //         Rgb([255, 0, 255]),
    //     );
    // }

    // if let Some(bottom_left_corner) = bottom_left_corner {
    //     draw_filled_rect_mut(
    //         &mut find_best_fit_line_debug_image,
    //         *bottom_left_corner,
    //         Rgb([255, 0, 255]),
    //     );
    // }

    // if let Some(bottom_right_corner) = bottom_right_corner {
    //     draw_filled_rect_mut(
    //         &mut find_best_fit_line_debug_image,
    //         *bottom_right_corner,
    //         Rgb([255, 0, 255]),
    //     );
    // }

    let top_left_intersection = intersection_of_lines(
        &Segment::new(top_start_rect_center, top_last_rect_center),
        &Segment::new(left_start_rect_center, left_last_rect_center),
        false,
    )
    .unwrap();
    // draw_cross_mut(
    //     &mut find_best_fit_line_debug_image,
    //     Rgb([255, 255, 255]),
    //     top_left_intersection.x.round() as i32,
    //     top_left_intersection.y.round() as i32,
    // );

    let top_right_intersection = intersection_of_lines(
        &Segment::new(top_start_rect_center, top_last_rect_center),
        &Segment::new(right_start_rect_center, right_last_rect_center),
        false,
    )
    .unwrap();
    // draw_cross_mut(
    //     &mut find_best_fit_line_debug_image,
    //     Rgb([255, 255, 255]),
    //     top_right_intersection.x.round() as i32,
    //     top_right_intersection.y.round() as i32,
    // );

    let bottom_left_intersection = intersection_of_lines(
        &Segment::new(bottom_start_rect_center, bottom_last_rect_center),
        &Segment::new(left_start_rect_center, left_last_rect_center),
        false,
    )
    .unwrap();
    // draw_cross_mut(
    //     &mut find_best_fit_line_debug_image,
    //     Rgb([255, 255, 255]),
    //     bottom_left_intersection.x.round() as i32,
    //     bottom_left_intersection.y.round() as i32,
    // );

    let bottom_right_intersection = intersection_of_lines(
        &Segment::new(bottom_start_rect_center, bottom_last_rect_center),
        &Segment::new(right_start_rect_center, right_last_rect_center),
        false,
    )
    .unwrap();

    Some(PartialTimingMarks {
        geometry: *geometry,
        top_left_corner: top_left_intersection,
        top_right_corner: top_right_intersection,
        bottom_left_corner: bottom_left_intersection,
        bottom_right_corner: bottom_right_intersection,
        top_left_rect: top_left_corner.copied(),
        top_right_rect: top_right_corner.copied(),
        bottom_left_rect: bottom_left_corner.copied(),
        bottom_right_rect: bottom_right_corner.copied(),
        top_rects: top_line,
        bottom_rects: bottom_line,
        left_rects: left_line,
        right_rects: right_line,
    })
}

/// Determines whether a rect could be a timing mark based on its size.
pub fn rect_could_be_timing_mark(geometry: &BallotCardGeometry, rect: &Rect) -> bool {
    let min_timing_mark_width = (geometry.timing_mark_size.width * 1.0 / 4.0).floor() as u32;
    let max_timing_mark_width = (geometry.timing_mark_size.width * 3.0 / 2.0).ceil() as u32;
    let min_timing_mark_height = (geometry.timing_mark_size.height * 2.0 / 3.0).floor() as u32;
    let max_timing_mark_height = (geometry.timing_mark_size.height * 3.0 / 2.0).ceil() as u32;
    rect.width() >= min_timing_mark_width
        && rect.width() <= max_timing_mark_width
        && rect.height() >= min_timing_mark_height
        && rect.height() <= max_timing_mark_height
}

/// Gets all the distances between adjacent rects in a list of rects.
pub fn distances_between_rects(rects: &[Rect]) -> Vec<f32> {
    let mut distances = rects
        .windows(2)
        .map(|w| Segment::new(center_of_rect(&w[1]), center_of_rect(&w[0])).length())
        .collect::<Vec<f32>>();
    distances.sort_by(|a, b| a.partial_cmp(b).expect("comparison of non-NaN to succeed"));
    distances
}
