use std::path::{Path, PathBuf};

use image::GrayImage;
use imageproc::rect::Rect;
use logging_timer::time;

use crate::ballot_card::get_scanned_ballot_card_geometry;
use crate::ballot_card::BallotCardGeometry;
use crate::ballot_card::BallotSide;
use crate::debug::ImageDebugWriter;
use crate::election::BallotStyleId;
use crate::election::Election;
use crate::image_utils::size_image_to_fit;
use crate::metadata::BallotCardMetadata;
use crate::metadata::BallotCardMetadataError;
use crate::timing_marks::find_timing_mark_grid;
use crate::timing_marks::{score_oval_marks_from_grid_layout, ScoredOvalMarks, TimingMarkGrid};

#[derive(Debug, Clone)]
pub struct InterpretOptions {
    pub debug: bool,
    pub oval_template: GrayImage,
    pub election: Election,
}

pub type LoadedBallotPage = (GrayImage, BallotCardGeometry);
pub type LoadedBallotCard = (GrayImage, GrayImage, BallotCardGeometry);

pub type InterpretedBallotPage = (TimingMarkGrid, ScoredOvalMarks);
pub type InterpretBallotCardResult =
    Result<(InterpretedBallotPage, InterpretedBallotPage), InterpretBallotCardError>;

#[derive(Debug)]
pub enum InterpretBallotCardError {
    ImageOpenError(PathBuf),
    InvalidCardMetadata(BallotCardMetadata, BallotCardMetadata),
    MetadataError(PathBuf, BallotCardMetadataError),
    MismatchedBallotCardGeometries((PathBuf, BallotCardGeometry), (PathBuf, BallotCardGeometry)),
    MissingGridLayout(BallotCardMetadata, BallotCardMetadata),
    MissingTimingMarks(Vec<Rect>),
    UnexpectedDimensionsError(PathBuf, (u32, u32)),
}

#[time]
/// Load both sides of a ballot card image and return the ballot card.
fn load_ballot_card_images(
    side_a_path: &Path,
    side_b_path: &Path,
) -> Result<LoadedBallotCard, InterpretBallotCardError> {
    let (side_a_result, side_b_result) = rayon::join(
        || load_ballot_page_image(side_a_path),
        || load_ballot_page_image(side_b_path),
    );

    let (side_a_image, side_a_geometry) = side_a_result?;
    let (side_b_image, side_b_geometry) = side_b_result?;

    if side_a_geometry != side_b_geometry {
        return Err(InterpretBallotCardError::MismatchedBallotCardGeometries(
            (side_a_path.to_path_buf(), side_a_geometry),
            (side_b_path.to_path_buf(), side_b_geometry),
        ));
    }

    Ok((side_a_image, side_b_image, side_a_geometry))
}

#[time]
pub fn load_ballot_page_image(
    image_path: &Path,
) -> Result<LoadedBallotPage, InterpretBallotCardError> {
    let img = match image::open(&image_path) {
        Ok(img) => img.into_luma8(),
        Err(_) => {
            return Err(InterpretBallotCardError::ImageOpenError(
                image_path.to_path_buf(),
            ))
        }
    };

    let geometry = match get_scanned_ballot_card_geometry(img.dimensions()) {
        Some(geometry) => geometry,
        None => {
            return Err(InterpretBallotCardError::UnexpectedDimensionsError(
                image_path.to_path_buf(),
                img.dimensions(),
            ))
        }
    };

    let img = size_image_to_fit(
        &img,
        geometry.canvas_size.width,
        geometry.canvas_size.height,
    );

    Ok((img, geometry))
}

#[time]
pub fn interpret_ballot_card(
    side_a_path: &Path,
    side_b_path: &Path,
    options: &InterpretOptions,
) -> InterpretBallotCardResult {
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
        || find_timing_mark_grid(&side_a_path, &geometry, &side_a_image, &side_a_debug),
        || find_timing_mark_grid(&side_b_path, &geometry, &side_b_image, &side_b_debug),
    );

    let side_a_grid = side_a_result?;
    let side_b_grid = side_b_result?;

    let ((front_image, front_grid, front_debug), (back_image, back_grid, back_debug)) =
        match (&side_a_grid.metadata, &side_b_grid.metadata) {
            (BallotCardMetadata::Front(_), BallotCardMetadata::Back(_)) => (
                (side_a_image, side_a_grid, side_a_debug),
                (side_b_image, side_b_grid, side_b_debug),
            ),
            (BallotCardMetadata::Back(_), BallotCardMetadata::Front(_)) => (
                (side_b_image, side_b_grid, side_b_debug),
                (side_a_image, side_a_grid, side_a_debug),
            ),
            _ => {
                return Err(InterpretBallotCardError::InvalidCardMetadata(
                    side_a_grid.metadata,
                    side_b_grid.metadata,
                ))
            }
        };

    let ballot_style_id = match &front_grid.metadata {
        BallotCardMetadata::Front(metadata) => {
            BallotStyleId::from(format!("card-number-{}", metadata.card_number))
        }
        _ => unreachable!(),
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
            return Err(InterpretBallotCardError::MissingGridLayout(
                front_grid.metadata,
                back_grid.metadata,
            ))
        }
    };

    let (front_scored_oval_marks, back_scored_oval_marks) = rayon::join(
        || {
            score_oval_marks_from_grid_layout(
                &front_image,
                &options.oval_template,
                &front_grid,
                &grid_layout,
                BallotSide::Front,
                &front_debug,
            )
        },
        || {
            score_oval_marks_from_grid_layout(
                &back_image,
                &options.oval_template,
                &back_grid,
                &grid_layout,
                BallotSide::Back,
                &back_debug,
            )
        },
    );

    Ok((
        (front_grid, front_scored_oval_marks),
        (back_grid, back_scored_oval_marks),
    ))
}
