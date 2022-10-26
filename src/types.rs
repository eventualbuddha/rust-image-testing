use imageproc::rect::Rect;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum BallotPaperSize {
    Letter,
    Legal,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Size<T> {
    pub width: T,
    pub height: T,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct BallotCardGeometry {
    pub ballot_paper_size: BallotPaperSize,
    pub pixels_per_inch: u32,
    pub canvas_size: Size<u32>,
    pub content_area: Rect,
    pub oval_size: Size<u32>,
    pub timing_mark_size: Size<f32>,
    pub grid_size: Size<u32>,
    pub front_usable_area: Rect,
    pub back_usable_area: Rect,
}
