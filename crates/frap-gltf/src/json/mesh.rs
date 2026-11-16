// SPDX-License-Identifier: GPL-3.0-or-later
//! `meshes[]` and their primitives.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use super::{Extensions, Extras, Index, gl_enum};

gl_enum! {
    /// Primitive topology.
    Mode {
        /// 0
        Points = 0,
        /// 1
        Lines = 1,
        /// 2
        LineLoop = 2,
        /// 3
        LineStrip = 3,
        /// 4 (default)
        Triangles = 4,
        /// 5
        TriangleStrip = 5,
        /// 6
        TriangleFan = 6,
    }
}

impl Mode {
    /// Every mode, in code order.
    pub const ALL: [Mode; 7] = [
        Mode::Points,
        Mode::Lines,
        Mode::LineLoop,
        Mode::LineStrip,
        Mode::Triangles,
        Mode::TriangleStrip,
        Mode::TriangleFan,
    ];
}

/// A mesh: one or more primitives sharing morph-target weights.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mesh {
    /// Extensions.
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    /// Application data (`targetNames` lives here by convention).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extras: Option<Extras>,
    /// Name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Primitives (at least one).
    pub primitives: Vec<Primitive>,
    /// Default morph target weights.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub weights: Vec<f64>,
}

/// One draw call: attributes, optional indices, material and topology.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Primitive {
    /// Semantic (`POSITION`, `NORMAL`, `TEXCOORD_0`, …) → accessor, in file order.
    pub attributes: IndexMap<String, Index>,
    /// Extensions.
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    /// Application data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extras: Option<Extras>,
    /// Index accessor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub indices: Option<Index>,
    /// Material.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<Index>,
    /// Topology (default TRIANGLES).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<Mode>,
    /// Morph targets: semantic → accessor of deltas.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub targets: Vec<IndexMap<String, Index>>,
}

impl Primitive {
    /// Effective topology.
    pub fn mode_or_default(&self) -> Mode {
        self.mode.unwrap_or(Mode::Triangles)
    }
}
