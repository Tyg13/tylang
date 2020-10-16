#![feature(or_patterns)]

use clap::{clap_app, AppSettings};
use std::fs;

mod ast;
mod bir;
mod compiler;
mod lexer;
//mod typeck;
mod util;

//mod pika;

use util::SourceBuilder;

#[derive(Debug)]
enum Error {
    ReadingInput(std::io::Error),
    UnknownAction(String),
}

fn main() -> Result<(), Error> {
    let matches = clap_app!(tylang =>
        (version: "0.1")
        (author: "Tyler Lanphear")
        (@arg INPUT: +required "Input source file")
        (@arg ACTION: -a --action +takes_value "Action to take on input file")
    )
    .setting(AppSettings::ArgRequiredElseHelp)
    .get_matches();
    env_logger::init();
    let input_path = matches.value_of("INPUT").unwrap();
    let mut input = String::new();
    if input_path == "-" {
        use std::io::Read;
        std::io::stdin()
            .read_to_string(&mut input)
            .map_err(Error::ReadingInput)?;
    } else {
        input = fs::read_to_string(input_path).map_err(Error::ReadingInput)?;
    }
    let source = SourceBuilder::new().file(input_path).source(input).build();
    let tokens = lexer::lex(&source);
    match matches.value_of("ACTION") {
        Some("print") => return Ok(pretty_print(&tokens)),
        _ => {
            let tree = ast::parse(&source, tokens, &mut std::io::stdout());
            if tree.valid() {
                let action = match matches.value_of("ACTION") {
                    None | Some("compile") => compiler::Action::WriteObject,
                    Some("ir") => compiler::Action::WriteIr,
                    Some("asm") => compiler::Action::WriteAssembly,
                    Some(action) => return Err(Error::UnknownAction(action.to_string())),
                };
                compiler::compile(&tree, &source, action);
            }
        }
    }
    Ok(())
}

fn pretty_print(map: &lexer::TokenMap) {
    for token in map.tokens() {
        print!("{}", token);
    }
}
