use crate::builder::{Builder, Session};
use crate::types::ValueRef;

pub(crate) fn lookup_name(sess: &Session, builder: &Builder, name: &str) -> ValueRef {
    let ns = sess.sema.ns(builder.current_ns()).unwrap();
    let name_id = ns.lookup(sess.sema, name).unwrap().id;
    let global = match sess.sema.kind(name_id) {
        sema::Kind::Function => true,
        sema::Kind::Param | sema::Kind::Var => false,
        _ => unreachable!(),
    };
    builder.val_from_sema(&name_id, global).copied().unwrap()
}
