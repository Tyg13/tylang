use clap::Parser;
use std::fs;
use std::sync::Arc;

use ast::Node;

#[derive(Debug)]
enum Error {
    ReadingInput(std::io::Error),
    UnknownAction(String),
    SemanticErrors(usize),
    BuildingCST,
    ParsingAST,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReadingInput(err) => write!(f, "reading input: {err}"),
            Self::UnknownAction(action) => {
                write!(f, "unknown action: {action}")
            }
            Self::SemanticErrors(n) => write!(f, "{n} semantic errors"),
            Self::BuildingCST => write!(f, "building CST"),
            Self::ParsingAST => write!(f, "parsing AST"),
        }
    }
}

#[derive(Parser, Debug)]
#[clap(author = "Tyler Lanphear", version = "0.1", about = "tylang compiler")]
struct Args {
    input: String,
    #[clap(short, long)]
    action: Option<String>,
    #[clap(short, long)]
    output_path: Option<String>,
    #[clap(long)]
    optimize: bool,
    #[clap(short, long)]
    quiet: bool,
}

fn main() -> () {
    env_logger::init();
    || -> Result<(), Error> {
        let args = Args::parse();
        let action = args.action.as_deref();

        let module_string = read_source(&args.input)?;
        let module_source = utils::Source::read_path(&args.input);

        if let Some("none") = action {
            return Ok(());
        }

        let mut module_ctx = ModuleCtx {
            source: &module_source,
            bir: None,
            sema: None,
        };

        let module_lexed = parser::Input::lex(&module_string);
        if let Some("tokens") = action {
            if !args.quiet {
                println!("{:#?}", module_lexed.tokens());
            }
            return Ok(());
        }

        let module_cst = parser::parse(module_lexed);
        if !module_cst.errors.is_empty() {
            if !args.quiet {
                for error in module_cst.errors {
                    let error = module_ctx
                        .pos_ctx_with_label(error.pos.offset, &error.msg);
                    eprintln!("{error}");
                }
            }
            return Ok(());
        }
        if let Some("cst") = action {
            if !args.quiet {
                pretty_print(&module_cst);
            }
            return Ok(());
        }

        let module_ast = ast::Module::cast(module_cst.root.clone()).unwrap();
        if let Some("ast") = action {
            if !args.quiet {
                println!("{}", module_ast);
            }
            return Ok(());
        }

        let module_bir = {
            struct AstBuilder;
            impl bir::translate::AstBuilder for AstBuilder {
                type Error = crate::Error;
                fn build(
                    &mut self,
                    module_name: &str,
                ) -> Result<Arc<ast::Module>, Error> {
                    parse_ast(&format!("{module_name}.ty"))
                }
            }
            bir::translate::ast(&module_ast, &mut AstBuilder)
        };
        if let Some("bir") = action {
            if !args.quiet {
                bir::print(&module_bir);
            }
            return Ok(());
        }
        module_ctx.bir = Some(&module_bir);

        let module_sema = sema::check::check(&module_bir);
        module_ctx.sema = Some(&module_sema);

        if let Some("sema") = action {
            if !args.quiet {
                let map = &module_sema;
                for (id, kind) in map.nodes() {
                    let repr = match kind {
                        sema::Kind::Module
                        | sema::Kind::Block
                        | sema::Kind::Error => continue,
                        sema::Kind::Type
                        | sema::Kind::Function
                        | sema::Kind::Param
                        | sema::Kind::Var
                        | sema::Kind::Constant
                        | sema::Kind::TypeMember
                        | sema::Kind::Expr => match map.ty(id) {
                            Some(ty) => ty.repr(map),
                            None => continue,
                        },
                        sema::Kind::Tombstone => unreachable!(),
                    };
                    let label = format!("{repr} {:?}", id);
                    println!(
                        "{}\n",
                        module_ctx.sema_ctx_with_label(&id, &label)
                    );
                }
            }
            report_sema_errs(&module_sema, &module_ctx);
            return Ok(());
        }

        let num_sema_errors = report_sema_errs(&module_sema, &module_ctx);
        if num_sema_errors > 0 {
            return Err(Error::SemanticErrors(num_sema_errors));
        }

        let mut module_lir = lir::translate(&module_bir, &module_sema);
        if let Some("lir") = action {
            lir::print(&module_lir);
            if args.optimize {
                lir::pass::run_pass(
                    &mut module_lir,
                    &mut lir::passes::JumpThreading,
                );
                lir::pass::run_pass(&mut module_lir, &mut lir::passes::DCE);
            }
            return Ok(());
        }

        let action = match action {
            None | Some("compile") => codegen::Action::WriteExecutable,
            Some("llvm-ir") => codegen::Action::WriteIr,
            Some("asm") => codegen::Action::WriteAssembly,
            Some("obj") => codegen::Action::WriteObject,
            Some(action) => {
                return Err(Error::UnknownAction(action.to_string()));
            }
        };
        codegen::compile(
            &module_lir,
            &args.input,
            args.output_path.as_deref(),
            action,
            args.optimize,
        );
        Ok(())
    }()
    .unwrap_or_else(|e| {
        eprintln!("error: {e}");
        std::process::exit(1)
    });
}

fn parse_ast(input: &str) -> Result<Arc<ast::Module>, Error> {
    let module_string = read_source(input)?;
    let module_lexed = parser::Input::lex(&module_string);
    let module_cst = parser::parse(module_lexed);
    if !module_cst.errors.is_empty() {
        return Err(Error::BuildingCST);
    }

    ast::Module::cast(module_cst.root.clone()).ok_or(Error::ParsingAST)
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
                ctx.sema_ctx_with_label(
                    id,
                    &format!("duplicate binding: {}", ctx.text_of(id)),
                )
            }
            ErrorKind::UnknownType => {
                let id = &err.ids[0];
                ctx.sema_ctx_with_label(
                    &id,
                    &format!("unknown type: `{}`", ctx.text_of(id)),
                )
            }
            ErrorKind::UnknownName => {
                let id = &err.ids[0];
                ctx.sema_ctx_with_label(
                    &id,
                    &format!("unknown name: `{}`", ctx.text_of(id)),
                )
            }
            ErrorKind::DuplicateType => {
                let id = &err.ids[0];
                ctx.sema_ctx_with_label(
                    &id,
                    &format!("redefined type: `{}`", ctx.text_of(id)),
                )
            }
            ErrorKind::UnknownCall => {
                let id = &err.ids[0];
                ctx.sema_ctx_with_label(
                    &id,
                    &format!("unknown call to `{}`", ctx.text_of(id)),
                )
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
            ErrorKind::CallToNonFnType => {
                let expr = &err.ids[0];
                format!(
                    "Not a function type!\n{}",
                    ctx.sema_ctx_with_label(expr, &ctx.type_of(expr)),
                )
            }
            ErrorKind::InvalidField => {
                let expr = &err.ids[0];
                format!(
                    "Invalid field: `{}`",
                    ctx.sema_ctx_with_label(expr, &ctx.type_of(expr)),
                )
            }
            ErrorKind::InvalidCallReceiver => {
                let expr = &err.ids[0];
                let kind = ctx.syntax_of(expr).unwrap().kind();
                format!("Cannot call field with: `{kind:?}`")
            }
            ErrorKind::InvalidFieldReceiver => {
                let expr = &err.ids[0];
                let ty = ctx.type_of(expr);
                format!(
                    "Cannot index into `{ty}` as a struct:\n{}",
                    ctx.sema_ctx_with_label(expr, &ty)
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

    fn range_ctx_with_label(
        &self,
        range: std::ops::Range<usize>,
        label: &str,
    ) -> String {
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
            .unwrap_or_else(|| {
                format!("{}\n[err getting context] {:?}", label, id)
            })
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
        let ast = self.bir().ast(&bir_id)?;
        Some(ast.syntax().clone())
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

fn pretty_print(output: &parser::Output) {
    println!("{}", output.root);
}
