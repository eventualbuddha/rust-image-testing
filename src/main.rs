extern crate log;
extern crate pretty_env_logger;

use std::env;
use std::path::Path;

use clap::{arg, command, Command};
use logging_timer::{finish, timer};

use crate::ballot_card::load_oval_template;
use crate::election::Election;
use crate::interpret::{InterpretOptions, interpret_ballot_card};

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
        .expect("side A image path");
    let side_b_path = matches
        .get_one::<String>("side_b_path")
        .expect("side B image path");
    let election_definition_path = matches
        .get_one::<String>("election")
        .expect("election path");

    // parse contents of election_definition_path with serde_json
    let election: Election = match serde_json::from_str(
        &std::fs::read_to_string(election_definition_path).expect("election file"),
    ) {
        Ok(election_definition) => election_definition,
        Err(e) => {
            panic!("Error parsing election definition: {}", e);
        }
    };

    println!("Election: {:?}", election);

    let oval_template = match load_oval_template() {
        Some(image) => image,
        None => {
            panic!("Error loading oval scan image");
        }
    };

    let options = InterpretOptions {
        debug,
        oval_template,
        election,
    };
    let timer = timer!("total");

    match interpret_ballot_card(Path::new(&side_a_path), Path::new(&side_b_path), &options) {
        Ok((front, back)) => {
            println!("Front: {:?}", front);
            println!("Back: {:?}", back);
        }
        Err(e) => {
            println!("Error: {:?}", e);
        }
    }

    finish!(timer);
}

fn cli() -> Command {
    command!()
        .arg(arg!(-e --election <PATH> "Path to election.json file").required(true))
        .arg(arg!(-d --debug "Enable debug mode"))
        .arg(arg!(side_a_path: <SIDE_A_IMAGE> "Path to image for side A").required(true))
        .arg(arg!(side_b_path: <SIDE_B_IMAGE> "Path to image for side B").required(true))
}
