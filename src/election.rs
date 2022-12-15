use std::fmt::Display;

use serde::{Deserialize, Serialize};

use crate::{types::idtype, ballot_card::BallotSide};

// import idtype macro from types.rs

idtype!(ContestId);
idtype!(OptionId);
idtype!(BallotStyleId);
idtype!(PrecinctId);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Election {
    pub title: String,
    pub grid_layouts: Vec<GridLayout>,
    pub mark_thresholds: Option<MarkThresholds>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GridLayout {
    pub precinct_id: PrecinctId,
    pub ballot_style_id: BallotStyleId,
    pub columns: u32,
    pub rows: u32,
    pub grid_positions: Vec<GridPosition>,
}

/// A position on the ballot grid defined by timing marks and the contest/option
/// for which a mark at this position is a vote for.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum GridPosition {
    /// A pre-defined labeled option on the ballot.
    #[serde(rename_all = "camelCase", rename = "option")]
    Option {
        side: BallotSide,
        column: u32,
        row: u32,
        contest_id: ContestId,
        option_id: OptionId,
    },

    /// A write-in option on the ballot.
    #[serde(rename_all = "camelCase", rename = "write-in")]
    WriteIn {
        side: BallotSide,
        column: u32,
        row: u32,
        contest_id: ContestId,
        write_in_index: u32,
    },
}

impl Display for GridPosition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GridPosition::Option { option_id, .. } => write!(f, "{}", option_id),

            GridPosition::WriteIn { write_in_index, .. } => {
                write!(f, "Write-In {}", write_in_index)
            }
        }
    }
}

impl GridPosition {
    pub fn location(&self) -> GridLocation {
        match self {
            GridPosition::Option {
                side, column, row, ..
            } => GridLocation::new(*side, *column, *row),

            GridPosition::WriteIn {
                side, column, row, ..
            } => GridLocation::new(*side, *column, *row),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GridLocation {
    pub side: BallotSide,
    pub column: u32,
    pub row: u32,
}

impl GridLocation {
    pub fn new(side: BallotSide, column: u32, row: u32) -> Self {
        Self { side, column, row }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkThresholds {
    pub definite: f32,
    pub marginal: f32,
}
