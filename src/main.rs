use clap::{clap_app, AppSettings};
use std::fs;

mod interpret;
mod lexer;
mod parser;
mod util;

use util::SourceBuilder;

#[derive(Debug)]
enum Error {
    ReadingInput(std::io::Error),
}

fn main() -> Result<(), Error> {
    let matches = clap_app!(tylang =>
        (version: "0.0")
        (author: "Tyler Lanphear")
        (@arg INPUT: +required "Source file to compile")
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
    let source = SourceBuilder::new().file(input_path).lines(input).build();
    let tokens = lexer::lex(&source);
    let tree = parser::parse(&source, tokens);
    interpret::interpret(tree, &mut std::io::stdout());
    Ok(())
}
