use super::*;

#[derive(Clone, PartialEq, Debug)]
pub(super) enum Error {
    UnexpectedToken(String),
    Internal(String),
    EOF,
}
