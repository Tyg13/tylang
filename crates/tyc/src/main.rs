#![feature(let_chains)]
use clap::{clap_app, AppSettings};
use std::fs;

use ast::Node;

mod compiler;
//mod typeck;

#[derive(Debug)]
enum Error {
    ReadingInput(std::io::Error),
    UnknownAction(String),
    //TypeError(typeck::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReadingInput(err) => write!(f, "reading input: {err}"),
            Self::UnknownAction(action) => write!(f, "unknown action: {action}"),
            //Self::TypeError(err) => write!(f, "{err}"),
        }
    }
}

fn main() -> () {
    env_logger::init();
    || -> Result<(), Error> {
        let matches = clap_app!(tylang =>
            (version: "0.1")
            (author: "Tyler Lanphear")
            (@arg INPUT: +required "Input source file")
            (@arg ACTION: -a --action +takes_value "Action to take on input file")
            (@arg OPTIMIZE: -O --optimize "Whether to optimize output")
            (@arg QUIET: -q --quiet "Whether to suppress output (if applicable)")
        )
        .setting(AppSettings::ArgRequiredElseHelp)
        .get_matches();
        let action = matches.value_of("ACTION");
        let input_path = matches.value_of("INPUT").unwrap();
        let optimize = matches.is_present("OPTIMIZE");
        let quiet = matches.is_present("QUIET");

        let module_string = read_source(input_path)?;

        if let Some("none") = action {
            return Ok(());
        }

        let module_lexed = cst::parser::Input::lex(&module_string);
        if let Some("tokens") = action {
            if !quiet {
                println!("{:#?}", module_lexed.tokens());
            }
            return Ok(());
        }

        let module_cst = cst::parser::parse(module_lexed);
        if let Some("cst") = action {
            if !quiet {
                pretty_print(&module_cst);
            }
            return Ok(());
        }

        if !module_cst.errors.is_empty() {
            if !quiet {
                for error in module_cst.errors {
                    println!("{}", error);
                }
                println!("{}", module_cst.root);
            }
            return Ok(());
        }

        let module_ast = ast::Module::cast(module_cst.root.clone()).unwrap();
        if let Some("ast") = action {
            if !quiet {
                println!("{}", module_ast);
            }
            return Ok(());
        }

        let module_bir = bir::translate::module(&module_ast).unwrap();

        //typeck::check(&module, &source).map_err(Error::TypeError)?;
        //if let Some("check") = action {
        //    return Ok(());
        //}

        if let Some("bir") = action {
            if !quiet {
                println!("{:#?}", module_bir);
            }
            return Ok(());
        }

        let module_lir = lir::translate::translate_module(&module_bir);

        let action = match action {
            None | Some("compile") => compiler::Action::WriteExecutable,
            Some("lir") => {
                if !quiet {
                    println!("{}", lir::printers::to_string(&module_lir));
                }
                return Ok(());
            }
            Some("llvm-ir") => compiler::Action::WriteIr,
            Some("asm") => compiler::Action::WriteAssembly,
            Some("obj") => compiler::Action::WriteObject,
            Some(action) => return Err(Error::UnknownAction(action.to_string())),
        };
        compiler::compile(&module_lir, input_path, action, optimize);
        Ok(())
    }()
    .unwrap_or_else(|e| eprintln!("error: {e}"));
}

fn read_source(input_path: &str) -> Result<String, Error> {
    if input_path == "-" {
        let mut input = String::new();
        use std::io::Read;
        std::io::stdin()
            .read_to_string(&mut input)
            .map_err(Error::ReadingInput)?;
        Ok(input)
    } else {
        fs::read_to_string(input_path).map_err(Error::ReadingInput)
    }
}

fn pretty_print(output: &cst::parser::Output) {
    println!("{}", output.root);
}
