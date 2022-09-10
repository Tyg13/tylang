use crate::{ast, declare_new_intern_id, util::intern_map::InternMap};

// How does type checking work? Ultimately we have two main sources of types in a block
// First: we have _variables_ themselves. These are introduced by a declaration, which
// has an optional explicit type, and an optional initializer. A variable's initializer is
// an example of the second type of source, _expressions_. Expressions can live without an
// associated variable, but typically are the child of an assignment or initializer node.
// NOTE: As of right now, we have no concept of indirection, so we don't need to concern
// ourselves with expressions on the left hand side of an assignment/declaration
//
// Everything in our source tree has some kind of flow, in or out
//
// Consider
// ```ty
// fn foo(n: i64) -> i64 {
//    return n + 1;
// }
// ```
//
// In this example, we have one scope, with one statement: a return statement.
// This return statement has one flow in: the expression `n + 1`.
// Additionally, it flows out: expected return type of the function is `i64`.
//
// The expression `n + 1` has one flow in: the parameter `n` of type `i64`.
// Additionally, it flows out: into the return statement.
//
// Occasionally an expression will not flow out:
// ```ty
// fn foo(n: i64) -> i64 {
//    n + 1;
//    return n;
// }
// ```
//
// In this example, `n + 1`'s result does not flow anywhere.
//
// So in order to correctly determine the type of intermediate expressions, we
// use the following method:
//
// Create a list of TypeSources. Populate it with the function's current parameters.
// Iterate over each statement and collect TypeSources, with either known or unknown type.
// If a TypeSource has an unknown type, assign it a fresh unknown type.
//
// We'll have a list like
// [(n, Some(i64), [], [n + 1]),
//  (return n + 1, Some(i64), [n, 1]),
//  (n + 1, None, [n], [return n + 1]),
//  (1, Some(integer), [], [n + 1])]
//
// For each TypeSource that has None type, we check its InFlow and OutFlow lists for known types.
// They should all unify, and their unification will be assigned as the new type.
