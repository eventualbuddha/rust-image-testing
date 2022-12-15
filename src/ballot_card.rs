use std::io;

use image::{GrayImage, Luma};
use imageproc::{rect::Rect, contrast::{threshold, otsu_level}};

use crate::image_utils::bleed;


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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BallotSide {
    Front,
    Back,
}

pub fn get_scanned_ballot_card_geometry_8pt5x11() -> BallotCardGeometry {
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

pub fn get_scanned_ballot_card_geometry_8pt5x14() -> BallotCardGeometry {
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

pub fn get_scanned_ballot_card_geometry(size: (u32, u32)) -> Option<BallotCardGeometry> {
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
pub fn load_oval_template() -> Option<GrayImage> {
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
