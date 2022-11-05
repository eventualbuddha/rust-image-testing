use std::f32::consts::PI;

use imageproc::point::Point;
use imageproc::rect::Rect;

/// A line segment from `start` to `end`.
pub struct Segment<T> {
    pub start: Point<T>,
    pub end: Point<T>,
}

impl<T> Segment<T> {
    /// Creates a new line segment from `start` to `end`.
    pub const fn new(start: Point<T>, end: Point<T>) -> Self {
        Self { start, end }
    }
}

/// Determines an intersection point of two line segments. If `bounded` is set
/// to `true`, the intersection point must be within the bounds of both
/// segments. If `bounded` is set to `false`, the intersection point may be
/// outside the bounds of either segment.
pub fn intersection_of_lines(
    segment1: &Segment<f32>,
    segment2: &Segment<f32>,
    bounded: bool,
) -> Option<Point<f32>> {
    let p1 = segment1.start;
    let p2 = segment1.end;
    let p3 = segment2.start;
    let p4 = segment2.end;
    let d = (p4.y - p3.y) * (p2.x - p1.x) - (p4.x - p3.x) * (p2.y - p1.y);
    if d == 0.0 {
        return None;
    }
    let u_a = ((p4.x - p3.x) * (p1.y - p3.y) - (p4.y - p3.y) * (p1.x - p3.x)) / d;
    let u_b = ((p2.x - p1.x) * (p1.y - p3.y) - (p2.y - p1.y) * (p1.x - p3.x)) / d;
    if !bounded || ((0.0..=1.0).contains(&u_a) && (0.0..=1.0).contains(&u_b)) {
        return Some(Point::new(
            u_a.mul_add(p2.x - p1.x, p1.x),
            u_a.mul_add(p2.y - p1.y, p1.y),
        ));
    }
    None
}

/// Determines whether the two line segments intersect.
pub fn segments_intersect(line1: &Segment<f32>, line2: &Segment<f32>) -> bool {
    intersection_of_lines(line1, line2, true).is_some()
}

/// Determines whether a line segment intersects a rectangle.
pub fn rect_intersects_line(rect: &Rect, line: &Segment<f32>) -> bool {
    let top_left = Point::new(rect.left() as f32, rect.top() as f32);
    let top_right = Point::new(rect.right() as f32, rect.top() as f32);
    let bottom_left = Point::new(rect.left() as f32, rect.bottom() as f32);
    let bottom_right = Point::new(rect.right() as f32, rect.bottom() as f32);
    let top_line = Segment::new(top_left, top_right);
    let right_line = Segment::new(top_right, bottom_right);
    let bottom_line = Segment::new(bottom_left, bottom_right);
    let left_line = Segment::new(top_left, bottom_left);

    segments_intersect(&top_line, line)
        || segments_intersect(&bottom_line, line)
        || segments_intersect(&left_line, line)
        || segments_intersect(&right_line, line)
}

/// Returns the angle between two angles in radians.
pub fn angle_diff(a: f32, b: f32) -> f32 {
    let diff = normalize_angle(a - b);
    diff.min(PI - diff)
}

/// Normalize angle to [0, PI). This means that two angles that are
/// equivalent modulo PI will be equal, e.g. 90° and 270°, even though
/// they are not equal in the mathematical sense.
pub fn normalize_angle(angle: f32) -> f32 {
    if angle.is_infinite() || angle.is_nan() {
        return angle;
    }

    let mut angle = angle % (2.0 * PI);
    while angle < 0.0 {
        angle += PI;
    }
    while angle >= PI {
        angle -= PI;
    }
    angle
}

/// Gets the distance between the two points of a line segment.
pub fn segment_distance(segment: &Segment<f32>) -> f32 {
    let p1 = segment.start;
    let p2 = segment.end;
    ((p1.x - p2.x).powi(2) + (p1.y - p2.y).powi(2)).sqrt()
}

/// Generates a new segment based on the given segment, but with the
/// given length. The new segment will have the same start point as the
/// given segment, but the end point will be the given length away from
/// the start point. The angle of the new segment will be the same as
/// the given segment.
pub fn segment_with_length(segment: &Segment<f32>, length: f32) -> Segment<f32> {
    let p1 = segment.start;
    let p2 = segment.end;
    let angle = (p2.y - p1.y).atan2(p2.x - p1.x);
    let p3 = Point::new(
        length.mul_add(angle.cos(), p1.x),
        length.mul_add(angle.sin(), p1.y),
    );
    Segment::new(p1, p3)
}

/// Returns the center of a rect.
pub fn center_of_rect(rect: &Rect) -> Point<f32> {
    Point::new(
        rect.left() as f32 + (rect.right() as f32 - rect.left() as f32) / 2.0,
        rect.top() as f32 + (rect.bottom() as f32 - rect.top() as f32) / 2.0,
    )
}

#[cfg(test)]
mod normalize_angle_tests {
    use std::{f32::consts::PI, ops::Range};

    use proptest::prelude::*;

    const ANGLE_RANGE: Range<f32> = -(10.0 * PI)..(10.0 * PI);

    macro_rules! assert_nearly_eq {
        ($a:expr, $b:expr) => {
            assert!(
                ($a - $b).abs() < 0.0001,
                "assertion failed: `({} - {}) < 0.0001`",
                $a,
                $b
            );
        };
    }

    #[test]
    fn test_normalize_angle() {
        assert_nearly_eq!(super::normalize_angle(0.0), 0.0);
        assert_nearly_eq!(super::normalize_angle(PI), 0.0);
        assert_nearly_eq!(super::normalize_angle(2.0 * PI), 0.0);
        assert_nearly_eq!(super::normalize_angle(1.5 * PI), 0.5 * PI);
    }

    #[test]
    fn test_normalize_infinity() {
        assert_eq!(super::normalize_angle(f32::INFINITY), f32::INFINITY);
        assert_eq!(super::normalize_angle(f32::NEG_INFINITY), f32::NEG_INFINITY);
    }

    proptest! {
        #[test]
        fn prop_normalize_angle(angle in ANGLE_RANGE) {
            let normalized = super::normalize_angle(angle);
            assert!((0.0..PI).contains(&normalized));
        }

        #[test]
        fn prop_normalize_angle_is_idempotent(angle in ANGLE_RANGE) {
            let normalized = super::normalize_angle(angle);
            let normalized_again = super::normalize_angle(normalized);
            assert_nearly_eq!(normalized, normalized_again);
        }

        #[test]
        fn prop_normalize_angle_is_equivalent(angle in ANGLE_RANGE) {
            let normalized = super::normalize_angle(angle);
            let equivalent = super::normalize_angle(angle + PI);
            assert_nearly_eq!(normalized, equivalent);
        }
    }
}

#[cfg(test)]
mod normalize_center_of_rect {
    use proptest::prelude::*;

    #[test]
    fn test_center_of_rect() {
        let rect = super::Rect::at(0, 0).of_size(10, 10);
        let center = super::center_of_rect(&rect);
        assert_eq!(center.x, 4.5);
        assert_eq!(center.y, 4.5);
    }

    #[test]
    fn test_center_of_rect_with_odd_dimensions() {
        let rect = super::Rect::at(0, 0).of_size(11, 11);
        let center = super::center_of_rect(&rect);
        assert_eq!(center.x, 5.0);
        assert_eq!(center.y, 5.0);
    }

    proptest! {
        #[test]
        fn prop_center_of_rect_is_in_rect(x in 0i32..100i32, y in 0i32..100i32, width in 1u32..100u32, height in 1u32..100u32) {
            let rect = super::Rect::at(x, y).of_size(width, height);
            let center = super::center_of_rect(&rect);
            prop_assert!((rect.left() as f32) <= center.x);
            prop_assert!(center.x <= (rect.right() as f32));
            prop_assert!((rect.top() as f32) <= center.y);
            prop_assert!(center.y <= (rect.bottom() as f32));
        }
    }
}
