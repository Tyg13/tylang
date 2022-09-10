use clap::{clap_app, AppSettings};
use std::fs;

use ast::Node;

mod codegen;

#[derive(Debug)]
enum Error {
    ReadingInput(std::io::Error),
    UnknownAction(String),
    SemanticErrors(usize),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReadingInput(err) => write!(f, "reading input: {err}"),
            Self::UnknownAction(action) => write!(f, "unknown action: {action}"),
            Self::SemanticErrors(n) => write!(f, "{n} semantic errors"),
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
        let module_source = utils::Source::read_path(input_path);

        if let Some("none") = action {
            return Ok(());
        }

        let mut module_ctx = ModuleCtx {
            source: &module_source,
            bir: None,
            sema: None,
        };

        let module_lexed = cst::parser::Input::lex(&module_string);
        if let Some("tokens") = action {
            if !quiet {
                println!("{:#?}", module_lexed.tokens());
            }
            return Ok(());
        }

        let module_cst = cst::parser::parse(module_lexed);
        if !module_cst.errors.is_empty() {
            if !quiet {
                for error in module_cst.errors {
                    let error = module_ctx.pos_ctx_with_label(error.pos.offset, &error.msg);
                    eprintln!("{error}");
                }
            }
            return Ok(());
        }
        if let Some("cst") = action {
            if !quiet {
                pretty_print(&module_cst);
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

        let module_bir = bir::translate::ast(&module_ast);
        if let Some("bir") = action {
            if !quiet {
                eprintln!("{module_bir:#?}");
                bir::print(&module_bir);
            }
            return Ok(());
        }
        module_ctx.bir = Some(&module_bir);

        let module_sema = sema::check::check(&module_bir);
        module_ctx.sema = Some(&module_sema);

        if let Some("sema") = action {
            if !quiet {
                let map = &module_sema;
                for (id, kind) in map.nodes() {
                    let label = match kind {
                        sema::Kind::Type => map.ty(id).unwrap().repr(map),
                        sema::Kind::Module => continue,
                        sema::Kind::Function => continue,
                        sema::Kind::Param => continue,
                        sema::Kind::Var => continue,
                        sema::Kind::Block => continue,
                        sema::Kind::Error => continue,
                        sema::Kind::Constant => map.ty(id).unwrap().repr(map),
                        sema::Kind::Other => map.ty(id).unwrap().repr(map),
                        sema::Kind::Tombstone => continue,
                    };
                    println!("{}\n", module_ctx.sema_ctx_with_label(&id, &label));
                }
            }
            report_sema_errs(&module_sema, &module_ctx);
            return Ok(());
        }

        let num_sema_errors = report_sema_errs(&module_sema, &module_ctx);
        if num_sema_errors > 0 {
            return Err(Error::SemanticErrors(num_sema_errors));
        }

        let module_lir = lir::translate(&module_bir, &module_sema);
        if let Some("lir") = action {
            lir::print(&module_sema, &module_lir);
            return Ok(());
        }

        let action = match action {
            None | Some("compile") => codegen::Action::WriteExecutable,
            Some("llvm-ir") => codegen::Action::WriteIr,
            Some("asm") => codegen::Action::WriteAssembly,
            Some("obj") => codegen::Action::WriteObject,
            Some(action) => return Err(Error::UnknownAction(action.to_string())),
        };
        codegen::compile(&module_lir, &module_sema, input_path, action, optimize);
        Ok(())
    }()
    .unwrap_or_else(|e| eprintln!("error: {e}"));
}

fn report_sema_errs(module_sema: &sema::Map, module_ctx: &ModuleCtx) -> usize {
    let mut num_sema_errors = 0;
    for err in module_sema.errors() {
        num_sema_errors += 1;
        report_sema_err(module_ctx, err);
    }
    num_sema_errors
}

fn report_sema_err(ctx: &ModuleCtx, err: &sema::errors::Error) {
    use sema::errors::ErrorKind;
    eprintln!(
        "{}",
        match err.kind {
            ErrorKind::DuplicateBinding => {
                let id = &err.ids[0];
                ctx.sema_ctx_with_label(id, &format!("duplicate binding: {}", ctx.text_of(id)))
            }
            ErrorKind::UnknownType => {
                let id = &err.ids[0];
                ctx.sema_ctx_with_label(&id, &format!("unknown type: `{}`", ctx.text_of(id)))
            }
            ErrorKind::UnknownName => {
                let id = &err.ids[0];
                ctx.sema_ctx_with_label(&id, &format!("unknown name: `{}`", ctx.text_of(id)))
            }
            ErrorKind::DuplicateType => {
                let id = &err.ids[0];
                ctx.sema_ctx_with_label(&id, &format!("redefined type: `{}`", ctx.text_of(id)))
            }
            ErrorKind::UnknownCall => {
                let id = &err.ids[0];
                ctx.sema_ctx_with_label(&id, &format!("unknown call to `{}`", ctx.text_of(id)))
            }
            ErrorKind::Unification => {
                let (a, b) = (&err.ids[0], &err.ids[1]);
                format!(
                    "Can't unify:\n{}\n\n{}",
                    ctx.sema_ctx_with_label(a, &ctx.type_of(a)),
                    ctx.sema_ctx_with_label(b, &ctx.type_of(b)),
                )
            }
            ErrorKind::InvalidIndexType => {
                let (a, b) = (&err.ids[0], &err.ids[1]);
                format!(
                    "Can't index pointer with non-integer type!\n{}\n\n{}",
                    ctx.sema_ctx_with_label(a, &ctx.type_of(a)),
                    ctx.sema_ctx_with_label(b, &ctx.type_of(b)),
                )
            }
            ErrorKind::InvalidPointeeType => {
                let expr = &err.ids[0];
                format!(
                    "Can't dereference non-pointer!\n{}",
                    ctx.sema_ctx_with_label(expr, &ctx.type_of(expr)),
                )
            }
            ErrorKind::ParamAssignment => {
                let expr = &err.ids[0];
                format!(
                    "Can't assign to param!\n{}",
                    ctx.sema_ctx_with_label(expr, &ctx.type_of(expr)),
                )
            }
        }
    );
}

struct ModuleCtx<'ctx> {
    source: &'ctx utils::Source,
    bir: Option<&'ctx bir::Map>,
    sema: Option<&'ctx sema::Map>,
}

impl ModuleCtx<'_> {
    fn bir(&self) -> &bir::Map {
        self.bir.unwrap()
    }

    fn sema(&self) -> &sema::Map {
        self.sema.unwrap()
    }

    fn pos_ctx_with_label(&self, pos: usize, label: &str) -> String {
        self.range_ctx_with_label(pos..pos, label)
    }

    fn range_ctx_with_label(&self, range: std::ops::Range<usize>, label: &str) -> String {
        self.source
            .span_for(range)
            .and_then(|span| {
                self.source.give_context_span_and_label(
                    span,
                    utils::HandPosition::WholeSpan,
                    Some(label),
                )
            })
            .unwrap_or_else(|| "[err getting context]".to_string())
    }

    fn sema_ctx_with_label(&self, id: &sema::ID, label: &str) -> String {
        self.syntax_of(id)
            .map(|node| self.range_ctx_with_label(node.range(), label))
            .unwrap_or_else(|| format!("{}\n[err getting context] {:?}", label, id))
    }

    fn type_of(&self, id: &sema::ID) -> String {
        self.sema()
            .ty(*id)
            .map(|ty| ty.repr(self.sema()))
            .unwrap_or_else(|| {
                debug_assert_eq!(self.sema().kind(*id), sema::Kind::Error);
                "<err>".to_string()
            })
    }

    fn text_of(&self, id: &sema::ID) -> String {
        self.syntax_of(id).map(|node| node.text()).unwrap()
    }

    fn syntax_of(&self, id: &sema::ID) -> Option<cst::syntax::Node> {
        let bir_id = self.sema().bir(*id)?;
        self.bir().ast(&bir_id).map(|node| node.syntax().clone())
    }
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
