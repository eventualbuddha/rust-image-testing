use std::fmt::Display;

use serde::{Serialize, Deserialize};

use crate::types::BallotSide;

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
    pub precinct_id: String,
    pub ballot_style_id: String,
    pub columns: u32,
    pub rows: u32,
    pub grid_positions: Vec<GridPosition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum GridPosition {
    #[serde(rename_all = "camelCase", rename = "option")]
    Option {
        side: String,
        column: u32,
        row: u32,
        contest_id: String,
        option_id: String,
    },
    #[serde(rename_all = "camelCase", rename = "write-in")]
    WriteIn {
        side: String,
        column: u32,
        row: u32,
        contest_id: String,
        write_in_index: u32,
    },
}

impl Display for GridPosition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GridPosition::Option { option_id, .. } => 
                write!(f, "{}", option_id),
            
            GridPosition::WriteIn { write_in_index, .. } =>
                write!(f, "Write-In {}", write_in_index),
        }
    }
}

impl GridPosition {
    pub fn id(&self) -> String {
        match self {
            GridPosition::Option { contest_id, option_id, .. } => 
                format!("{}-{}", contest_id, option_id),
            
            GridPosition::WriteIn { contest_id, write_in_index, .. } =>
                format!("{}-write-in-{}", contest_id, write_in_index),
        }
    }

    pub fn location(&self) -> GridLocation {
        match self {
            GridPosition::Option { side, column, row, .. } => 
                GridLocation::new(side.as_str().into(), *column, *row),
            
            GridPosition::WriteIn { side, column, row, .. } =>
                GridLocation::new(side.as_str().into(), *column, *row),
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
