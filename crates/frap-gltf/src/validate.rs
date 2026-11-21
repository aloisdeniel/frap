// SPDX-License-Identifier: GPL-3.0-or-later
//! Validator: indices in range, required extensions present (§17.1). WP 1.5.
//!
//! Three levels:
//! - [`validate`]: the JSON alone (references, array lengths, accessor/buffer-view layout and
//!   alignment, attribute and index formats, node hierarchy, cameras, skins, animations,
//!   extension declarations, and each typed extension's own checks via
//!   [`crate::json::EXTENSIONS`]).
//! - [`validate_data`]: plus the binary data (every accessor decodes, min/max equal the data,
//!   indices in range and free of restart values, sparse indices and animation times
//!   increasing).
//! - [`validate_glb`]: plus GLB container rules (buffer 0 is the BIN chunk).
//! - [`validate_file`]: from the file bytes, plus what only the JSON text shows (members the
//!   model does not keep, integers written as floats; [`json_text_issues`]).
//!
//! Every size and offset computed from file values uses checked arithmetic: an overflow is
//! reported as the corresponding TOO_LONG / INVALID_COUNT error, never a panic or a wrap.
//!
//! Issue codes follow the Khronos glTF-Validator's where one exists.

use std::fmt;

use crate::accessor::{AccessorData, element_size, load_buffers, read_accessor};
use crate::glb::{Glb, is_glb, split_glb};
use crate::json::root::{escape_pointer_token, trim_json};
use crate::json::{
    AccessorType, AnimationPath, CameraType, ComponentType, Document, ExtensionCx, Interpolation,
    Target, TextureInfo, UnknownKind, extensions::extension_def,
};

/// How bad an issue is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// The file violates the spec.
    Error,
    /// Allowed but probably wrong.
    Warning,
}

/// One finding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Issue {
    /// Severity.
    pub severity: Severity,
    /// Stable code (`UNRESOLVED_REFERENCE`, `ACCESSOR_TOO_LONG`, …).
    pub code: &'static str,
    /// JSON pointer of the offending value.
    pub pointer: String,
    /// Human-readable explanation.
    pub message: String,
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sev = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        write!(
            f,
            "{sev} {} at {}: {}",
            self.code, self.pointer, self.message
        )
    }
}

/// All findings of a validation run, in discovery order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// Findings.
    pub issues: Vec<Issue>,
}

impl Report {
    /// Records an error.
    pub fn error(
        &mut self,
        code: &'static str,
        pointer: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.issues.push(Issue {
            severity: Severity::Error,
            code,
            pointer: pointer.into(),
            message: message.into(),
        });
    }

    /// Records a warning.
    pub fn warning(
        &mut self,
        code: &'static str,
        pointer: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.issues.push(Issue {
            severity: Severity::Warning,
            code,
            pointer: pointer.into(),
            message: message.into(),
        });
    }

    /// No errors (warnings allowed).
    pub fn is_ok(&self) -> bool {
        self.errors().next().is_none()
    }

    /// The errors.
    pub fn errors(&self) -> impl Iterator<Item = &Issue> {
        self.issues.iter().filter(|i| i.severity == Severity::Error)
    }

    /// Whether an issue with `code` was found.
    pub fn has(&self, code: &str) -> bool {
        self.issues.iter().any(|i| i.code == code)
    }
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for i in &self.issues {
            writeln!(f, "{i}")?;
        }
        Ok(())
    }
}

/// Reports `UNRESOLVED_REFERENCE` unless `index < len`; returns whether it resolved.
fn check_ref(r: &mut Report, pointer: &str, index: u32, len: usize, what: &str) -> bool {
    if (index as usize) < len {
        true
    } else {
        r.error(
            "UNRESOLVED_REFERENCE",
            pointer,
            format!("{what} {index} does not exist ({len} defined)"),
        );
        false
    }
}

fn check_texture_info(r: &mut Report, doc: &Document, ptr: &str, ti: &TextureInfo) {
    check_ref(
        r,
        &format!("{ptr}/index"),
        ti.index,
        doc.textures.len(),
        "texture",
    );
}

fn check_unit_range(r: &mut Report, ptr: &str, v: f64) {
    if !(0.0..=1.0).contains(&v) {
        r.error("VALUE_NOT_IN_RANGE", ptr, format!("{v} is outside [0, 1]"));
    }
}

/// Allowed element shapes and `(component type, normalized)` pairs.
type AttributeFormats = (&'static [AccessorType], Vec<(ComponentType, bool)>);

/// Allowed `(component type, normalized)` pairs for a vertex attribute semantic; `None` for
/// application-specific (`_`-prefixed) and unknown semantics. With `quantized`
/// (KHR_mesh_quantization), POSITION, NORMAL, TANGENT and TEXCOORD accept integer types.
fn attribute_formats(semantic: &str, quantized: bool, target: bool) -> Option<AttributeFormats> {
    use AccessorType::*;
    use ComponentType::*;
    let f = (Float, false);
    let norm_u = [(UnsignedByte, true), (UnsignedShort, true)];
    let base = semantic.split('_').next().unwrap_or(semantic);
    let indexed = semantic.contains('_') && semantic[base.len() + 1..].parse::<u32>().is_ok();
    Some(match (semantic, base) {
        ("POSITION", _) => (
            &[Vec3],
            if quantized {
                vec![
                    f,
                    (Byte, false),
                    (Byte, true),
                    (UnsignedByte, false),
                    (UnsignedByte, true),
                    (Short, false),
                    (Short, true),
                    (UnsignedShort, false),
                    (UnsignedShort, true),
                ]
            } else {
                vec![f]
            },
        ),
        ("NORMAL", _) => (
            &[Vec3],
            if quantized {
                vec![f, (Byte, true), (Short, true)]
            } else {
                vec![f]
            },
        ),
        // Morph-target tangents are VEC3 deltas.
        ("TANGENT", _) => (
            if target { &[Vec3] } else { &[Vec4] },
            if quantized {
                vec![f, (Byte, true), (Short, true)]
            } else {
                vec![f]
            },
        ),
        (_, "TEXCOORD") if indexed => (
            &[Vec2],
            if quantized {
                vec![
                    f,
                    (Byte, false),
                    (Byte, true),
                    (UnsignedByte, false),
                    (UnsignedByte, true),
                    (Short, false),
                    (Short, true),
                    (UnsignedShort, false),
                    (UnsignedShort, true),
                ]
            } else {
                vec![f, norm_u[0], norm_u[1]]
            },
        ),
        (_, "COLOR") if indexed => (&[Vec3, Vec4], vec![f, norm_u[0], norm_u[1]]),
        (_, "JOINTS") if indexed => (&[Vec4], vec![(UnsignedByte, false), (UnsignedShort, false)]),
        (_, "WEIGHTS") if indexed => (&[Vec4], vec![f, norm_u[0], norm_u[1]]),
        _ => return None,
    })
}

/// Validates the JSON of `doc` (no binary data needed).
pub fn validate(doc: &Document) -> Report {
    let mut r = Report::default();
    check_unknown(&mut r, doc);
    check_asset(&mut r, doc);
    check_extensions(&mut r, doc);
    check_scenes_nodes(&mut r, doc);
    check_meshes(&mut r, doc);
    check_buffers(&mut r, doc);
    check_accessors(&mut r, doc);
    check_materials_textures(&mut r, doc);
    check_cameras(&mut r, doc);
    check_skins(&mut r, doc);
    check_animations(&mut r, doc);
    r
}

/// Whether accessor `i` has an unknown `componentType` or `type` (or an unknown sparse
/// index type): its data cannot be laid out, so checks that read its format skip it.
pub(crate) fn layout_unknown(doc: &Document, i: u32) -> bool {
    !doc.unknown.is_empty() && {
        let ptr = format!("/accessors/{i}");
        doc.has_unknown_value(&ptr) || doc.has_unknown_value(&format!("{ptr}/sparse/indices"))
    }
}

/// What the reader kept outside the typed model ([`Document::unknown`]): UNEXPECTED_PROPERTY
/// for unknown members and VALUE_NOT_IN_LIST for unknown enum values, both warnings as in the
/// Khronos validator (the document loads and is written back with them).
fn check_unknown(r: &mut Report, doc: &Document) {
    for u in &doc.unknown.0 {
        match u.kind {
            UnknownKind::Member => r.warning(
                "UNEXPECTED_PROPERTY",
                u.member_pointer(),
                format!("unexpected property {:?} (kept)", u.key),
            ),
            UnknownKind::EnumValue { allowed } => r.warning(
                "VALUE_NOT_IN_LIST",
                u.member_pointer(),
                format!(
                    "invalid value {}; valid values are ({})",
                    u.value,
                    allowed.join(", ")
                ),
            ),
        }
    }
}

fn check_asset(r: &mut Report, doc: &Document) {
    let major = |v: &str| v.split('.').next().and_then(|m| m.parse::<u32>().ok());
    if major(&doc.asset.version) != Some(2) || !doc.asset.version.contains('.') {
        r.error(
            "UNKNOWN_ASSET_MAJOR_VERSION",
            "/asset/version",
            format!("unsupported glTF version {:?}", doc.asset.version),
        );
    }
    if let Some(min) = &doc.asset.min_version
        && min.as_str() > doc.asset.version.as_str()
    {
        r.error(
            "ASSET_MIN_VERSION_GREATER_THAN_VERSION",
            "/asset/minVersion",
            format!(
                "minVersion {min} is greater than version {}",
                doc.asset.version
            ),
        );
    }
}

fn check_extensions(r: &mut Report, doc: &Document) {
    for (i, name) in doc.extensions_used.iter().enumerate() {
        if doc.extensions_used[..i].contains(name) {
            r.error(
                "DUPLICATE_ELEMENTS",
                format!("/extensionsUsed/{i}"),
                format!("{name} is listed twice"),
            );
        }
    }
    for (i, name) in doc.extensions_required.iter().enumerate() {
        if !doc.extensions_used.contains(name) {
            r.error(
                "NOT_IN_USED_EXTENSIONS",
                format!("/extensionsRequired/{i}"),
                format!("required extension {name} is not in extensionsUsed"),
            );
        }
    }
    let mut present: Vec<String> = Vec::new();
    let mut occurrences: Vec<(String, serde_json::Value, String, serde_json::Value)> = Vec::new();
    doc.for_each_extension(|owner, owner_value, name, v| {
        if !present.iter().any(|n| n == name) {
            present.push(name.to_owned());
            if !doc.extensions_used.iter().any(|n| n == name) {
                r.error(
                    "UNDECLARED_EXTENSION",
                    format!("{owner}/extensions/{}", escape_pointer_token(name)),
                    format!("extension {name} is used but not listed in extensionsUsed"),
                );
            }
        }
        if extension_def(name).is_some() {
            occurrences.push((
                owner.to_owned(),
                owner_value.clone(),
                name.to_owned(),
                v.clone(),
            ));
        }
    });
    for (i, name) in doc.extensions_used.iter().enumerate() {
        if !present.contains(name) {
            r.warning(
                "UNUSED_EXTENSION",
                format!("/extensionsUsed/{i}"),
                format!("{name} is declared but never used"),
            );
        }
    }
    for (owner, owner_value, name, v) in &occurrences {
        if let Some(def) = extension_def(name) {
            (def.validate)(
                &ExtensionCx {
                    doc,
                    owner,
                    owner_value,
                    name,
                    value: v,
                },
                r,
            );
        }
    }
}

fn check_scenes_nodes(r: &mut Report, doc: &Document) {
    let n = doc.nodes.len();
    if let Some(s) = doc.scene {
        check_ref(r, "/scene", s, doc.scenes.len(), "scene");
    }
    // Parents, multiple parents.
    let mut parent: Vec<Option<usize>> = vec![None; n];
    for (i, node) in doc.nodes.iter().enumerate() {
        for (k, &c) in node.children.iter().enumerate() {
            let ptr = format!("/nodes/{i}/children/{k}");
            if !check_ref(r, &ptr, c, n, "node") {
                continue;
            }
            let c = c as usize;
            if c == i {
                r.error("NODE_LOOP", ptr, format!("node {i} is its own child"));
            } else if let Some(p) = parent[c] {
                r.error(
                    "NODE_PARENT_OVERRIDE",
                    ptr,
                    format!("node {c} already has parent {p}"),
                );
            } else {
                parent[c] = Some(i);
            }
        }
    }
    // Cycles through the parent chain.
    for start in 0..n {
        let mut cur = parent[start];
        let mut steps = 0;
        while let Some(p) = cur {
            if p == start || steps > n {
                r.error(
                    "NODE_LOOP",
                    format!("/nodes/{start}"),
                    format!("node {start} is its own ancestor"),
                );
                break;
            }
            cur = parent[p];
            steps += 1;
        }
    }
    for (s, scene) in doc.scenes.iter().enumerate() {
        for (k, &root) in scene.nodes.iter().enumerate() {
            let ptr = format!("/scenes/{s}/nodes/{k}");
            if !check_ref(r, &ptr, root, n, "node") {
                continue;
            }
            if scene.nodes[..k].contains(&root) {
                r.error(
                    "DUPLICATE_ELEMENTS",
                    &ptr,
                    format!("node {root} is listed twice"),
                );
            }
            if let Some(p) = parent[root as usize] {
                r.error(
                    "SCENE_NON_ROOT_NODE",
                    ptr,
                    format!("node {root} is a child of node {p}, not a root"),
                );
            }
        }
    }
    for (i, node) in doc.nodes.iter().enumerate() {
        let ptr = format!("/nodes/{i}");
        if node.matrix.is_some()
            && (node.translation.is_some() || node.rotation.is_some() || node.scale.is_some())
        {
            r.error(
                "NODE_MATRIX_TRS",
                &ptr,
                "a node has either matrix or translation/rotation/scale",
            );
        }
        if let Some(q) = node.rotation {
            let len2: f64 = q.iter().map(|c| c * c).sum();
            if (len2.sqrt() - 1.0).abs() > 5e-4 {
                r.error(
                    "ROTATION_NON_UNIT",
                    format!("{ptr}/rotation"),
                    format!("quaternion length {} is not 1", len2.sqrt()),
                );
            }
        }
        let finite = node
            .matrix
            .iter()
            .flatten()
            .chain(node.translation.iter().flatten())
            .chain(node.rotation.iter().flatten())
            .chain(node.scale.iter().flatten())
            .all(|v| v.is_finite());
        if !finite {
            r.error("VALUE_NOT_FINITE", &ptr, "transform has a non-finite value");
        }
        if let Some(c) = node.camera {
            check_ref(r, &format!("{ptr}/camera"), c, doc.cameras.len(), "camera");
        }
        let mesh = node
            .mesh
            .filter(|&m| check_ref(r, &format!("{ptr}/mesh"), m, doc.meshes.len(), "mesh"))
            .map(|m| &doc.meshes[m as usize]);
        if let Some(s) = node.skin {
            check_ref(r, &format!("{ptr}/skin"), s, doc.skins.len(), "skin");
            if node.mesh.is_none() {
                r.error(
                    "NODE_SKIN_NO_MESH",
                    format!("{ptr}/skin"),
                    "a node with a skin needs a mesh",
                );
            }
        }
        if !node.weights.is_empty() {
            match mesh {
                None => r.error(
                    "NODE_WEIGHTS_INVALID",
                    format!("{ptr}/weights"),
                    "weights on a node without a mesh",
                ),
                Some(m) => {
                    let targets = m.primitives.first().map_or(0, |p| p.targets.len());
                    if node.weights.len() != targets {
                        r.error(
                            "NODE_WEIGHTS_INVALID",
                            format!("{ptr}/weights"),
                            format!("{} weights for {targets} morph targets", node.weights.len()),
                        );
                    }
                }
            }
        }
    }
}

fn check_meshes(r: &mut Report, doc: &Document) {
    let quantized = doc
        .extensions_used
        .iter()
        .any(|e| e == "KHR_mesh_quantization");
    for (m, mesh) in doc.meshes.iter().enumerate() {
        let mptr = format!("/meshes/{m}");
        if mesh.primitives.is_empty() {
            r.error(
                "EMPTY_ENTITY",
                format!("{mptr}/primitives"),
                "a mesh needs a primitive",
            );
        }
        let targets = mesh.primitives.first().map_or(0, |p| p.targets.len());
        if !mesh.weights.is_empty() && mesh.weights.len() != targets {
            r.error(
                "MESH_INVALID_WEIGHTS_COUNT",
                format!("{mptr}/weights"),
                format!("{} weights for {targets} morph targets", mesh.weights.len()),
            );
        }
        for (p, prim) in mesh.primitives.iter().enumerate() {
            let pptr = format!("{mptr}/primitives/{p}");
            if prim.targets.len() != targets {
                r.error(
                    "MESH_PRIMITIVES_UNEQUAL_TARGETS_COUNT",
                    format!("{pptr}/targets"),
                    format!(
                        "{} targets where primitive 0 has {targets}",
                        prim.targets.len()
                    ),
                );
            }
            if let Some(mat) = prim.material {
                check_ref(
                    r,
                    &format!("{pptr}/material"),
                    mat,
                    doc.materials.len(),
                    "material",
                );
            }
            let mut count: Option<u64> = None;
            let attr_sets =
                std::iter::once((format!("{pptr}/attributes"), &prim.attributes, false)).chain(
                    prim.targets
                        .iter()
                        .enumerate()
                        .map(|(t, a)| (format!("{pptr}/targets/{t}"), a, true)),
                );
            for (aptr, attrs, is_target) in attr_sets {
                for (sem, &a) in attrs {
                    let ptr = format!("{aptr}/{}", escape_pointer_token(sem));
                    if !check_ref(r, &ptr, a, doc.accessors.len(), "accessor") {
                        continue;
                    }
                    let acc = &doc.accessors[a as usize];
                    match count {
                        None => count = Some(acc.count),
                        Some(c) if c != acc.count => r.error(
                            "MESH_PRIMITIVE_UNEQUAL_ACCESSOR_COUNT",
                            &ptr,
                            format!("{} elements where other attributes have {c}", acc.count),
                        ),
                        _ => {}
                    }
                    if let Some((types, formats)) = attribute_formats(sem, quantized, is_target)
                        && !layout_unknown(doc, a)
                    {
                        let fmt = (acc.component_type, acc.normalized_or_default());
                        if !types.contains(&acc.type_) || !formats.contains(&fmt) {
                            r.error(
                                "MESH_PRIMITIVE_ATTRIBUTE_FORMAT",
                                &ptr,
                                format!(
                                    "{sem} cannot be {} {:?}{}",
                                    acc.type_.as_str(),
                                    acc.component_type,
                                    if fmt.1 { " normalized" } else { "" }
                                ),
                            );
                        }
                    } else if !sem.starts_with('_') {
                        r.warning(
                            "MESH_PRIMITIVE_INVALID_ATTRIBUTE",
                            &ptr,
                            format!(
                                "unknown semantic {sem} (application-specific ones start with '_')"
                            ),
                        );
                    }
                    if sem == "POSITION" && (acc.min.is_none() || acc.max.is_none()) {
                        r.error(
                            "MESH_PRIMITIVE_POSITION_ACCESSOR_WITHOUT_BOUNDS",
                            &ptr,
                            "POSITION accessors need min and max",
                        );
                    }
                    if !is_target
                        && let Some(bv) = acc
                            .buffer_view
                            .and_then(|v| doc.buffer_views.get(v as usize))
                        && bv.target == Some(Target::ElementArrayBuffer)
                    {
                        r.error(
                            "BUFFER_VIEW_TARGET_OVERRIDE",
                            &ptr,
                            "vertex attribute in an ELEMENT_ARRAY_BUFFER view",
                        );
                    }
                }
            }
            for set in ["TEXCOORD", "COLOR", "JOINTS", "WEIGHTS"] {
                let mut sets: Vec<u32> = prim
                    .attributes
                    .keys()
                    .filter_map(|k| k.strip_prefix(set)?.strip_prefix('_')?.parse().ok())
                    .collect();
                sets.sort_unstable();
                if sets.iter().enumerate().any(|(i, &s)| s as usize != i) {
                    r.error(
                        "MESH_PRIMITIVE_INDEXED_SEMANTIC_CONTINUITY",
                        format!("{pptr}/attributes"),
                        format!("{set}_n sets are not numbered 0, 1, …"),
                    );
                }
            }
            let has = |s: &str| prim.attributes.keys().any(|k| k.starts_with(s));
            if has("JOINTS_") != has("WEIGHTS_") {
                r.error(
                    "MESH_PRIMITIVE_JOINTS_WEIGHTS_MISMATCH",
                    format!("{pptr}/attributes"),
                    "JOINTS_n and WEIGHTS_n come in pairs",
                );
            }
            if let Some(i) = prim.indices {
                let ptr = format!("{pptr}/indices");
                if check_ref(r, &ptr, i, doc.accessors.len(), "accessor") {
                    let acc = &doc.accessors[i as usize];
                    if !layout_unknown(doc, i)
                        && (acc.type_ != AccessorType::Scalar
                            || !acc.component_type.is_unsigned_int()
                            || acc.normalized_or_default())
                    {
                        r.error(
                            "MESH_PRIMITIVE_INDICES_FORMAT",
                            &ptr,
                            "indices must be non-normalized SCALAR unsigned integers",
                        );
                    }
                    if let Some(bv) = acc
                        .buffer_view
                        .and_then(|v| doc.buffer_views.get(v as usize))
                    {
                        if bv.byte_stride.is_some() {
                            r.error(
                                "MESH_PRIMITIVE_INDICES_FORMAT",
                                &ptr,
                                "index buffer views have no byteStride",
                            );
                        }
                        if bv.target == Some(Target::ArrayBuffer) {
                            r.error(
                                "BUFFER_VIEW_TARGET_OVERRIDE",
                                &ptr,
                                "indices in an ARRAY_BUFFER view",
                            );
                        }
                    }
                }
            }
            if prim.attributes.is_empty() && prim.extensions.is_empty() {
                r.error(
                    "EMPTY_ENTITY",
                    format!("{pptr}/attributes"),
                    "a primitive needs attributes",
                );
            }
        }
    }
}

fn check_buffers(r: &mut Report, doc: &Document) {
    for (i, b) in doc.buffers.iter().enumerate() {
        let ptr = format!("/buffers/{i}");
        if b.byte_length == 0 {
            r.error(
                "VALUE_NOT_IN_RANGE",
                format!("{ptr}/byteLength"),
                "byteLength must be at least 1",
            );
        }
        let fallback = ["EXT_meshopt_compression", "KHR_meshopt_compression"]
            .iter()
            .any(|e| {
                b.extensions
                    .get_raw(e)
                    .and_then(|v| v.get("fallback"))
                    .and_then(serde_json::Value::as_bool)
                    == Some(true)
            });
        if b.uri.is_none() && i != 0 && !fallback {
            r.error(
                "BUFFER_MISSING_GLB_DATA",
                &ptr,
                "only buffer 0 (the GLB BIN chunk) may omit uri",
            );
        }
    }
    for (i, bv) in doc.buffer_views.iter().enumerate() {
        let ptr = format!("/bufferViews/{i}");
        if bv.byte_length == 0 {
            r.error(
                "VALUE_NOT_IN_RANGE",
                format!("{ptr}/byteLength"),
                "byteLength must be at least 1",
            );
        }
        if let Some(s) = bv.byte_stride
            && (!(4..=252).contains(&s) || s % 4 != 0)
        {
            r.error(
                "BUFFER_VIEW_INVALID_BYTE_STRIDE",
                format!("{ptr}/byteStride"),
                format!("byteStride {s} must be a multiple of 4 in 4..=252"),
            );
        }
        if bv.byte_stride.is_some() && bv.target == Some(Target::ElementArrayBuffer) {
            r.error(
                "BUFFER_VIEW_INVALID_BYTE_STRIDE",
                format!("{ptr}/byteStride"),
                "ELEMENT_ARRAY_BUFFER views cannot have a byteStride",
            );
        }
        if check_ref(
            r,
            &format!("{ptr}/buffer"),
            bv.buffer,
            doc.buffers.len(),
            "buffer",
        ) {
            let len = doc.buffers[bv.buffer as usize].byte_length;
            let end = bv.byte_offset_or_default().checked_add(bv.byte_length);
            if end.is_none_or(|e| e > len) {
                r.error(
                    "BUFFER_VIEW_TOO_LONG",
                    &ptr,
                    format!(
                        "offset {} + length {} exceeds buffer {} ({len} bytes)",
                        bv.byte_offset_or_default(),
                        bv.byte_length,
                        bv.buffer
                    ),
                );
            }
        }
    }
}

fn check_accessors(r: &mut Report, doc: &Document) {
    for (i, acc) in doc.accessors.iter().enumerate() {
        let ptr = format!("/accessors/{i}");
        // An unknown `componentType`/`type` (VALUE_NOT_IN_LIST) leaves no layout to check,
        // as the Khronos validator skips the accessor.
        if layout_unknown(doc, i as u32) {
            continue;
        }
        let ct = acc.component_type;
        let n = acc.type_.components();
        if acc.count == 0 {
            r.error(
                "VALUE_NOT_IN_RANGE",
                format!("{ptr}/count"),
                "count must be at least 1",
            );
        }
        if acc.normalized_or_default() && !ct.can_normalize() {
            r.error(
                "ACCESSOR_NORMALIZED_INVALID",
                format!("{ptr}/normalized"),
                format!("{ct:?} accessors cannot be normalized"),
            );
        }
        for (key, mm) in [("min", &acc.min), ("max", &acc.max)] {
            if let Some(v) = mm
                && v.len() != n
            {
                r.error(
                    "ACCESSOR_MIN_MAX_LENGTH",
                    format!("{ptr}/{key}"),
                    format!(
                        "{} values for a {}-component {}",
                        v.len(),
                        n,
                        acc.type_.as_str()
                    ),
                );
            }
        }
        if acc.min.is_some() != acc.max.is_some() {
            r.error("ACCESSOR_MIN_MAX_PAIR", &ptr, "min and max come together");
        }
        let elem = element_size(ct, acc.type_) as u64;
        let off = acc.byte_offset_or_default();
        if acc.buffer_view.is_none() && acc.byte_offset.is_some() {
            r.warning(
                "UNSATISFIED_DEPENDENCY",
                format!("{ptr}/byteOffset"),
                "byteOffset without bufferView",
            );
        }
        if let Some(v) = acc.buffer_view
            && check_ref(
                r,
                &format!("{ptr}/bufferView"),
                v,
                doc.buffer_views.len(),
                "bufferView",
            )
        {
            let bv = &doc.buffer_views[v as usize];
            let csize = ct.size() as u64;
            if !off.is_multiple_of(csize) {
                r.error(
                    "ACCESSOR_OFFSET_ALIGNMENT",
                    format!("{ptr}/byteOffset"),
                    format!("offset {off} is not a multiple of the {csize}-byte component"),
                );
            } else if !(off % csize + bv.byte_offset_or_default() % csize).is_multiple_of(csize) {
                r.error(
                    "ACCESSOR_TOTAL_OFFSET_ALIGNMENT",
                    format!("{ptr}/byteOffset"),
                    format!(
                        "accessor + view offset is not a multiple of the {csize}-byte component"
                    ),
                );
            }
            let stride = bv.byte_stride.map_or(elem, u64::from);
            if bv.byte_stride.is_some() && stride < elem {
                r.error(
                    "ACCESSOR_SMALL_BYTESTRIDE",
                    format!("{ptr}/bufferView"),
                    format!("byteStride {stride} is smaller than the {elem}-byte element"),
                );
            }
            if bv.target == Some(Target::ArrayBuffer)
                && (!stride.is_multiple_of(4) || !off.is_multiple_of(4))
            {
                r.error(
                    "ACCESSOR_VERTEX_ATTRIBUTE_ALIGNMENT",
                    &ptr,
                    "vertex attribute elements must start on 4-byte boundaries",
                );
            }
            if acc.count > 0 {
                let need = (acc.count - 1)
                    .checked_mul(stride)
                    .and_then(|x| x.checked_add(off))
                    .and_then(|x| x.checked_add(elem));
                if need.is_none_or(|x| x > bv.byte_length) {
                    r.error(
                        "ACCESSOR_TOO_LONG",
                        &ptr,
                        format!(
                            "{} elements of {elem} bytes (stride {stride}) from offset {off} exceed bufferView {v} ({} bytes)",
                            acc.count, bv.byte_length
                        ),
                    );
                }
            }
        }
        if let Some(sp) = &acc.sparse {
            let sptr = format!("{ptr}/sparse");
            if sp.count == 0 || sp.count > acc.count {
                r.error(
                    "VALUE_NOT_IN_RANGE",
                    format!("{sptr}/count"),
                    format!("sparse count {} must be in 1..={}", sp.count, acc.count),
                );
            }
            if !sp.indices.component_type.is_unsigned_int() {
                r.error(
                    "INVALID_COMPONENT_TYPE",
                    format!("{sptr}/indices/componentType"),
                    "sparse indices must be UNSIGNED_BYTE, UNSIGNED_SHORT or UNSIGNED_INT",
                );
            }
            for (key, view, off, size) in [
                (
                    "indices",
                    sp.indices.buffer_view,
                    sp.indices.byte_offset.unwrap_or(0),
                    sp.indices.component_type.size() as u64,
                ),
                (
                    "values",
                    sp.values.buffer_view,
                    sp.values.byte_offset.unwrap_or(0),
                    elem,
                ),
            ] {
                let vptr = format!("{sptr}/{key}/bufferView");
                if !check_ref(r, &vptr, view, doc.buffer_views.len(), "bufferView") {
                    continue;
                }
                let bv = &doc.buffer_views[view as usize];
                if bv.byte_stride.is_some() || bv.target.is_some() {
                    r.error(
                        "BUFFER_VIEW_TARGET_OVERRIDE",
                        &vptr,
                        "sparse buffer views have no byteStride and no target",
                    );
                }
                let end = size.checked_mul(sp.count).and_then(|x| x.checked_add(off));
                if end.is_none_or(|e| e > bv.byte_length) {
                    r.error(
                        "ACCESSOR_SPARSE_TOO_LONG",
                        &vptr,
                        format!(
                            "sparse {key} exceed bufferView {view} ({} bytes)",
                            bv.byte_length
                        ),
                    );
                }
            }
        }
    }
}

fn check_materials_textures(r: &mut Report, doc: &Document) {
    for (i, m) in doc.materials.iter().enumerate() {
        let ptr = format!("/materials/{i}");
        if let Some(pbr) = &m.pbr_metallic_roughness {
            let p = format!("{ptr}/pbrMetallicRoughness");
            if let Some(c) = pbr.base_color_factor {
                for (k, v) in c.iter().enumerate() {
                    check_unit_range(r, &format!("{p}/baseColorFactor/{k}"), *v);
                }
            }
            if let Some(v) = pbr.metallic_factor {
                check_unit_range(r, &format!("{p}/metallicFactor"), v);
            }
            if let Some(v) = pbr.roughness_factor {
                check_unit_range(r, &format!("{p}/roughnessFactor"), v);
            }
            if let Some(t) = &pbr.base_color_texture {
                check_texture_info(r, doc, &format!("{p}/baseColorTexture"), t);
            }
            if let Some(t) = &pbr.metallic_roughness_texture {
                check_texture_info(r, doc, &format!("{p}/metallicRoughnessTexture"), t);
            }
        }
        if let Some(t) = &m.normal_texture {
            check_ref(
                r,
                &format!("{ptr}/normalTexture/index"),
                t.index,
                doc.textures.len(),
                "texture",
            );
        }
        if let Some(t) = &m.occlusion_texture {
            check_ref(
                r,
                &format!("{ptr}/occlusionTexture/index"),
                t.index,
                doc.textures.len(),
                "texture",
            );
            if let Some(s) = t.strength {
                check_unit_range(r, &format!("{ptr}/occlusionTexture/strength"), s);
            }
        }
        if let Some(t) = &m.emissive_texture {
            check_texture_info(r, doc, &format!("{ptr}/emissiveTexture"), t);
        }
        if let Some(e) = m.emissive_factor {
            for (k, v) in e.iter().enumerate() {
                check_unit_range(r, &format!("{ptr}/emissiveFactor/{k}"), *v);
            }
        }
        if let Some(c) = m.alpha_cutoff {
            if c < 0.0 {
                r.error(
                    "VALUE_NOT_IN_RANGE",
                    format!("{ptr}/alphaCutoff"),
                    "alphaCutoff must be ≥ 0",
                );
            }
            if m.alpha_mode != Some(crate::json::AlphaMode::Mask) {
                r.warning(
                    "MATERIAL_ALPHA_CUTOFF_INVALID_MODE",
                    format!("{ptr}/alphaCutoff"),
                    "alphaCutoff is only used with alphaMode MASK",
                );
            }
        }
    }
    for (i, t) in doc.textures.iter().enumerate() {
        let ptr = format!("/textures/{i}");
        if let Some(s) = t.sampler {
            check_ref(
                r,
                &format!("{ptr}/sampler"),
                s,
                doc.samplers.len(),
                "sampler",
            );
        }
        if let Some(s) = t.source {
            check_ref(r, &format!("{ptr}/source"), s, doc.images.len(), "image");
        }
    }
    for (i, img) in doc.images.iter().enumerate() {
        let ptr = format!("/images/{i}");
        match (&img.uri, img.buffer_view) {
            (Some(_), Some(_)) => r.error(
                "ONE_OF_MISMATCH",
                &ptr,
                "an image has either uri or bufferView, not both",
            ),
            (None, None) => r.error("ONE_OF_MISMATCH", &ptr, "an image needs uri or bufferView"),
            (None, Some(v)) => {
                if img.mime_type.is_none() {
                    r.error(
                        "UNSATISFIED_DEPENDENCY",
                        format!("{ptr}/mimeType"),
                        "an image with a bufferView needs a mimeType",
                    );
                }
                if check_ref(
                    r,
                    &format!("{ptr}/bufferView"),
                    v,
                    doc.buffer_views.len(),
                    "bufferView",
                ) {
                    let bv = &doc.buffer_views[v as usize];
                    if bv.byte_stride.is_some() || bv.target.is_some() {
                        r.error(
                            "BUFFER_VIEW_TARGET_OVERRIDE",
                            format!("{ptr}/bufferView"),
                            "image buffer views have no byteStride and no target",
                        );
                    }
                }
            }
            (Some(_), None) => {}
        }
    }
}

fn check_cameras(r: &mut Report, doc: &Document) {
    for (i, c) in doc.cameras.iter().enumerate() {
        let ptr = format!("/cameras/{i}");
        // An unknown `type` (VALUE_NOT_IN_LIST): no projection to check.
        if doc.unknown_value(&ptr, "type").is_some() {
            continue;
        }
        match c.type_ {
            CameraType::Perspective => match &c.perspective {
                None => r.error(
                    "UNSATISFIED_DEPENDENCY",
                    &ptr,
                    "perspective camera without perspective",
                ),
                Some(p) => {
                    if p.yfov <= 0.0 || !p.yfov.is_finite() {
                        r.error(
                            "VALUE_NOT_IN_RANGE",
                            format!("{ptr}/perspective/yfov"),
                            "yfov must be > 0",
                        );
                    }
                    if p.znear <= 0.0 {
                        r.error(
                            "VALUE_NOT_IN_RANGE",
                            format!("{ptr}/perspective/znear"),
                            "znear must be > 0",
                        );
                    }
                    if let Some(f) = p.zfar
                        && f <= p.znear
                    {
                        r.error(
                            "CAMERA_ZFAR_LEQUAL_ZNEAR",
                            format!("{ptr}/perspective/zfar"),
                            "zfar must exceed znear",
                        );
                    }
                    if p.aspect_ratio.is_some_and(|a| a <= 0.0) {
                        r.error(
                            "VALUE_NOT_IN_RANGE",
                            format!("{ptr}/perspective/aspectRatio"),
                            "aspectRatio must be > 0",
                        );
                    }
                }
            },
            CameraType::Orthographic => match &c.orthographic {
                None => r.error(
                    "UNSATISFIED_DEPENDENCY",
                    &ptr,
                    "orthographic camera without orthographic",
                ),
                Some(o) => {
                    if o.znear < 0.0 {
                        r.error(
                            "VALUE_NOT_IN_RANGE",
                            format!("{ptr}/orthographic/znear"),
                            "znear must be ≥ 0",
                        );
                    }
                    if o.zfar <= o.znear {
                        r.error(
                            "CAMERA_ZFAR_LEQUAL_ZNEAR",
                            format!("{ptr}/orthographic/zfar"),
                            "zfar must exceed znear",
                        );
                    }
                    if o.xmag == 0.0 || o.ymag == 0.0 {
                        r.error(
                            "CAMERA_XMAG_YMAG_ZERO",
                            &ptr,
                            "xmag and ymag must be non-zero",
                        );
                    }
                }
            },
        }
        if c.perspective.is_some() && c.orthographic.is_some() {
            r.error(
                "ONE_OF_MISMATCH",
                &ptr,
                "a camera has perspective or orthographic, not both",
            );
        }
    }
}

fn check_skins(r: &mut Report, doc: &Document) {
    for (i, s) in doc.skins.iter().enumerate() {
        let ptr = format!("/skins/{i}");
        if s.joints.is_empty() {
            r.error(
                "EMPTY_ENTITY",
                format!("{ptr}/joints"),
                "a skin needs joints",
            );
        }
        for (k, &j) in s.joints.iter().enumerate() {
            let jp = format!("{ptr}/joints/{k}");
            if check_ref(r, &jp, j, doc.nodes.len(), "node") && s.joints[..k].contains(&j) {
                r.error(
                    "DUPLICATE_ELEMENTS",
                    jp,
                    format!("joint node {j} is listed twice"),
                );
            }
        }
        if let Some(sk) = s.skeleton {
            check_ref(r, &format!("{ptr}/skeleton"), sk, doc.nodes.len(), "node");
        }
        if let Some(a) = s.inverse_bind_matrices {
            let ap = format!("{ptr}/inverseBindMatrices");
            if check_ref(r, &ap, a, doc.accessors.len(), "accessor") {
                let acc = &doc.accessors[a as usize];
                if !layout_unknown(doc, a)
                    && (acc.type_ != AccessorType::Mat4
                        || acc.component_type != ComponentType::Float)
                {
                    r.error(
                        "SKIN_IBM_INVALID_FORMAT",
                        &ap,
                        "inverse bind matrices must be MAT4 FLOAT",
                    );
                }
                if acc.count != s.joints.len() as u64 {
                    r.error(
                        "INVALID_IBM_ACCESSOR_COUNT",
                        &ap,
                        format!("{} matrices for {} joints", acc.count, s.joints.len()),
                    );
                }
            }
        }
    }
}

fn check_animations(r: &mut Report, doc: &Document) {
    for (a, anim) in doc.animations.iter().enumerate() {
        let ptr = format!("/animations/{a}");
        if anim.channels.is_empty() {
            r.error(
                "EMPTY_ENTITY",
                format!("{ptr}/channels"),
                "an animation needs channels",
            );
        }
        if anim.samplers.is_empty() {
            r.error(
                "EMPTY_ENTITY",
                format!("{ptr}/samplers"),
                "an animation needs samplers",
            );
        }
        for (s, smp) in anim.samplers.iter().enumerate() {
            let sp = format!("{ptr}/samplers/{s}");
            if check_ref(
                r,
                &format!("{sp}/input"),
                smp.input,
                doc.accessors.len(),
                "accessor",
            ) {
                let acc = &doc.accessors[smp.input as usize];
                if !layout_unknown(doc, smp.input)
                    && (acc.type_ != AccessorType::Scalar
                        || acc.component_type != ComponentType::Float)
                {
                    r.error(
                        "ANIMATION_SAMPLER_INPUT_ACCESSOR_INVALID_FORMAT",
                        format!("{sp}/input"),
                        "input must be SCALAR FLOAT",
                    );
                }
                if acc.min.is_none() || acc.max.is_none() {
                    r.error(
                        "ANIMATION_SAMPLER_INPUT_ACCESSOR_WITHOUT_BOUNDS",
                        format!("{sp}/input"),
                        "input accessors need min and max",
                    );
                }
                if smp.interpolation_or_default() == Interpolation::CubicSpline && acc.count < 2 {
                    r.error(
                        "ANIMATION_SAMPLER_INPUT_ACCESSOR_TOO_FEW_ELEMENTS",
                        format!("{sp}/input"),
                        "CUBICSPLINE needs at least 2 keyframes",
                    );
                }
            }
            check_ref(
                r,
                &format!("{sp}/output"),
                smp.output,
                doc.accessors.len(),
                "accessor",
            );
        }
        let mut seen: Vec<(u32, &AnimationPath)> = Vec::new();
        for (c, ch) in anim.channels.iter().enumerate() {
            let cp = format!("{ptr}/channels/{c}");
            let sampler = check_ref(
                r,
                &format!("{cp}/sampler"),
                ch.sampler,
                anim.samplers.len(),
                "sampler",
            )
            .then(|| &anim.samplers[ch.sampler as usize]);
            let path = &ch.target.path;
            if matches!(path, AnimationPath::Other(_)) {
                r.warning(
                    "ANIMATION_CHANNEL_TARGET_PATH",
                    format!("{cp}/target/path"),
                    format!("unknown path {:?}", path.as_str()),
                );
            }
            let Some(node) = ch.target.node else {
                if *path != AnimationPath::Pointer {
                    r.warning(
                        "ANIMATION_CHANNEL_TARGET_NODE",
                        format!("{cp}/target"),
                        "channel without target node is ignored",
                    );
                }
                continue;
            };
            if !check_ref(
                r,
                &format!("{cp}/target/node"),
                node,
                doc.nodes.len(),
                "node",
            ) {
                continue;
            }
            if seen.iter().any(|(n, p)| *n == node && *p == path) {
                r.error(
                    "ANIMATION_DUPLICATE_TARGETS",
                    format!("{cp}/target"),
                    format!("node {node} {} is animated twice", path.as_str()),
                );
            }
            seen.push((node, path));
            let Some(smp) = sampler else { continue };
            let (Some(input), Some(output)) = (
                doc.accessors.get(smp.input as usize),
                doc.accessors.get(smp.output as usize),
            ) else {
                continue;
            };
            if layout_unknown(doc, smp.input) || layout_unknown(doc, smp.output) {
                continue;
            }
            let targets = match path {
                AnimationPath::Weights => {
                    let n = &doc.nodes[node as usize];
                    let Some(mesh) = n.mesh.and_then(|m| doc.meshes.get(m as usize)) else {
                        r.error(
                            "ANIMATION_CHANNEL_TARGET_NODE_WEIGHTS_NO_MORPHS",
                            format!("{cp}/target"),
                            "weights animated on a node without a morphed mesh",
                        );
                        continue;
                    };
                    mesh.primitives.first().map_or(0, |p| p.targets.len()) as u64
                }
                _ => 1,
            };
            let per_key = if smp.interpolation_or_default() == Interpolation::CubicSpline {
                3
            } else {
                1
            };
            let expected = input
                .count
                .checked_mul(per_key)
                .and_then(|x| x.checked_mul(targets));
            if expected != Some(output.count) {
                r.error(
                    "ANIMATION_SAMPLER_OUTPUT_ACCESSOR_INVALID_COUNT",
                    format!("{ptr}/samplers/{}/output", ch.sampler),
                    format!(
                        "{} output elements for {} keys × {per_key} × {targets}",
                        output.count, input.count
                    ),
                );
            }
            let ok = match path {
                AnimationPath::Translation | AnimationPath::Scale => {
                    output.type_ == AccessorType::Vec3
                        && output.component_type == ComponentType::Float
                }
                AnimationPath::Rotation => {
                    output.type_ == AccessorType::Vec4
                        && (output.component_type == ComponentType::Float
                            || (output.normalized_or_default()
                                && output.component_type != ComponentType::UnsignedInt))
                }
                AnimationPath::Weights => {
                    output.type_ == AccessorType::Scalar
                        && (output.component_type == ComponentType::Float
                            || (output.normalized_or_default()
                                && output.component_type != ComponentType::UnsignedInt))
                }
                _ => true,
            };
            if !ok {
                r.error(
                    "ANIMATION_SAMPLER_OUTPUT_ACCESSOR_FORMAT",
                    format!("{ptr}/samplers/{}/output", ch.sampler),
                    format!(
                        "{} {:?} cannot animate {}",
                        output.type_.as_str(),
                        output.component_type,
                        path.as_str()
                    ),
                );
            }
        }
    }
}

/// [`validate`] plus checks of the binary data. `buffers[i]` holds `doc.buffers[i]`'s bytes.
pub fn validate_data<B: AsRef<[u8]>>(doc: &Document, buffers: &[B]) -> Report {
    let mut r = validate(doc);
    if !r.is_ok() {
        // Structural errors make data checks unreliable (and noisy).
        return r;
    }
    let mut decoded: Vec<Option<AccessorData>> = Vec::with_capacity(doc.accessors.len());
    for (i, acc) in doc.accessors.iter().enumerate() {
        let ptr = format!("/accessors/{i}");
        if layout_unknown(doc, i as u32) {
            decoded.push(None);
            continue;
        }
        let d = match read_accessor(doc, buffers, i as u32) {
            Ok(d) => d,
            Err(crate::GltfError::Compressed { .. }) => {
                decoded.push(None);
                continue;
            }
            Err(e) => {
                r.error("ACCESSOR_DATA", &ptr, e.to_string());
                decoded.push(None);
                continue;
            }
        };
        if let (Some(min), Some(max), Some((amin, amax))) = (&acc.min, &acc.max, d.min_max()) {
            // Compare at the precision of the component type (f32 for FLOAT).
            let same = |a: f64, b: f64| {
                if acc.component_type == ComponentType::Float {
                    #[allow(clippy::cast_possible_truncation)]
                    let eq = (a as f32) == (b as f32);
                    eq
                } else {
                    a == b
                }
            };
            for (k, (&declared, &actual)) in min.iter().zip(&amin).enumerate() {
                if !same(declared, actual) {
                    r.error(
                        "ACCESSOR_MIN_MISMATCH",
                        format!("{ptr}/min/{k}"),
                        format!("declared {declared}, data {actual}"),
                    );
                }
            }
            for (k, (&declared, &actual)) in max.iter().zip(&amax).enumerate() {
                if !same(declared, actual) {
                    r.error(
                        "ACCESSOR_MAX_MISMATCH",
                        format!("{ptr}/max/{k}"),
                        format!("declared {declared}, data {actual}"),
                    );
                }
            }
        }
        if let Some(sp) = &acc.sparse
            && let Ok(ix) = read_accessor_sparse_indices(doc, buffers, sp)
            && let Some(k) = ix.windows(2).position(|w| w[0] >= w[1])
        {
            r.error(
                "ACCESSOR_SPARSE_INDICES_NON_INCREASING",
                format!("{ptr}/sparse/indices"),
                format!("sparse index {} is not greater than {}", ix[k + 1], ix[k]),
            );
        }
        decoded.push(Some(d));
    }
    for (m, mesh) in doc.meshes.iter().enumerate() {
        for (p, prim) in mesh.primitives.iter().enumerate() {
            let Some(ia) = prim.indices else { continue };
            let Some(Some(d)) = decoded.get(ia as usize) else {
                continue;
            };
            let Ok(ix) = d.to_u32() else { continue };
            let vcount = prim
                .attributes
                .values()
                .next()
                .and_then(|&a| doc.accessors.get(a as usize))
                .map_or(0, |a| a.count);
            let restart = match d.component_type() {
                ComponentType::UnsignedByte => u32::from(u8::MAX),
                ComponentType::UnsignedShort => u32::from(u16::MAX),
                _ => u32::MAX,
            };
            let ptr = format!("/meshes/{m}/primitives/{p}/indices");
            if let Some(k) = ix.iter().position(|&i| i == restart) {
                r.error(
                    "ACCESSOR_INDEX_PRIMITIVE_RESTART",
                    &ptr,
                    format!("index {k} is the primitive restart value {restart}"),
                );
            } else if let Some(k) = ix.iter().position(|&i| u64::from(i) >= vcount) {
                r.error(
                    "ACCESSOR_INDEX_OOB",
                    &ptr,
                    format!("index {k} = {} is past the {vcount} vertices", ix[k]),
                );
            }
        }
    }
    for (a, anim) in doc.animations.iter().enumerate() {
        for (s, smp) in anim.samplers.iter().enumerate() {
            let Some(Some(d)) = decoded.get(smp.input as usize) else {
                continue;
            };
            let t = d.to_f32();
            let ptr = format!("/animations/{a}/samplers/{s}/input");
            if t.first().is_some_and(|&t0| t0 < 0.0) {
                r.error(
                    "ACCESSOR_ANIMATION_INPUT_NEGATIVE",
                    &ptr,
                    "keyframe times must be ≥ 0",
                );
            }
            if let Some(k) = t.windows(2).position(|w| w[0] >= w[1]) {
                r.error(
                    "ACCESSOR_ANIMATION_INPUT_NON_INCREASING",
                    &ptr,
                    format!(
                        "time {} at key {} is not greater than {}",
                        t[k + 1],
                        k + 1,
                        t[k]
                    ),
                );
            }
        }
    }
    r
}

fn read_accessor_sparse_indices<B: AsRef<[u8]>>(
    doc: &Document,
    buffers: &[B],
    sp: &crate::json::Sparse,
) -> Result<Vec<u32>, crate::GltfError> {
    let mut acc = crate::json::Accessor::new(
        Some(sp.indices.buffer_view),
        sp.indices.component_type,
        AccessorType::Scalar,
        sp.count,
    );
    acc.byte_offset = sp.indices.byte_offset;
    crate::accessor::read_accessor_obj(doc, buffers, &acc)?.to_u32()
}

/// The entries of `extensionsRequired` that `supported` does not accept, in file order. A
/// loader must refuse the file when this is not empty (glTF 2.0 §3.12).
pub fn unsupported_required(doc: &Document, supported: impl Fn(&str) -> bool) -> Vec<&str> {
    doc.extensions_required
        .iter()
        .map(String::as_str)
        .filter(|n| !supported(n))
        .collect()
}

/// Validates a GLB: [`validate`], the container's buffer rules (buffer 0 without `uri` is
/// the BIN chunk, whose length is `byteLength` rounded up to 4 at most), and, when no buffer
/// needs an external file, [`validate_data`].
pub fn validate_glb(glb: &Glb) -> Report {
    let doc = &glb.document;
    let b0 = doc.buffers.first().filter(|b| b.uri.is_none());
    let mut container = Report::default();
    match (b0, &glb.bin) {
        (Some(_), None) => container.error(
            "BUFFER_MISSING_GLB_DATA",
            "/buffers/0",
            "buffer 0 has no uri but the GLB has no BIN chunk",
        ),
        (None, Some(_)) => container.warning(
            "GLB_UNUSED_BIN_CHUNK",
            "",
            "the BIN chunk is not referenced by buffer 0",
        ),
        (Some(b), Some(bin)) => {
            let len = bin.len() as u64;
            if b.byte_length > len {
                container.error(
                    "BUFFER_GLB_CHUNK_TOO_BIG",
                    "/buffers/0/byteLength",
                    format!(
                        "byteLength {} exceeds the {len}-byte BIN chunk",
                        b.byte_length
                    ),
                );
            } else if len - b.byte_length > 3 {
                container.error(
                    "BUFFER_BYTE_LENGTH_MISMATCH",
                    "/buffers/0/byteLength",
                    format!(
                        "BIN chunk is {len} bytes, more than 3 beyond byteLength {}",
                        b.byte_length
                    ),
                );
            }
        }
        (None, None) => {}
    }
    let self_contained = doc
        .buffers
        .iter()
        .all(|b| b.uri.as_deref().is_none_or(|u| u.starts_with("data:")));
    let mut r = if self_contained && container.is_ok() {
        match load_buffers(doc, glb.bin.as_deref(), |_| Err("external".into())) {
            Ok(bufs) => validate_data(doc, &bufs),
            Err(e) => {
                let mut r = validate(doc);
                r.error("BUFFER_DATA", "/buffers", e.to_string());
                r
            }
        }
    } else {
        validate(doc)
    };
    container.issues.append(&mut r.issues);
    container
}

/// Validates a `.glb` or `.gltf` file from its bytes: [`validate_glb`] (data checks included
/// when every buffer is embedded) plus [`json_text_issues`]. Fails only when the file cannot be
/// read at all (bad container, invalid JSON, a property of the wrong type).
pub fn validate_file(bytes: &[u8]) -> Result<Report, crate::GltfError> {
    let (json, bin) = if is_glb(bytes) {
        split_glb(bytes)?
    } else {
        (bytes, None)
    };
    let glb = Glb {
        document: Document::from_slice(json)?,
        bin,
    };
    let mut r = validate_glb(&glb);
    json_text_issues(json, &glb.document, &mut r);
    Ok(r)
}

/// Issues visible only in the JSON text `doc` was parsed from: integer properties written
/// with a fraction (INTEGER_WRITTEN_AS_FLOAT, warning), and members the document lost
/// (UNEXPECTED_PROPERTY; since WP 7.4 the reader keeps unknown members in
/// [`Document::unknown`], which [`validate`] reports, so this only fires for a document that
/// was not parsed from `json`). `extensions` and `extras` values are kept verbatim and not
/// inspected.
pub fn json_text_issues(json: &[u8], doc: &Document, r: &mut Report) {
    let Ok(src) = serde_json::from_slice::<serde_json::Value>(trim_json(json)) else {
        return;
    };
    let mut ptr = String::new();
    compare_json(&src, &doc.to_value_with_unknown(), &mut ptr, r);
}

fn compare_json(
    src: &serde_json::Value,
    kept: &serde_json::Value,
    ptr: &mut String,
    r: &mut Report,
) {
    use serde_json::Value;
    match (src, kept) {
        (Value::Object(s), Value::Object(k)) => {
            for (key, sv) in s {
                if key == "extensions" || key == "extras" {
                    continue;
                }
                let len = ptr.len();
                ptr.push('/');
                ptr.push_str(&escape_pointer_token(key));
                match k.get(key) {
                    Some(kv) => compare_json(sv, kv, ptr, r),
                    // Empty arrays and objects are known members the writer leaves out.
                    None if matches!(sv, Value::Array(a) if a.is_empty())
                        || matches!(sv, Value::Object(o) if o.is_empty()) => {}
                    None => r.warning(
                        "UNEXPECTED_PROPERTY",
                        ptr.as_str(),
                        format!("unexpected property {key:?} (not kept)"),
                    ),
                }
                ptr.truncate(len);
            }
        }
        (Value::Array(s), Value::Array(k)) => {
            for (i, (sv, kv)) in s.iter().zip(k).enumerate() {
                let len = ptr.len();
                ptr.push('/');
                ptr.push_str(&i.to_string());
                compare_json(sv, kv, ptr, r);
                ptr.truncate(len);
            }
        }
        (Value::Number(s), Value::Number(k)) if s.is_f64() && !k.is_f64() => r.warning(
            "INTEGER_WRITTEN_AS_FLOAT",
            ptr.as_str(),
            format!("integer value written as {s}"),
        ),
        _ => {}
    }
}
