// SPDX-License-Identifier: GPL-3.0-or-later
//! `--describe[=<target>]` and `frap_describe(target)`: target parsing and dispatch (§14).
//!
//! | target | output |
//! |---|---|
//! | `manifest` (default, also empty) | [`super::manifest`] |
//! | `jsonschema` | one flattened JSON Schema per operator input + the protocol envelope |
//! | `tools-anthropic`, `tools-openai`, `tools-mcp` | the fixed LLM toolset |
//! | `protocol` | response envelope, request and event schemas |
//! | `errors` | every registered error code |
//! | `op:<idname>` (also `mesh.bevel`, `bpy.ops.mesh.bevel`, `MESH_OT_bevel`) | one operator |
//! | `struct:<Struct>` (also `Object`) | one RNA struct |
//! | `<Struct>.<prop>` (`Object.location`) | one property or function |

use frap_rna::{ERROR_CODES, ErrorCode, ErrorCodeDoc, FrapError, PyExc};
use serde_json::Value as J;

use super::{all_ops, describe_entry, describe_struct, describe_struct_prop, find_op};

/// A describe target that does not exist (unknown operator, struct, property or target form).
pub const DESCRIBE_TARGET: ErrorCode = ErrorCode("DESCRIBE_TARGET");

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(ERROR_CODES)]
#[linkme(crate = frap_rna::linkme)]
static DOC_DESCRIBE_TARGET: ErrorCodeDoc = ErrorCodeDoc {
    code: DESCRIBE_TARGET,
    exception: PyExc::KeyError,
    summary: "frap --describe / frap_describe: unknown operator, struct, property or target form (detail.did_you_mean)",
};

/// What to describe.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Target {
    /// The whole manifest.
    Manifest,
    /// JSON Schemas of every operator input plus the protocol.
    JsonSchema,
    /// Anthropic Messages API tool definitions.
    ToolsAnthropic,
    /// OpenAI (Chat Completions) function tool definitions.
    ToolsOpenai,
    /// MCP `tools/list` result.
    ToolsMcp,
    /// Protocol schemas.
    Protocol,
    /// Registered error codes.
    Errors,
    /// One operator (`mesh.bevel`).
    Op(String),
    /// One RNA struct (`Object`).
    Struct(String),
    /// One struct member (`Object`, `location`).
    Prop(String, String),
}

/// Options of `--describe`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DescribeOptions {
    /// `--filter 'mesh.*'`: comma-separated globs over operator idnames.
    pub filter: Option<String>,
    /// `--implemented-only`.
    pub implemented_only: bool,
    /// Tools only: also emit one tool per listed operator namespace (`frap_ops_mesh`).
    pub tool_namespaces: Vec<String>,
}

/// A failed describe.
#[derive(Clone, Debug, PartialEq)]
pub struct DescribeError {
    /// Message.
    pub message: String,
    /// Close names.
    pub did_you_mean: Vec<String>,
}

impl std::fmt::Display for DescribeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)?;
        if !self.did_you_mean.is_empty() {
            write!(f, " (did you mean: {})", self.did_you_mean.join(", "))?;
        }
        Ok(())
    }
}

impl std::error::Error for DescribeError {}

impl DescribeError {
    fn new(message: String, candidates: Vec<&str>, word: &str) -> Self {
        DescribeError {
            message,
            did_you_mean: frap_op::suggest(word, candidates.into_iter())
                .into_iter()
                .map(str::to_owned)
                .collect(),
        }
    }
    /// The protocol error (`KeyError`, code `DESCRIBE_TARGET`).
    pub fn into_frap(self) -> FrapError {
        FrapError::new(PyExc::KeyError, DESCRIBE_TARGET, self.message)
            .with_detail(
                "did_you_mean",
                self.did_you_mean.into_iter().map(J::from).collect::<Vec<_>>(),
            )
            .with_hint("frap --describe lists every operator and struct; targets: op:<idname>, struct:<Struct>, <Struct>.<prop>")
    }
}

impl From<DescribeError> for FrapError {
    fn from(e: DescribeError) -> Self {
        e.into_frap()
    }
}

fn is_ident(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

impl Target {
    /// Parses a `--describe=<target>` value or a `frap_describe` / `frap.describe` target.
    pub fn parse(s: &str) -> Result<Target, DescribeError> {
        let s = s.trim();
        Ok(match s {
            "" | "manifest" => Target::Manifest,
            "jsonschema" => Target::JsonSchema,
            "tools-anthropic" | "tools" => Target::ToolsAnthropic,
            "tools-openai" => Target::ToolsOpenai,
            "tools-mcp" => Target::ToolsMcp,
            "protocol" => Target::Protocol,
            "errors" => Target::Errors,
            _ => {
                if let Some(op) = s.strip_prefix("op:") {
                    Target::Op(op.trim().to_owned())
                } else if let Some(st) = s.strip_prefix("struct:") {
                    Target::Struct(st.trim().trim_start_matches("bpy.types.").to_owned())
                } else if let Some(op) = s.strip_prefix("bpy.ops.") {
                    Target::Op(op.to_owned())
                } else {
                    let t = s.trim_start_matches("bpy.types.");
                    match t.split_once('.') {
                        Some((a, b)) if is_ident(a) && is_ident(b) => {
                            if a.starts_with(|c: char| c.is_ascii_uppercase()) {
                                Target::Prop(a.to_owned(), b.to_owned())
                            } else {
                                Target::Op(t.to_owned())
                            }
                        }
                        None if is_ident(t) && t.starts_with(|c: char| c.is_ascii_uppercase()) => {
                            Target::Struct(t.to_owned())
                        }
                        _ => {
                            return Err(DescribeError::new(
                                format!(
                                    "frap: unknown describe target '{s}'; expected manifest, jsonschema, tools-anthropic, tools-openai, tools-mcp, protocol, errors, op:<idname>, struct:<Struct> or <Struct>.<prop>"
                                ),
                                vec![
                                    "manifest",
                                    "jsonschema",
                                    "tools-anthropic",
                                    "tools-openai",
                                    "tools-mcp",
                                    "protocol",
                                    "errors",
                                ],
                                s,
                            ));
                        }
                    }
                }
            }
        })
    }
}

fn find_struct(name: &str) -> Result<&'static frap_rna::StructDef, DescribeError> {
    frap_spec_rna::find_struct(name).ok_or_else(|| {
        DescribeError::new(
            format!("frap: bpy.types has no struct '{name}'"),
            frap_spec_rna::ALL_STRUCTS
                .iter()
                .map(|s| s.identifier)
                .collect(),
            name,
        )
    })
}

/// Runs one describe request.
pub fn describe(target: &Target, opts: &DescribeOptions) -> Result<J, DescribeError> {
    Ok(match target {
        Target::Manifest => super::manifest(opts),
        Target::JsonSchema => crate::jsonschema::describe_jsonschema(opts),
        Target::ToolsAnthropic => crate::tools::anthropic(opts)?,
        Target::ToolsOpenai => crate::tools::openai(opts)?,
        Target::ToolsMcp => crate::tools::mcp(opts)?,
        Target::Protocol => crate::jsonschema::protocol(),
        Target::Errors => super::errors(),
        Target::Op(name) => match find_op(name) {
            Some(e) => describe_entry(e),
            None => {
                return Err(DescribeError::new(
                    format!(
                        "frap: operator \"bpy.ops.{name}\" does not exist in Blender 5.2 or frap"
                    ),
                    all_ops().iter().map(|e| e.spec.idname).collect(),
                    name,
                ));
            }
        },
        Target::Struct(name) => describe_struct(find_struct(name)?),
        Target::Prop(st, prop) => {
            let s = find_struct(st)?;
            match describe_struct_prop(s, prop) {
                Some(v) => v,
                None => {
                    let mut names: Vec<&str> = s.all_props().iter().map(|p| p.identifier).collect();
                    let mut cur = Some(s);
                    while let Some(d) = cur {
                        names.extend(d.funcs.iter().map(|f| f.identifier));
                        cur = d.base;
                    }
                    return Err(DescribeError::new(
                        format!("frap: '{st}' object has no attribute '{prop}'"),
                        names,
                        prop,
                    ));
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_targets() {
        let p = |s: &str| Target::parse(s).unwrap();
        assert_eq!(p(""), Target::Manifest);
        assert_eq!(p("tools-openai"), Target::ToolsOpenai);
        assert_eq!(p("op:mesh.bevel"), Target::Op("mesh.bevel".into()));
        assert_eq!(p("mesh.bevel"), Target::Op("mesh.bevel".into()));
        assert_eq!(p("bpy.ops.mesh.bevel"), Target::Op("mesh.bevel".into()));
        assert_eq!(
            p("struct:BevelModifier"),
            Target::Struct("BevelModifier".into())
        );
        assert_eq!(p("Object"), Target::Struct("Object".into()));
        assert_eq!(
            p("Object.location"),
            Target::Prop("Object".into(), "location".into())
        );
        assert_eq!(
            p("bpy.types.Object.location"),
            Target::Prop("Object".into(), "location".into())
        );
        let e = Target::parse("jsonshema").unwrap_err();
        assert_eq!(e.did_you_mean, vec!["jsonschema".to_owned()]);
    }
}
