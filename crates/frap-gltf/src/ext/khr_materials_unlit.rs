// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_unlit`: typed struct + validation (§17.1). WP 3.5.
//!
//! An empty object on a material: the material is shaded with its base colour only. Clients
//! that do not support it fall back to the PBR parameters, which io_scene_gltf2 writes as
//! `metallicFactor` 0 and `roughnessFactor` 0.9. The add-on writes it as not required
//! (`blender/exp/material/materials.py`: `Extension("KHR_materials_unlit", {}, required=False)`)
//! and keeps it although it is empty (`__should_include_json_value`, `blender/exp/export.py`).

use serde::{Deserialize, Serialize};

use crate::json::{EXTENSIONS, Extension, ExtensionCx, ExtensionDef, Extensions, Extras};
use crate::validate::Report;

/// The unlit marker (no properties).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KhrMaterialsUnlit {
    /// Extensions.
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    /// Application data.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::json::extensions::de_extras"
    )]
    pub extras: Option<Extras>,
}

impl Extension for KhrMaterialsUnlit {
    const NAME: &'static str = "KHR_materials_unlit";
}

fn validate(cx: &ExtensionCx, r: &mut Report) {
    if cx.owner_index_in("materials").is_none() {
        cx.wrong_location(r, "a material");
        return;
    }
    let _ = cx.decode::<KhrMaterialsUnlit>(r);
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(EXTENSIONS)]
static DEF: ExtensionDef = ExtensionDef {
    name: KhrMaterialsUnlit::NAME,
    required_by_default: KhrMaterialsUnlit::REQUIRED_BY_DEFAULT,
    validate,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::{Document, Material};
    use crate::validate::validate;

    #[test]
    fn typed_access_and_validation() {
        let mut d = Document::default();
        let mut m = Material::default();
        m.extensions.set(Some(KhrMaterialsUnlit::default()));
        d.materials.push(m);
        d.use_extension(
            KhrMaterialsUnlit::NAME,
            KhrMaterialsUnlit::REQUIRED_BY_DEFAULT,
        );
        assert!(d.extensions_required.is_empty());
        assert!(validate(&d).is_ok(), "{}", validate(&d));
        let json = String::from_utf8(d.to_vec(&Default::default())).unwrap();
        assert!(
            json.contains(r#"{"KHR_materials_unlit":{}}"#),
            "the empty object is written: {json}"
        );
        assert!(d.materials[0].extensions.contains(KhrMaterialsUnlit::NAME));
        let e: KhrMaterialsUnlit = d.materials[0].extensions.get().unwrap().unwrap();
        assert_eq!(e, KhrMaterialsUnlit::default());
        // On a node it is misplaced.
        let mut d2 = d.clone();
        d2.materials[0].extensions.set::<KhrMaterialsUnlit>(None);
        let mut n = crate::json::Node::default();
        n.extensions.set(Some(KhrMaterialsUnlit::default()));
        d2.nodes.push(n);
        assert!(validate(&d2).has("EXTENSION_INVALID_LOCATION"));
    }
}
