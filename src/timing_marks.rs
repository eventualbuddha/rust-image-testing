use imageproc::rect::Rect;

use crate::types::BallotCardGeometry;

pub fn rect_could_be_timing_mark(geometry: &BallotCardGeometry, rect: &Rect) -> bool {
    let min_timing_mark_width = (geometry.timing_mark_size.width * 1.0 / 4.0).floor() as u32;
    let max_timing_mark_width = (geometry.timing_mark_size.width * 3.0 / 2.0).ceil() as u32;
    let min_timing_mark_height = (geometry.timing_mark_size.height * 2.0 / 3.0).floor() as u32;
    let max_timing_mark_height = (geometry.timing_mark_size.height * 3.0 / 2.0).ceil() as u32;
    return rect.width() >= min_timing_mark_width
        && rect.width() <= max_timing_mark_width
        && rect.height() >= min_timing_mark_height
        && rect.height() <= max_timing_mark_height;
}