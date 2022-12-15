use std::fmt::{Debug, Formatter};

use imageproc::rect::Rect;

use crate::timing_marks::{CompleteTimingMarks, PartialTimingMarks};

pub const METADATA_BITS: usize = 32;

pub const ENDER_CODE: [bool; 11] = [
    false, true, true, true, true, false, true, true, true, true, false,
];

fn print_boolean_slice_as_binary(slice: &[bool]) -> String {
    slice
        .iter()
        .map(|b| if *b { "1" } else { "0" })
        .collect::<Vec<_>>()
        .join("")
}

/// Metadata encoded by the bottom row of the front of a ballot card.
pub struct BallotCardMetadataFront {
    /// Raw bits 0-31 in LSB-MSB order (right to left).
    pub bits: [bool; METADATA_BITS],

    /// Mod 4 check sum from bits 0-1 (2 bits).
    ///
    /// The mod 4 check sum bits are obtained by adding the number of 1’s in bits 2
    /// through 31, then encoding the results of a mod 4 operation in bits 0 and 1.
    /// For example, if bits 2 through 31 have 18 1’s, bits 0 and 1 will hold the
    /// value 2 (18 mod 4 = 2).
    pub mod_4_checksum: u8,

    /// The mod 4 check sum computed from bits 2-31.
    pub computed_mod_4_checksum: u8,

    /// Batch or precinct number from bits 2-14 (13 bits).
    pub batch_or_precinct_number: u16,

    /// Card number (CardRotID) from bits 15-27 (13 bits).
    pub card_number: u16,

    /// Sequence number (always 0) from bits 28-30 (3 bits).
    pub sequence_number: u8,

    /// Start bit (always 1) from bit 31-31 (1 bit).
    pub start_bit: u8,
}

impl Debug for BallotCardMetadataFront {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FrontMetadata")
            .field("bits", &print_boolean_slice_as_binary(&self.bits))
            .field("mod_4_checksum", &self.mod_4_checksum)
            .field("computed_mod_4_checksum", &self.computed_mod_4_checksum)
            .field("batch_or_precinct_number", &self.batch_or_precinct_number)
            .field("card_number", &self.card_number)
            .field("sequence_number", &self.sequence_number)
            .field("start_bit", &self.start_bit)
            .finish()
    }
}

/// Metadata encoded by the bottom row of the back of a ballot card.
pub struct BallotCardMetadataBack {
    /// Raw bits 0-31 in LSB-MSB order (right-to-left).
    pub bits: [bool; METADATA_BITS],

    /// Election day of month (1..31) from bits 0-4 (5 bits).
    pub election_day: u8,

    /// Election month (1..12) from bits 5-8 (4 bits).
    pub election_month: u8,

    /// Election year (2 digits) from bits 9-15 (7 bits).
    pub election_year: u8,

    /// Election type from bits 16-20 (5 bits).
    ///
    /// @example "G" for general election
    pub election_type: u8,

    /// Ender code (binary 01111011110) from bits 21-31 (11 bits).
    pub ender_code: [bool; 11],

    /// Ender code (binary 01111011110) hardcoded to the expected value.
    pub expected_ender_code: [bool; 11],
}

impl Debug for BallotCardMetadataBack {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BackMetadata")
            .field("bits", &print_boolean_slice_as_binary(&self.bits))
            .field("election_day", &self.election_day)
            .field("election_month", &self.election_month)
            .field("election_year", &self.election_year)
            .field("election_type", &self.election_type)
            .field(
                "ender_code",
                &print_boolean_slice_as_binary(&self.ender_code),
            )
            .field(
                "expected_ender_code",
                &print_boolean_slice_as_binary(&self.expected_ender_code),
            )
            .finish()
    }
}

#[derive(Debug)]
pub enum BallotCardMetadata {
    Front(BallotCardMetadataFront),
    Back(BallotCardMetadataBack),
}

#[derive(Debug)]
pub enum BallotCardMetadataError {
    ValueOutOfRange {
        field: String,
        value: u32,
        min: u32,
        max: u32,
        metadata: BallotCardMetadata,
    },
    InvalidChecksum(BallotCardMetadataFront),
    InvalidEnderCode(BallotCardMetadataBack),
    InvalidTimingMarkCount {
        expected: usize,
        actual: usize,
    },
    AmbiguousMetadata {
        front_metadata: BallotCardMetadataFront,
        back_metadata: BallotCardMetadataBack,
    },
}

pub fn compute_bits_from_bottom_timing_marks(
    partial_timing_marks: &[Rect],
    complete_timing_marks: &[Rect],
) -> Result<[bool; METADATA_BITS], BallotCardMetadataError> {
    if complete_timing_marks.len() != 34 {
        return Err(BallotCardMetadataError::InvalidTimingMarkCount {
            expected: 34,
            actual: complete_timing_marks.len(),
        });
    }

    let mut bits = [false; METADATA_BITS];

    let mut partial_iter = partial_timing_marks.iter().rev();
    let mut complete_iter = complete_timing_marks.iter().rev();

    // Skip the last timing mark.
    complete_iter.next();
    partial_iter.next();

    let mut current_complete = complete_iter.next().unwrap();
    let mut current_partial = partial_iter.next().unwrap();

    for i in 0..METADATA_BITS {
        if current_complete == current_partial {
            bits[i] = true;
            current_partial = match partial_iter.next() {
                Some(partial) => partial,
                None => break,
            };
        }
        current_complete = complete_iter.next().unwrap();
    }

    Ok(bits)
}

pub fn decode_front_metadata_from_bits(
    bits_rtl: &[bool; METADATA_BITS],
) -> Result<BallotCardMetadataFront, BallotCardMetadataError> {
    let computed_mod_4_checksum = bits_rtl[2..]
        .iter()
        .map(|&bit| if bit { 1 } else { 0 })
        .sum::<u8>()
        % 4;

    let mod_4_checksum = bits_rtl[0..2]
        .iter()
        .rev()
        .fold(0, |acc, &bit| (acc << 1) + if bit { 1 } else { 0 });

    let batch_or_precinct_number = bits_rtl[2..15]
        .iter()
        .rev()
        .fold(0, |acc, &bit| (acc << 1) + if bit { 1 } else { 0 });

    let card_number = bits_rtl[15..28]
        .iter()
        .rev()
        .fold(0, |acc, &bit| (acc << 1) + if bit { 1 } else { 0 });

    let sequence_number = bits_rtl[28..31]
        .iter()
        .rev()
        .fold(0, |acc, &bit| (acc << 1) + if bit { 1 } else { 0 });

    let start_bit = if bits_rtl[31] { 1u8 } else { 0u8 };

    let front_metadata = BallotCardMetadataFront {
        bits: *bits_rtl,
        mod_4_checksum,
        computed_mod_4_checksum,
        batch_or_precinct_number,
        card_number,
        sequence_number,
        start_bit,
    };

    if computed_mod_4_checksum != mod_4_checksum {
        return Err(BallotCardMetadataError::InvalidChecksum(front_metadata));
    }

    if start_bit != 1 {
        return Err(BallotCardMetadataError::ValueOutOfRange {
            field: "start_bit".to_string(),
            value: start_bit as u32,
            min: 1,
            max: 1,
            metadata: BallotCardMetadata::Front(front_metadata),
        });
    }

    Ok(front_metadata)
}

pub fn decode_back_metadata_from_bits(
    bits_rtl: &[bool; METADATA_BITS],
) -> Result<BallotCardMetadataBack, BallotCardMetadataError> {
    let election_day = bits_rtl[0..5]
        .iter()
        .rev()
        .fold(0, |acc, &bit| (acc << 1) + if bit { 1 } else { 0 });

    let election_month = bits_rtl[5..9]
        .iter()
        .rev()
        .fold(0, |acc, &bit| (acc << 1) + if bit { 1 } else { 0 });

    let election_year = bits_rtl[9..16]
        .iter()
        .rev()
        .fold(0, |acc, &bit| (acc << 1) + if bit { 1 } else { 0 });

    let election_type = bits_rtl[16..21]
        .iter()
        .rev()
        .fold(0, |acc, &bit| (acc << 1) + if bit { 1 } else { 0 });

    let ender_code: [bool; 11] = bits_rtl[21..32]
        .try_into()
        .expect("slice with correct length");

    let back_metadata = BallotCardMetadataBack {
        bits: *bits_rtl,
        election_day,
        election_month,
        election_year,
        election_type,
        ender_code,
        expected_ender_code: ENDER_CODE,
    };

    if ender_code != ENDER_CODE {
        return Err(BallotCardMetadataError::InvalidEnderCode(back_metadata));
    }

    Ok(back_metadata)
}

pub fn decode_metadata_from_timing_marks(
    partial_timing_marks: &PartialTimingMarks,
    complete_timing_marks: &CompleteTimingMarks,
) -> Result<BallotCardMetadata, BallotCardMetadataError> {
    let bits = compute_bits_from_bottom_timing_marks(
        &partial_timing_marks.bottom_rects,
        &complete_timing_marks.bottom_rects,
    )?;

    let front_metadata_result = decode_front_metadata_from_bits(&bits);
    let back_metadata_result = decode_back_metadata_from_bits(&bits);

    match (front_metadata_result, back_metadata_result) {
        (Ok(front_metadata), Ok(back_metadata)) => {
            Err(BallotCardMetadataError::AmbiguousMetadata {
                front_metadata,
                back_metadata,
            })
        }
        (Ok(front_metadata), Err(_)) => Ok(BallotCardMetadata::Front(front_metadata)),
        (Err(_), Ok(back_metadata)) => Ok(BallotCardMetadata::Back(back_metadata)),
        (Err(front_metadata_error), Err(_)) => Err(front_metadata_error),
    }
}