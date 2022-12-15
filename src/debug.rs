use std::path::{Path, PathBuf};

use image::{RgbImage, Rgb};
use imageproc::{
    drawing::{
        draw_cross_mut, draw_filled_rect_mut, draw_hollow_rect_mut, draw_line_segment_mut,
        draw_text_mut, text_size,
    },
    rect::Rect,
};
use rusttype::{Font, Scale};

use crate::{
    election::GridPosition,
    geometry::{segment_with_length, Segment},
    image_utils::{
        BLUE, CYAN, DARK_BLUE, DARK_CYAN, DARK_GREEN, DARK_RED, GREEN, PINK, RAINBOW,
        RED, WHITE_RGB,
    },
    timing_marks::{PartialTimingMarks, ScoredOvalMark, TimingMarkGrid},
    types::BallotCardGeometry,
};

/// Creates a path for a debug image.
pub fn debug_image_path(base: &Path, label: &str) -> PathBuf {
    let mut result = PathBuf::from(base);
    result.set_file_name(format!(
        "{}_debug_{}.png",
        base.file_stem().unwrap_or_default().to_str().unwrap(),
        label
    ));
    result
}

/// Draws a debug image of the rectangles found using the contour algorithm.
pub fn draw_contour_rects_debug_image_mut(canvas: &mut RgbImage, contour_rects: &Vec<Rect>) {
    for (i, rect) in contour_rects.iter().enumerate() {
        draw_filled_rect_mut(canvas, *rect, RAINBOW[i % RAINBOW.len()]);
    }
}

/// Draws a debug image of the timing marks.
pub fn draw_timing_mark_debug_image_mut(
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
        GREEN,
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
        BLUE,
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
        RED,
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
        CYAN,
    );

    for rect in &partial_timing_marks.top_rects {
        draw_filled_rect_mut(canvas, *rect, GREEN);
    }
    for rect in &partial_timing_marks.bottom_rects {
        draw_filled_rect_mut(canvas, *rect, BLUE);
    }
    for rect in &partial_timing_marks.left_rects {
        draw_filled_rect_mut(canvas, *rect, RED);
    }
    for rect in &partial_timing_marks.right_rects {
        draw_filled_rect_mut(canvas, *rect, CYAN);
    }

    if let Some(top_left_corner) = partial_timing_marks.top_left_rect {
        draw_filled_rect_mut(canvas, top_left_corner, PINK);
    }

    if let Some(top_right_corner) = partial_timing_marks.top_right_rect {
        draw_filled_rect_mut(canvas, top_right_corner, PINK);
    }

    if let Some(bottom_left_corner) = partial_timing_marks.bottom_left_rect {
        draw_filled_rect_mut(canvas, bottom_left_corner, PINK);
    }

    if let Some(bottom_right_corner) = partial_timing_marks.bottom_right_rect {
        draw_filled_rect_mut(canvas, bottom_right_corner, PINK);
    }

    draw_cross_mut(
        canvas,
        WHITE_RGB,
        partial_timing_marks.top_left_corner.x.round() as i32,
        partial_timing_marks.top_left_corner.y.round() as i32,
    );

    draw_cross_mut(
        canvas,
        WHITE_RGB,
        partial_timing_marks.top_right_corner.x.round() as i32,
        partial_timing_marks.top_right_corner.y.round() as i32,
    );

    draw_cross_mut(
        canvas,
        WHITE_RGB,
        partial_timing_marks.bottom_left_corner.x.round() as i32,
        partial_timing_marks.bottom_left_corner.y.round() as i32,
    );

    draw_cross_mut(
        canvas,
        WHITE_RGB,
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
            DARK_GREEN,
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
            DARK_BLUE,
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
            DARK_RED,
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
            DARK_CYAN,
            expected_right_timing_mark_center.x.round() as i32,
            expected_right_timing_mark_center.y.round() as i32,
        );
    }
}

/// Draws a debug image showing all the points of the timing mark grid.
pub fn draw_timing_mark_grid_debug_image_mut(
    canvas: &mut RgbImage,
    timing_mark_grid: &TimingMarkGrid,
    geometry: &BallotCardGeometry,
) {
    for x in 0..geometry.grid_size.width {
        for y in 0..geometry.grid_size.height {
            let point = timing_mark_grid.get(x, y).expect("grid point is defined");
            draw_cross_mut(canvas, PINK, point.x.round() as i32, point.y.round() as i32);
        }
    }
}

fn monospace_font() -> Font<'static> {
    Font::try_from_bytes(include_bytes!("../fonts/Inconsolata-Regular.ttf")).expect("font is valid")
}

/// Draws a debug image outlining all the scored oval marks.
pub fn draw_scored_oval_marks_debug_image_mut(
    canvas: &mut RgbImage,
    scored_oval_marks: &Vec<(GridPosition, Option<ScoredOvalMark>)>,
) {
    let option_color = PINK;
    let matched_oval_color = DARK_GREEN;
    let original_oval_color = DARK_BLUE;
    let score_color = DARK_GREEN;
    let font = &monospace_font();
    let font_scale = 20.0;
    let scale = Scale::uniform(font_scale);

    for (grid_position, scored_oval_mark) in scored_oval_marks {
        if let Some(scored_oval_mark) = scored_oval_mark {
            let mut option_text = grid_position.to_string();
            option_text.truncate(25);

            let (option_text_width, option_text_height) =
                text_size(scale, font, option_text.as_str());

            let score_text = scored_oval_mark.fill_score.to_string();
            let (score_text_width, _) = text_size(scale, font, score_text.as_str());

            draw_text_with_background_mut(
                canvas,
                &option_text,
                scored_oval_mark
                    .original_bounds
                    .left()
                    .min(scored_oval_mark.matched_bounds.left())
                    - option_text_width as i32
                    - 5,
                (scored_oval_mark
                    .original_bounds
                    .top()
                    .min(scored_oval_mark.matched_bounds.top())
                    + scored_oval_mark
                        .original_bounds
                        .bottom()
                        .max(scored_oval_mark.matched_bounds.bottom())) as i32
                    / 2
                    - (option_text_height as i32 / 2),
                scale,
                font,
                option_color,
                WHITE_RGB,
            );

            draw_text_with_background_mut(
                canvas,
                &score_text,
                (scored_oval_mark
                    .original_bounds
                    .left()
                    .min(scored_oval_mark.matched_bounds.left())
                    + scored_oval_mark
                        .original_bounds
                        .right()
                        .max(scored_oval_mark.matched_bounds.right())) as i32
                    / 2
                    - (score_text_width as i32 / 2),
                scored_oval_mark
                    .original_bounds
                    .bottom()
                    .max(scored_oval_mark.matched_bounds.bottom()) as i32
                    + 5,
                scale,
                font,
                score_color,
                WHITE_RGB,
            );

            draw_hollow_rect_mut(
                canvas,
                scored_oval_mark.original_bounds,
                original_oval_color,
            );
            draw_hollow_rect_mut(canvas, scored_oval_mark.matched_bounds, matched_oval_color);
        }
    }
}

fn draw_text_with_background_mut(
    canvas: &mut RgbImage,
    text: &str,
    x: i32,
    y: i32,
    scale: Scale,
    font: &Font,
    text_color: Rgb<u8>,
    background_color: Rgb<u8>,
) {
    let (text_width, text_height) = text_size(scale, font, text);
    let text_width = text_width as i32;
    let text_height = text_height as i32;

    draw_filled_rect_mut(
        canvas,
        Rect::at(x, y).of_size(text_width as u32, text_height as u32),
        background_color,
    );
    draw_text_mut(canvas, text_color, x, y, scale, font, text);
}
