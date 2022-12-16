extern crate log;
extern crate pretty_env_logger;

use std::env;
use std::path::Path;
use std::process::exit;

use clap::{arg, command, Command};

use crate::ballot_card::load_oval_template;
use crate::election::Election;
use crate::interpret::{interpret_ballot_card, Options};

mod ballot_card;
mod debug;
mod election;
mod geometry;
mod image_utils;
mod interpret;
mod metadata;
mod timing_marks;
mod types;

fn main() {
    pretty_env_logger::init_custom_env("LOG");

    let matches = cli().get_matches();
    let debug = matches.get_flag("debug");
    let side_a_path = matches
        .get_one::<String>("side_a_path")
        .expect("side A image path is required");
    let side_b_path = matches
        .get_one::<String>("side_b_path")
        .expect("side B image path is required");
    let election_definition_path = matches
        .get_one::<String>("election")
        .expect("election path is required");

    let election_definition_json = match std::fs::read_to_string(election_definition_path) {
        Ok(json) => json,
        Err(e) => {
            eprintln!("Error reading election definition: {}", e);
            exit(1);
        }
    };

    // parse contents of election_definition_path with serde_json
    let election: Election = match serde_json::from_str(&election_definition_json) {
        Ok(election_definition) => election_definition,
        Err(e) => {
            eprintln!("Error parsing election definition: {}", e);
            exit(1);
        }
    };

    let oval_template = load_oval_template().map_or_else(
        || {
            eprintln!("Error loading oval template");
            exit(1);
        },
        |image| image,
    );

    let options = Options {
        debug,
        oval_template,
        election,
    };

    match interpret_ballot_card(Path::new(&side_a_path), Path::new(&side_b_path), &options) {
        Ok((front, back)) => {
            println!("Front: {:?}", front);
            println!("Back: {:?}", back);
        }
        Err(e) => {
            eprintln!("Error: {:?}", e);
            exit(1);
        }
    }
}

#[allow(clippy::cognitive_complexity)]
fn cli() -> Command {
    command!()
        .arg(arg!(-e --election <PATH> "Path to election.json file").required(true))
        .arg(arg!(-d --debug "Enable debug mode"))
        .arg(arg!(side_a_path: <SIDE_A_IMAGE> "Path to image for side A").required(true))
        .arg(arg!(side_b_path: <SIDE_B_IMAGE> "Path to image for side B").required(true))
}
