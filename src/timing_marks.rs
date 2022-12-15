use std::{
    f32::consts::PI,
    fmt::{Display, Formatter},
    io,
};

use image::{GenericImageView, GrayImage, Luma};
use imageproc::{
    contours::{find_contours_with_threshold, BorderType},
    contrast::{otsu_level, threshold},
    point::Point,
    rect::Rect,
};
use logging_timer::time;

use crate::{
    election::{GridLayout, GridLocation, GridPosition},
    geometry::{center_of_rect, find_best_line_through_items, intersection_of_lines, Segment},
    get_contour_bounding_rect,
    image_utils::{bleed, diff, ratio, BLACK, WHITE},
    is_contour_rectangular,
    types::{BallotCardGeometry, BallotSide},
};

/// Represents partial timing marks found in a ballot card.
#[derive(Debug, Clone)]
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

impl From<CompleteTimingMarks> for PartialTimingMarks {
    fn from(complete_timing_marks: CompleteTimingMarks) -> Self {
        Self {
            geometry: complete_timing_marks.geometry,
            top_left_corner: complete_timing_marks.top_left_corner,
            top_right_corner: complete_timing_marks.top_right_corner,
            bottom_left_corner: complete_timing_marks.bottom_left_corner,
            bottom_right_corner: complete_timing_marks.bottom_right_corner,
            top_rects: complete_timing_marks.top_rects,
            bottom_rects: complete_timing_marks.bottom_rects,
            left_rects: complete_timing_marks.left_rects,
            right_rects: complete_timing_marks.right_rects,
            top_left_rect: Some(complete_timing_marks.top_left_rect),
            top_right_rect: Some(complete_timing_marks.top_right_rect),
            bottom_left_rect: Some(complete_timing_marks.bottom_left_rect),
            bottom_right_rect: Some(complete_timing_marks.bottom_right_rect),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CompleteTimingMarks {
    pub geometry: BallotCardGeometry,
    pub top_left_corner: Point<f32>,
    pub top_right_corner: Point<f32>,
    pub bottom_left_corner: Point<f32>,
    pub bottom_right_corner: Point<f32>,
    pub top_rects: Vec<Rect>,
    pub bottom_rects: Vec<Rect>,
    pub left_rects: Vec<Rect>,
    pub right_rects: Vec<Rect>,
    pub top_left_rect: Rect,
    pub top_right_rect: Rect,
    pub bottom_left_rect: Rect,
    pub bottom_right_rect: Rect,
}

/// Represents a grid of timing marks and provides access to the location of
/// ovals in the grid.
pub struct TimingMarkGrid {
    geometry: BallotCardGeometry,
    complete_timing_marks: CompleteTimingMarks,
}

impl TimingMarkGrid {
    pub fn new(geometry: BallotCardGeometry, complete_timing_marks: CompleteTimingMarks) -> Self {
        Self {
            geometry,
            complete_timing_marks,
        }
    }

    /// Returns the center of the grid position at the given coordinates. Timing
    /// marks are at the edges of the grid, and the inside of the grid is where
    /// the ovals are.
    ///
    /// For example, if the grid is 34x51, then:
    ///
    ///   - (0, 0) is the top left corner of the grid
    ///   - (33, 0) is the top right corner of the grid
    ///   - (0, 50) is the bottom left corner of the grid
    ///   - (33, 50) is the bottom right corner of the grid
    ///   - (c, r) where 0 < c < 33 and 0 < r < 50 is the oval at column c and
    ///     row r
    pub fn get(&self, column: u32, row: u32) -> Option<Point<f32>> {
        if column >= self.geometry.grid_size.width || row >= self.geometry.grid_size.height {
            return None;
        }

        let left = self.complete_timing_marks.left_rects.get(row as usize)?;
        let right = self.complete_timing_marks.right_rects.get(row as usize)?;
        let top = self.complete_timing_marks.top_rects.get(column as usize)?;
        let bottom = self
            .complete_timing_marks
            .bottom_rects
            .get(column as usize)?;
        let horizontal_segment = Segment::new(center_of_rect(left), center_of_rect(right));
        let vertical_segment = Segment::new(center_of_rect(top), center_of_rect(bottom));

        intersection_of_lines(&horizontal_segment, &vertical_segment, false)
    }
}

#[time]
pub fn find_timing_mark_shapes(geometry: &BallotCardGeometry, img: &GrayImage) -> Vec<Rect> {
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

#[time]
pub fn find_complete_timing_marks_from_partial_timing_marks(
    partial_timing_marks: &PartialTimingMarks,
    geometry: &BallotCardGeometry,
) -> Option<CompleteTimingMarks> {
    let top_line = &partial_timing_marks.top_rects;
    let bottom_line = &partial_timing_marks.bottom_rects;
    let left_line = &partial_timing_marks.left_rects;
    let right_line = &partial_timing_marks.right_rects;
    let (top_left_rect, top_right_rect, bottom_left_rect, bottom_right_rect) = match (
        &partial_timing_marks.top_left_rect,
        &partial_timing_marks.top_right_rect,
        &partial_timing_marks.bottom_left_rect,
        &partial_timing_marks.bottom_right_rect,
    ) {
        (
            Some(top_left_rect),
            Some(top_right_rect),
            Some(bottom_left_rect),
            Some(bottom_right_rect),
        ) => (
            top_left_rect,
            top_right_rect,
            bottom_left_rect,
            bottom_right_rect,
        ),
        _ => return None,
    };

    let mut all_distances = vec![];
    all_distances.append(&mut distances_between_rects(&top_line));
    all_distances.append(&mut distances_between_rects(&bottom_line));
    all_distances.append(&mut distances_between_rects(&left_line));
    all_distances.append(&mut distances_between_rects(&right_line));
    all_distances.sort_by(|a, b| a.partial_cmp(b).unwrap());

    if all_distances.is_empty() {
        return None;
    }

    let median_distance = all_distances[all_distances.len() / 2];

    let top_line = infer_missing_timing_marks_on_segment(
        &top_line,
        &Segment::new(
            partial_timing_marks.top_left_corner,
            partial_timing_marks.top_right_corner,
        ),
        median_distance,
        geometry.grid_size.width,
        &geometry,
    );

    let bottom_line = infer_missing_timing_marks_on_segment(
        &bottom_line,
        &Segment::new(
            partial_timing_marks.bottom_left_corner,
            partial_timing_marks.bottom_right_corner,
        ),
        median_distance,
        geometry.grid_size.width,
        &geometry,
    );

    let left_line = infer_missing_timing_marks_on_segment(
        &left_line,
        &Segment::new(
            partial_timing_marks.top_left_corner,
            partial_timing_marks.bottom_left_corner,
        ),
        median_distance,
        geometry.grid_size.height,
        &geometry,
    );

    let right_line = infer_missing_timing_marks_on_segment(
        &right_line,
        &Segment::new(
            partial_timing_marks.top_right_corner,
            partial_timing_marks.bottom_right_corner,
        ),
        median_distance,
        geometry.grid_size.height,
        &geometry,
    );

    if top_line.len() != bottom_line.len() || left_line.len() != right_line.len() {
        return None;
    }

    Some(CompleteTimingMarks {
        geometry: *geometry,
        top_rects: top_line,
        bottom_rects: bottom_line,
        left_rects: left_line,
        right_rects: right_line,
        top_left_corner: partial_timing_marks.top_left_corner,
        top_right_corner: partial_timing_marks.top_right_corner,
        bottom_left_corner: partial_timing_marks.bottom_left_corner,
        bottom_right_corner: partial_timing_marks.bottom_right_corner,
        top_left_rect: *top_left_rect,
        top_right_rect: *top_right_rect,
        bottom_left_rect: *bottom_left_rect,
        bottom_right_rect: *bottom_right_rect,
    })
}

/// Infers missing timing marks along a segment. It's expected that there are
/// timing marks centered at the start and end of the segment and that the
/// distance between them is roughly `expected_distance`. There should be
/// exactly `expected_count` timing marks along the segment.
fn infer_missing_timing_marks_on_segment(
    timing_marks: &[Rect],
    segment: &Segment,
    expected_distance: f32,
    expected_count: u32,
    geometry: &BallotCardGeometry,
) -> Vec<Rect> {
    if timing_marks.is_empty() {
        return vec![];
    }

    let mut inferred_timing_marks = vec![];
    let mut current_timing_mark_center = segment.start;
    let next_point_vector = segment.with_length(expected_distance).vector();
    let maximum_error = expected_distance / 2.0;
    while inferred_timing_marks.len() < expected_count as usize {
        // find the point closest existing timing mark
        let closest_rect = timing_marks
            .iter()
            .min_by(|a, b| {
                let a_distance =
                    Segment::new(center_of_rect(*a), current_timing_mark_center).length();
                let b_distance =
                    Segment::new(center_of_rect(*b), current_timing_mark_center).length();
                a_distance.partial_cmp(&b_distance).unwrap()
            })
            .unwrap();

        // if the closest timing mark is close enough, use it
        if Segment::new(center_of_rect(closest_rect), current_timing_mark_center).length()
            <= maximum_error
        {
            inferred_timing_marks.push(*closest_rect);
            current_timing_mark_center = center_of_rect(closest_rect) + next_point_vector;
        } else {
            // otherwise, we need to fill in a point
            inferred_timing_marks.push(
                Rect::at(
                    (current_timing_mark_center.x - geometry.timing_mark_size.width / 2.0).round()
                        as i32,
                    (current_timing_mark_center.y - geometry.timing_mark_size.height / 2.0).round()
                        as i32,
                )
                .of_size(
                    geometry.timing_mark_size.width.round() as u32,
                    geometry.timing_mark_size.height.round() as u32,
                ),
            );
            current_timing_mark_center = current_timing_mark_center + next_point_vector;
        }
    }
    inferred_timing_marks
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

pub fn load_oval_scan_image() -> Option<GrayImage> {
    let oval_scan_bytes = include_bytes!("../oval_scan.png");
    let inner = io::Cursor::new(oval_scan_bytes);
    let oval_scan_image = match image::load(inner, image::ImageFormat::Png).ok() {
        Some(image) => image.to_luma8(),
        _ => return None,
    };
    Some(bleed(
        &threshold(&oval_scan_image, otsu_level(&oval_scan_image)),
        &Luma([0u8]),
    ))
}

pub struct OvalMarkScore(pub f32);

impl Display for OvalMarkScore {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "{:.2}%", self.0 * 100.0)
    }
}

impl core::fmt::Debug for OvalMarkScore {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "{:.2}%", self.0 * 100.0)
    }
}

impl PartialEq for OvalMarkScore {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

impl PartialOrd for OvalMarkScore {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

pub struct ScoredOvalMark {
    pub location: GridLocation,
    pub match_score: OvalMarkScore,
    pub fill_score: OvalMarkScore,
    pub original_bounds: Rect,
    pub matched_bounds: Rect,
    pub source_image: GrayImage,
    pub binarized_source_image: GrayImage,
    pub match_diff_image: GrayImage,
    pub fill_diff_image: GrayImage,
}

impl std::fmt::Debug for ScoredOvalMark {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(
            f,
            "ScoredOvalMark {{ location: {:?}, match_score: {}, fill_score: {}, matched_bounds: {:?} }}",
            self.location, self.match_score, self.fill_score, self.matched_bounds
        )
    }
}

pub const DEFAULT_MAXIMUM_SEARCH_DISTANCE: u32 = 7;

#[time]
pub fn score_oval_marks_from_grid_layout(
    img: &GrayImage,
    oval_template: &GrayImage,
    timing_mark_grid: &TimingMarkGrid,
    grid_layout: &GridLayout,
    side: BallotSide,
) -> Vec<(GridPosition, Option<ScoredOvalMark>)> {
    let threshold = otsu_level(&img);
    let mut scored_ovals = vec![];

    for grid_position in &grid_layout.grid_positions {
        let location = grid_position.location();

        if location.side != side {
            continue;
        }

        match timing_mark_grid.get(location.column, location.row) {
            Some(expected_oval_center) => {
                scored_ovals.push((
                    grid_position.clone(),
                    score_oval_mark(
                        img,
                        oval_template,
                        &expected_oval_center,
                        &location,
                        DEFAULT_MAXIMUM_SEARCH_DISTANCE,
                        threshold,
                    ),
                ));
            }
            None => scored_ovals.push((grid_position.clone(), None)),
        }
    }

    scored_ovals
}

/// Scores an oval mark within a scanned ballot image.
pub fn score_oval_mark(
    img: &GrayImage,
    oval_template: &GrayImage,
    expected_oval_center: &Point<f32>,
    location: &GridLocation,
    maximum_search_distance: u32,
    threshold: u8,
) -> Option<ScoredOvalMark> {
    let center_x = expected_oval_center.x.round() as u32;
    let center_y = expected_oval_center.y.round() as u32;
    let left = center_x - oval_template.width() / 2;
    let top = center_y - oval_template.height() / 2;
    let width = oval_template.width();
    let height = oval_template.height();
    let original_bounds = Rect::at(left as i32, top as i32).of_size(width, height);
    let mut best_match_score = OvalMarkScore(f32::NEG_INFINITY);
    let mut best_match_bounds: Option<Rect> = None;
    let mut best_match_diff: Option<GrayImage> = None;

    for offset_x in -(maximum_search_distance as i32)..(maximum_search_distance as i32) {
        let x = left as i32 + offset_x;
        if x < 0 {
            continue;
        }

        for offset_y in -(maximum_search_distance as i32)..(maximum_search_distance as i32) {
            let y = top as i32 + offset_y as i32;
            if y < 0 {
                continue;
            }

            let cropped = img.view(x as u32, y as u32, width, height).to_image();
            let cropped_and_thresholded = imageproc::contrast::threshold(&cropped, threshold);

            let match_diff = diff(&cropped_and_thresholded, &oval_template);
            let match_score = OvalMarkScore(ratio(&match_diff, &WHITE));

            if match_score > best_match_score {
                best_match_score = match_score;
                best_match_bounds = Some(Rect::at(x, y).of_size(width, oval_template.height()));
                best_match_diff = Some(match_diff);
            }
        }
    }

    let best_match_bounds = best_match_bounds?;
    let best_match_diff = best_match_diff?;

    let source_image = img
        .view(
            best_match_bounds.left() as u32,
            best_match_bounds.top() as u32,
            best_match_bounds.width(),
            best_match_bounds.height(),
        )
        .to_image();
    let binarized_source_image = imageproc::contrast::threshold(&source_image, threshold);
    let diff_image = diff(&oval_template, &binarized_source_image);
    let fill_score = OvalMarkScore(ratio(&diff_image, &BLACK));

    Some(ScoredOvalMark {
        location: *location,
        match_score: best_match_score,
        fill_score,
        original_bounds,
        matched_bounds: best_match_bounds,
        source_image,
        binarized_source_image,
        match_diff_image: best_match_diff,
        fill_diff_image: diff_image,
    })
}
