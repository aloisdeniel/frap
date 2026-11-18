// SPDX-License-Identifier: GPL-3.0-or-later
//! The `frap_op::DESCRIBE_PROVIDERS` entry that serves `bpy.ops.frap.describe(target=…)` from
//! the manifest of this crate (M2 W0 contract delta, ARCHITECTURE §14; implemented by WP 2.13).
//!
//! The manifest lives here (L11), above the operator crates (L10), so this crate registers the
//! provider and `frap-ops-frap/src/ops/describe.rs` calls it through
//! [`frap_op::describe_provider`]. A target is parsed exactly as `frap --describe=<target>`
//! and `frap_describe(target)` parse it ([`Target::parse`]) and answered with the default
//! [`DescribeOptions`] (no `--filter`, every operator), so the operator, the CLI flag and the
//! MCP tool return the same fragment. An unknown target, operator, struct or member is
//! `KeyError` with code `DESCRIBE_TARGET` and `detail.did_you_mean`.

use frap_rna::FrapError;
use serde_json::Value as J;

use crate::manifest::{DescribeOptions, Target, describe};

/// The `--describe` fragment of `target` (`op:mesh.bevel`, `mesh.bevel`, `struct:Object`,
/// `Object.location`, `errors`, …), or `DESCRIBE_TARGET`.
pub fn describe_target(target: &str) -> Result<J, FrapError> {
    let t = Target::parse(target).map_err(|e| e.into_frap())?;
    describe(&t, &DescribeOptions::default()).map_err(|e| e.into_frap())
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_op::linkme::distributed_slice(frap_op::DESCRIBE_PROVIDERS)]
#[linkme(crate = frap_op::linkme)]
static PROVIDER: frap_op::DescribeProvider = frap_op::DescribeProvider {
    describe: describe_target,
};

/// The registered entry. A front-end that links `frap-schema` only for `frap.describe` (and
/// never calls the manifest itself) calls this once so the linker keeps the [`PROVIDER`]
/// entry, as `frap_ops_all::link()` does for the operators; it has no other effect. `frap`
/// (the CLI) already uses the manifest for `--describe` and `frap_describe`.
#[inline(never)]
pub fn link() -> &'static frap_op::DescribeProvider {
    &PROVIDER
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registered_provider_is_this_one() {
        let p = frap_op::describe_provider().expect("frap-schema registers the provider");
        assert!(std::ptr::eq(p, link()));
        assert_eq!(frap_op::DESCRIBE_PROVIDERS.len(), 1);
    }

    #[test]
    fn fragments_equal_the_describe_targets() {
        for t in [
            "op:mesh.bevel",
            "mesh.bevel",
            "bpy.ops.mesh.bevel",
            "struct:Object",
            "Object",
            "Object.location",
            "Object.select_set",
            "errors",
        ] {
            let want = describe(&Target::parse(t).unwrap(), &DescribeOptions::default()).unwrap();
            assert_eq!(describe_target(t).unwrap(), want, "{t}");
        }
        assert_eq!(
            describe_target("mesh.bevel").unwrap()["idname"],
            "mesh.bevel"
        );
    }

    /// `bpy.ops.frap.describe` through a real session (both spellings), against the provider
    /// and the published response schema.
    #[test]
    fn operator_answers_the_fragment_in_a_session() {
        frap_ops_all::link();
        let mut s =
            frap_session::Session::new(frap_session::SessionOptions::default()).expect("session");
        let rs = s.exec_source_at(
            "bpy.ops.frap.describe(target='op:mesh.bevel')\n\
             frap.describe target=Object.location\n\
             bpy.ops.frap.describe(target='struct:Objcet')\n\
             bpy.ops.frap.describe()\n\
             bpy.ops.frap.describe(target='errors')\n",
            1,
        );
        assert_eq!(rs.len(), 5);
        let schema = crate::jsonschema::protocol();
        let js: Vec<J> = rs.iter().map(|r| r.to_json()).collect();
        for j in &js {
            crate::jsonschema::validate::validate(&schema["response"], j)
                .unwrap_or_else(|e| panic!("{e:?}\n{j:#}"));
        }
        for (j, target) in [
            (&js[0], "op:mesh.bevel"),
            (&js[1], "Object.location"),
            (&js[4], "errors"),
        ] {
            assert_eq!(j["status"], "FINISHED", "{j:#}");
            assert_eq!(j["op"], "frap.describe");
            assert_eq!(j["result"]["target"], target);
            assert_eq!(j["result"]["describe"], describe_target(target).unwrap());
            assert_eq!(j["changes"]["modified"], serde_json::json!([]));
        }
        assert_eq!(js[0]["result"]["describe"]["frap"]["status"], "implemented");
        assert_eq!(js[1]["result"]["describe"]["unit"], "LENGTH");
        let e = &js[2]["error"];
        assert_eq!(
            (&e["exception"], &e["code"]),
            (&J::from("KeyError"), &J::from("DESCRIBE_TARGET"))
        );
        assert_eq!(e["detail"]["did_you_mean"][0], "Object");
        assert_eq!(js[2]["state"], "unchanged");
        let e = &js[3]["error"];
        assert_eq!(
            (&e["exception"], &e["code"]),
            (&J::from("TypeError"), &J::from("OP_ARG_TYPE"))
        );
    }

    #[test]
    fn unknown_targets_are_describe_target_key_errors() {
        for (t, close) in [
            ("op:mesh.bevl", "mesh.bevel"),
            ("mesh.bevl", "mesh.bevel"),
            ("struct:Objcet", "Object"),
            ("Object.locaton", "location"),
            ("jsonshema", "jsonschema"),
        ] {
            let e = describe_target(t).unwrap_err();
            assert_eq!(e.code.0, "DESCRIBE_TARGET", "{t}");
            assert_eq!(e.exception, frap_rna::PyExc::KeyError, "{t}");
            let dym = e.detail["did_you_mean"].as_array().expect("did_you_mean");
            assert!(dym.iter().any(|v| v == close), "{t}: {dym:?}");
        }
    }
}
