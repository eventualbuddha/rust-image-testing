use std::f32::consts::PI;

use imageproc::point::Point;
use imageproc::rect::Rect;

pub type Segment<T> = (Point<T>, Point<T>);

pub fn intersection_of_lines(
    segment1: &Segment<f32>,
    segment2: &Segment<f32>,
    bounded: bool,
) -> Option<Point<f32>> {
    let (p1, p2) = segment1;
    let (p3, p4) = segment2;
    let d = (p4.y - p3.y) * (p2.x - p1.x) - (p4.x - p3.x) * (p2.y - p1.y);
    if d == 0.0 {
        return None;
    }
    let u_a = ((p4.x - p3.x) * (p1.y - p3.y) - (p4.y - p3.y) * (p1.x - p3.x)) / d;
    let u_b = ((p2.x - p1.x) * (p1.y - p3.y) - (p2.y - p1.y) * (p1.x - p3.x)) / d;
    if !bounded || (u_a >= 0.0 && u_a <= 1.0 && u_b >= 0.0 && u_b <= 1.0) {
        return Some(Point::new(
            p1.x + u_a * (p2.x - p1.x),
            p1.y + u_a * (p2.y - p1.y),
        ));
    }
    return None;
}

pub fn lines_intersect(line1: &Segment<f32>, line2: &Segment<f32>) -> bool {
    // let (p1, p2) = line1;
    // let (p3, p4) = line2;
    // let d = (p4.y - p3.y) * (p2.x - p1.x) - (p4.x - p3.x) * (p2.y - p1.y);
    // if d == 0.0 {
    //     return false;
    // }
    // let u_a = ((p4.x - p3.x) * (p1.y - p3.y) - (p4.y - p3.y) * (p1.x - p3.x)) / d;
    // let u_b = ((p2.x - p1.x) * (p1.y - p3.y) - (p2.y - p1.y) * (p1.x - p3.x)) / d;
    // return u_a >= 0.0 && u_a <= 1.0 && u_b >= 0.0 && u_b <= 1.0;
    return intersection_of_lines(line1, line2, true).is_some();
}

pub fn rect_intersects_line(rect: &Rect, line: &Segment<f32>) -> bool {
    let top_left = Point::new(rect.left() as f32, rect.top() as f32);
    let top_right = Point::new(rect.right() as f32, rect.top() as f32);
    let bottom_left = Point::new(rect.left() as f32, rect.bottom() as f32);
    let bottom_right = Point::new(rect.right() as f32, rect.bottom() as f32);
    let top_line = (top_left, top_right);
    let right_line = (top_right, bottom_right);
    let bottom_line = (bottom_left, bottom_right);
    let left_line = (top_left, bottom_left);

    return lines_intersect(&top_line, &line)
        || lines_intersect(&bottom_line, &line)
        || lines_intersect(&left_line, &line)
        || lines_intersect(&right_line, &line);
}

pub fn angle_diff(a: f32, b: f32) -> f32 {
    let diff = normalize_angle(a - b);
    return diff.min(PI - diff);
}

pub fn normalize_angle(angle: f32) -> f32 {
    let mut angle = angle;
    while angle < 0.0 {
        angle += PI;
    }
    while angle >= PI {
        angle -= PI;
    }
    return angle;
}

pub fn distance_from_point_to_point(p1: &Point<f32>, p2: &Point<f32>) -> f32 {
    ((p1.x - p2.x).powf(2.0) + (p1.y - p2.y).powf(2.0)).sqrt()
}

pub fn distances_between_rects(rects: &Vec<Rect>) -> Vec<f32> {
    let mut distances = rects
        .windows(2)
        .map(|w| distance_from_point_to_point(&center_of_rect(&w[1]), &center_of_rect(&w[0])))
        .collect::<Vec<f32>>();
    distances.sort_by(|a, b| a.partial_cmp(b).unwrap());
    return distances;
}

pub fn segment_from_point_toward_point_with_length(
    p1: &Point<f32>,
    p2: &Point<f32>,
    length: f32,
) -> Segment<f32> {
    let angle = (p2.y - p1.y).atan2(p2.x - p1.x);
    let p3 = Point::new(p1.x + length * angle.cos(), p1.y + length * angle.sin());
    return (p1.clone(), p3);
}

pub fn center_of_rect(rect: &Rect) -> Point<f32> {
    return Point::new(
        rect.left() as f32 + rect.width() as f32 / 2.0,
        rect.top() as f32 + rect.height() as f32 / 2.0,
    );
}
