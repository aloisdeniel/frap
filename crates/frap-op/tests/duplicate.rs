// SPDX-License-Identifier: GPL-3.0-or-later
//! Two `OperatorDef`s with the same idname registered through `linkme` make
//! `Registry::global()` fail (its own test binary, so the other tests keep a valid registry).

mod common;

use std::panic::{AssertUnwindSafe, catch_unwind};

use common::dummy;
use frap_op::prelude::*;
use frap_op::{Registry, operator};

#[operator(poll = poll::ALWAYS, tier = T0)]
fn first(_cx: &mut OpCtx, _p: dummy::Dup) -> OpResult {
    Ok(OpStatus::Finished)
}

#[operator(poll = poll::ALWAYS, tier = T0)]
fn second(_cx: &mut OpCtx, _p: dummy::Dup) -> OpResult {
    Ok(OpStatus::Finished)
}

#[test]
fn duplicate_idnames_make_the_registry_fail() {
    assert_eq!(
        OPERATORS
            .iter()
            .filter(|d| d.spec.idname == "dummy.dup")
            .count(),
        2
    );
    let e = Registry::try_global()
        .err()
        .expect("registry must not build");
    assert_eq!(
        e,
        "duplicate operator registration: dummy.dup (2 OperatorDefs share this idname)"
    );
    let panic = catch_unwind(AssertUnwindSafe(|| {
        let _ = Registry::global();
    }))
    .expect_err("Registry::global() panics");
    let msg = panic.downcast_ref::<String>().cloned().unwrap_or_default();
    assert!(
        msg.contains("duplicate operator registration: dummy.dup"),
        "{msg}"
    );
    // Every entry point that needs the registry fails the same way.
    assert!(catch_unwind(|| frap_op::lookup("dummy.dup").is_ok()).is_err());
}
