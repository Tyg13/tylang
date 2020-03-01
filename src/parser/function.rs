use super::*;

struct Function {
    span: Span,
    parameters: Vec<Variable>,
    body: Scope,
}
