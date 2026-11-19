// SPDX-License-Identifier: GPL-3.0-or-later
//! `NodesModifier.node_group`: assigns the group and rebuilds `properties.inputs` from its
//! interface, as `MOD_nodes_update_interface` does (Blender 5.2.4, verified: after
//! `modifiers.new('GN', 'NODES')` and `node_group = bpy.data.node_groups['Smooth by Angle']`,
//! `properties.inputs.Input_1.value` is the group's default 0.5236; with `None`,
//! `properties` is `None`). An input that exists with a value of the same type keeps it;
//! the others get the interface socket's default.

use frap_nodes::{InOut, SocketValue};
use frap_rna::{IdRef, NodeTreeId, PropDef, RnaError, RnaPtr, Value, rna_hook};
use indexmap::IndexMap;

use crate::Host;
use crate::id::paths::{internal, object_of};
use crate::id::{GnInput, GnInputs, GnValue, Modifier, ModifierSettings};

fn value_of(v: &SocketValue) -> GnValue {
    match v {
        SocketValue::Float(f) => GnValue::Float(*f),
        SocketValue::Int(i) => GnValue::Int(*i),
        SocketValue::Bool(b) => GnValue::Bool(*b),
        SocketValue::Vector(v) => GnValue::Vector([v[0], v[1], v[2]]),
        SocketValue::Color(c) => GnValue::Color(*c),
        SocketValue::String(s) => GnValue::String(s.to_string()),
        _ => GnValue::None,
    }
}

#[rna_hook(srna = "NodesModifier", prop = "node_group", krate = crate)]
fn set(h: &mut dyn Host, this: &RnaPtr, _prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
    let group = match v {
        Value::Id(Some(a)) => Some(a),
        _ => None,
    };
    let ob = object_of(this)?;
    let (main, log) = h.main_mut();
    let interface: Vec<(smol_str::SmolStr, GnValue)> = group
        .and_then(|g| g.typed::<NodeTreeId>())
        .and_then(|g: IdRef<NodeTreeId>| main.node_groups.get(g))
        .map(|t| {
            t.interface
                .iter()
                .filter(|s| s.in_out == InOut::Input)
                .map(|s| (s.identifier.clone(), value_of(&s.default_value)))
                .collect()
        })
        .unwrap_or_default();
    let o = main
        .objects
        .get(ob)
        .ok_or_else(|| internal("NodesModifier.node_group: the object was removed"))?;
    let i = Modifier::index_in(&o.modifiers, &this.path)
        .ok_or_else(|| internal("NodesModifier.node_group: not a modifier pointer"))?;
    let Some(o) = main.objects.get_mut(ob, log) else {
        return Ok(());
    };
    let ModifierSettings::Nodes(s) = &mut o.modifiers[i].settings else {
        return Err(internal("NodesModifier.node_group: not a NODES modifier"));
    };
    s.node_group = group;
    let mut inputs = IndexMap::new();
    for (id, default) in interface {
        let kept =
            s.inputs.0.get(&id).filter(|old| {
                std::mem::discriminant(&old.value) == std::mem::discriminant(&default)
            });
        let input = kept.cloned().unwrap_or_else(|| GnInput::new(default));
        inputs.insert(id, input);
    }
    s.inputs = GnInputs(inputs);
    Ok(())
}
