use std::path::Path;

use image::GrayImage;
use logging_timer::time;
use serde::Serialize;

use crate::ballot_card::get_scanned_ballot_card_geometry;
use crate::ballot_card::BallotSide;
use crate::ballot_card::Geometry;
use crate::debug::ImageDebugWriter;
use crate::election::BallotStyleId;
use crate::election::Election;
use crate::geometry::Rect;
use crate::geometry::Size;
use crate::image_utils::size_image_to_fit;
use crate::metadata::BallotPageMetadata;
use crate::metadata::BallotPageMetadataError;
use crate::timing_marks::find_timing_mark_grid;
use crate::timing_marks::{score_oval_marks_from_grid_layout, ScoredOvalMarks, TimingMarkGrid};

#[derive(Debug, Clone)]
pub struct Options {
    pub debug: bool,
    pub oval_template: GrayImage,
    pub election: Election,
}

pub type LoadedBallotPage = (GrayImage, Geometry);
pub type LoadedBallotCard = (GrayImage, GrayImage, Geometry);

pub type InterpretedBallotPage = (TimingMarkGrid, ScoredOvalMarks);
#[derive(Debug, Serialize)]
pub struct InterpretedBallotCard {
    pub front: InterpretedBallotPage,
    pub back: InterpretedBallotPage,
}
pub type Result = core::result::Result<InterpretedBallotCard, Error>;

#[derive(Debug, Serialize)]
pub struct BallotPagePathAndGeometry {
    pub path: String,
    pub geometry: Geometry,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum Error {
    ImageOpenFailure {
        path: String,
    },
    InvalidCardMetadata {
        side_a: BallotPageMetadata,
        side_b: BallotPageMetadata,
    },
    InvalidMetadata {
        path: String,
        error: BallotPageMetadataError,
    },
    MismatchedBallotCardGeometries {
        side_a: BallotPagePathAndGeometry,
        side_b: BallotPagePathAndGeometry,
    },
    MissingGridLayout {
        front: BallotPageMetadata,
        back: BallotPageMetadata,
    },
    MissingTimingMarks {
        rects: Vec<Rect>,
    },
    UnexpectedDimensions {
        path: String,
        dimensions: Size<u32>,
    },
}

#[time]
/// Load both sides of a ballot card image and return the ballot card.
fn load_ballot_card_images(
    side_a_path: &Path,
    side_b_path: &Path,
) -> core::result::Result<LoadedBallotCard, Error> {
    let (side_a_result, side_b_result) = rayon::join(
        || load_ballot_page_image(side_a_path),
        || load_ballot_page_image(side_b_path),
    );

    let (side_a_image, side_a_geometry) = side_a_result?;
    let (side_b_image, side_b_geometry) = side_b_result?;

    if side_a_geometry != side_b_geometry {
        return Err(Error::MismatchedBallotCardGeometries {
            side_a: BallotPagePathAndGeometry {
                path: side_a_path.to_str().unwrap_or_default().to_string(),
                geometry: side_a_geometry,
            },
            side_b: BallotPagePathAndGeometry {
                path: side_b_path.to_str().unwrap_or_default().to_string(),
                geometry: side_b_geometry,
            },
        });
    }

    Ok((side_a_image, side_b_image, side_a_geometry))
}

#[time]
pub fn load_ballot_page_image(image_path: &Path) -> core::result::Result<LoadedBallotPage, Error> {
    let img = match image::open(image_path) {
        Ok(img) => img.into_luma8(),
        Err(_) => {
            return Err(Error::ImageOpenFailure {
                path: image_path.to_str().unwrap_or_default().to_string(),
            })
        }
    };

    let geometry = if let Some(geometry) = get_scanned_ballot_card_geometry(img.dimensions()) {
        geometry
    } else {
        let (width, height) = img.dimensions();
        return Err(Error::UnexpectedDimensions {
            path: image_path.to_str().unwrap_or_default().to_string(),
            dimensions: Size { width, height },
        });
    };

    let img = size_image_to_fit(
        &img,
        geometry.canvas_size.width,
        geometry.canvas_size.height,
    );

    Ok((img, geometry))
}

#[time]
pub fn interpret_ballot_card(side_a_path: &Path, side_b_path: &Path, options: &Options) -> Result {
    let (side_a_image, side_b_image, geometry) = load_ballot_card_images(side_a_path, side_b_path)?;

    let side_a_debug = if options.debug {
        ImageDebugWriter::new(side_a_path.to_path_buf(), side_a_image.clone())
    } else {
        ImageDebugWriter::disabled()
    };
    let side_b_debug = if options.debug {
        ImageDebugWriter::new(side_b_path.to_path_buf(), side_b_image.clone())
    } else {
        ImageDebugWriter::disabled()
    };

    let (side_a_result, side_b_result) = rayon::join(
        || find_timing_mark_grid(side_a_path, &geometry, &side_a_image, &side_a_debug),
        || find_timing_mark_grid(side_b_path, &geometry, &side_b_image, &side_b_debug),
    );

    let side_a_grid = side_a_result?;
    let side_b_grid = side_b_result?;

    let ((front_image, front_grid, front_debug), (back_image, back_grid, back_debug)) =
        match (&side_a_grid.metadata, &side_b_grid.metadata) {
            (BallotPageMetadata::Front(_), BallotPageMetadata::Back(_)) => (
                (side_a_image, side_a_grid, side_a_debug),
                (side_b_image, side_b_grid, side_b_debug),
            ),
            (BallotPageMetadata::Back(_), BallotPageMetadata::Front(_)) => (
                (side_b_image, side_b_grid, side_b_debug),
                (side_a_image, side_a_grid, side_a_debug),
            ),
            _ => {
                return Err(Error::InvalidCardMetadata {
                    side_a: side_a_grid.metadata,
                    side_b: side_b_grid.metadata,
                })
            }
        };

    let ballot_style_id = match &front_grid.metadata {
        BallotPageMetadata::Front(metadata) => {
            BallotStyleId::from(format!("card-number-{}", metadata.card_number))
        }
        BallotPageMetadata::Back(_) => unreachable!(),
    };

    // TODO: discover this from the ballot card metadata
    let grid_layout = match options
        .election
        .grid_layouts
        .iter()
        .find(|layout| layout.ballot_style_id == ballot_style_id)
    {
        Some(layout) => layout,
        None => {
            return Err(Error::MissingGridLayout {
                front: front_grid.metadata,
                back: back_grid.metadata,
            })
        }
    };

    let (front_scored_oval_marks, back_scored_oval_marks) = rayon::join(
        || {
            score_oval_marks_from_grid_layout(
                &front_image,
                &options.oval_template,
                &front_grid,
                grid_layout,
                BallotSide::Front,
                &front_debug,
            )
        },
        || {
            score_oval_marks_from_grid_layout(
                &back_image,
                &options.oval_template,
                &back_grid,
                grid_layout,
                BallotSide::Back,
                &back_debug,
            )
        },
    );

    Ok(InterpretedBallotCard {
        front: (front_grid, front_scored_oval_marks),
        back: (back_grid, back_scored_oval_marks),
    })
}
