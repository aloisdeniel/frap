// SPDX-License-Identifier: GPL-3.0-or-later
//! `Modifier.name`: unique among the object's modifiers (sub-data name, 63 bytes,
//! `BLI_uniquename`); an empty name gives the type's default name (verified on 5.2.4:
//! renaming `B` to the taken `A` gives `A.001`; `''` on a Bevel gives `Bevel`).

use frap_rna::naming::unique_subname;
use frap_rna::{PropDef, RnaError, RnaPtr, Seg, Value, rna_hook};
use smol_str::SmolStr;

use crate::Host;
use crate::generated::modifier_settings::ModifierType;
use crate::id::paths::{internal, object_of};

#[rna_hook(srna = "Modifier", prop = "name", krate = crate)]
fn set(h: &mut dyn Host, this: &RnaPtr, _prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
    let ob = object_of(this)?;
    let requested = v.as_str().unwrap_or_default().to_owned();
    let o = h
        .main()
        .objects
        .get(ob)
        .ok_or_else(|| internal("Modifier.name: the object was removed"))?;
    let i = match this.path.as_slice() {
        [Seg::Prop("modifiers"), Seg::Key(k)] => o.modifiers.iter().position(|m| m.name == *k),
        [Seg::Prop("modifiers"), Seg::Index(i)] => usize::try_from(*i).ok(),
        _ => None,
    }
    .filter(|i| *i < o.modifiers.len())
    .ok_or_else(|| internal("Modifier.name: not a modifier pointer"))?;
    let ty = o.modifiers[i].settings.type_identifier().to_owned();
    let default = ModifierType::ALL
        .iter()
        .find(|t| t.identifier() == ty)
        .and_then(|t| t.default_name())
        .unwrap_or("Modifier");
    let others: Vec<SmolStr> = o
        .modifiers
        .iter()
        .enumerate()
        .filter(|(j, _)| *j != i)
        .map(|(_, m)| m.name.clone())
        .collect();
    let name = unique_subname(&|n| others.iter().any(|x| x == n), &requested, 63, default);
    let (main, log) = h.main_mut();
    if let Some(o) = main.objects.get_mut(ob, log) {
        o.modifiers[i].name = name;
    }
    Ok(())
}
