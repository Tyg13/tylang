#![feature(associated_type_defaults)]

use clap::{clap_app, AppSettings};
use std::fs;

mod ast;
mod compiler;
mod lexer;
mod util;

use util::SourceBuilder;

#[derive(Debug)]
enum Error {
    ReadingInput(std::io::Error),
    UnknownAction(String),
}

fn main() -> Result<(), Error> {
    let matches = clap_app!(tylang =>
        (version: "0.0")
        (author: "Tyler Lanphear")
        (@arg INPUT: +required "Input source file")
        (@arg ACTION: -a --action +takes_value "Action to take on input file")
    )
    .setting(AppSettings::ArgRequiredElseHelp)
    .get_matches();
    let input_path = matches.value_of("INPUT").unwrap();
    let mut input = String::new();
    if input_path == "-" {
        use std::io::Read;
        std::io::stdin()
            .read_to_string(&mut input)
            .map_err(|e| Error::ReadingInput(e))?;
    } else {
        input = fs::read_to_string(input_path).map_err(|e| Error::ReadingInput(e))?;
    }
    let source = SourceBuilder::new().file(input_path).source(input).build();
    let tokens = lexer::lex(&source);
    let tree = ast::parse(&source, tokens, &mut std::io::stdout());
    if tree.valid() {
        match matches.value_of("ACTION") {
            None | Some("compile") => compiler::compile(&tree, &source),
            Some(action) => return Err(Error::UnknownAction(action.to_string())),
        };
    }
    Ok(())
}
