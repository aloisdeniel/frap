// SPDX-License-Identifier: GPL-3.0-or-later
//! RNA of a group interface (WP 3.1): `NodeTree.interface` (`NodeTreeInterface`: `items_tree`,
//! `active_index`, `active`) and its socket items (`NodeTreeInterfaceSocket<Type>`).
//!
//! The typed item structs (`NodeTreeInterfaceSocketFloat`, …: `default_value`, `min_value`,
//! `max_value`, `subtype`, `dimensions`) are not in the generated RNA tables (the spec dump
//! stops at `NodeTreeInterfaceSocket`), so their `StructDef`s are written here from Blender
//! 5.2.4's `bl_rna` (names, descriptions, ranges and defaults verified).
//!
//! Verified on 5.2.4: an item reads `description` "", `hide_value`/`hide_in_modifier`/
//! `force_non_field` False, `attribute_domain` 'POINT', `default_input` 'VALUE',
//! `structure_type` 'AUTO', `item_type` 'SOCKET', `min_value`/`max_value` ±3.4028e38 (int:
//! ±2³¹), `subtype` 'NONE', `index`/`position` its place in `items_tree`; `active_index` stays 0
//! while sockets are added. Writing `name` renames the sockets of the group's input/output
//! nodes (group nodes in other trees: the `NodeTreeInterfaceSocket.name` set hook); writing
//! `default_value` changes no existing socket.

use frap_rna::traits::{index_out_of_range, read_only, wrong_access};
use frap_rna::*;
use smol_str::SmolStr;

use super::{as_bool, as_f32, as_floats, as_str, floats, internal, str_val, unbound, val};
use crate::{InOut, InterfaceSocket, NodeTree, SocketType, SocketValue, interface_socket_idname};

// ------------------------------------------------------------------- typed item structs

const MIN_F: f64 = -3.402_823_466_385_288_6e38;
const MAX_F: f64 = 3.402_823_466_385_288_6e38;

static SUBTYPE_ITEMS: [EnumItem; 1] = [EnumItem {
    identifier: "NONE",
    name: "None",
    description: "",
    value: 0,
}];

const fn prop(
    identifier: &'static str,
    name: &'static str,
    description: &'static str,
    kind: PropKind,
    subtype: Subtype,
    array: ArrayShape,
) -> PropDef {
    PropDef {
        identifier,
        name,
        description,
        kind,
        subtype,
        unit: Unit::None,
        array,
        flags: PropFlags::empty(),
        deprecated: None,
    }
}

const SUBTYPE: PropDef = prop(
    "subtype",
    "Subtype",
    "Subtype of the default value",
    PropKind::Enum {
        items: &SUBTYPE_ITEMS,
        default: EnumDefault::Item("NONE"),
        dynamic: None,
    },
    Subtype::None,
    ArrayShape::Scalar,
);

const fn float(identifier: &'static str, name: &'static str, description: &'static str) -> PropDef {
    prop(
        identifier,
        name,
        description,
        PropKind::Float {
            default: Defaults::Scalar(0.0),
            hard: (MIN_F, MAX_F),
            soft: (-10000.0, 10000.0),
            precision: 3,
            storage: FloatStorage::F32,
        },
        Subtype::None,
        ArrayShape::Scalar,
    )
}

const fn int(identifier: &'static str, name: &'static str, description: &'static str) -> PropDef {
    prop(
        identifier,
        name,
        description,
        PropKind::Int {
            default: Defaults::Scalar(0),
            hard: (-2_147_483_648, 2_147_483_647),
            soft: (-10000, 10000),
        },
        Subtype::None,
        ArrayShape::Scalar,
    )
}

const DEFAULT_DESC: &str = "Input value used for unconnected socket";

static FLOAT_PROPS: [PropDef; 4] = [
    SUBTYPE,
    float("default_value", "Default Value", DEFAULT_DESC),
    float("min_value", "Minimum Value", "Minimum value"),
    float("max_value", "Maximum Value", "Maximum value"),
];
static INT_PROPS: [PropDef; 4] = [
    SUBTYPE,
    int("default_value", "Default Value", DEFAULT_DESC),
    int("min_value", "Minimum Value", "Minimum value"),
    int("max_value", "Maximum Value", "Maximum value"),
];
static BOOL_PROPS: [PropDef; 1] = [prop(
    "default_value",
    "Default Value",
    DEFAULT_DESC,
    PropKind::Bool {
        default: Defaults::Scalar(false),
    },
    Subtype::None,
    ArrayShape::Scalar,
)];
static VECTOR_PROPS: [PropDef; 5] = [
    SUBTYPE,
    prop(
        "dimensions",
        "Dimensions",
        "Dimensions of the vector socket",
        PropKind::Int {
            default: Defaults::Scalar(3),
            hard: (2, 4),
            soft: (2, 4),
        },
        Subtype::None,
        ArrayShape::Scalar,
    ),
    prop(
        "default_value",
        "Default Value",
        DEFAULT_DESC,
        PropKind::Float {
            default: Defaults::Array(&[0.0, 0.0, 0.0]),
            hard: (MIN_F, MAX_F),
            soft: (-10000.0, 10000.0),
            precision: 3,
            storage: FloatStorage::F32,
        },
        Subtype::None,
        ArrayShape::Fixed(3),
    ),
    float("min_value", "Minimum Value", "Minimum value"),
    float("max_value", "Maximum Value", "Maximum value"),
];
static COLOR_PROPS: [PropDef; 1] = [prop(
    "default_value",
    "Default Value",
    DEFAULT_DESC,
    PropKind::Float {
        default: Defaults::Array(&[0.0, 0.0, 0.0, 0.0]),
        hard: (0.0, MAX_F),
        soft: (0.0, 1.0),
        precision: 3,
        storage: FloatStorage::F32,
    },
    Subtype::Color,
    ArrayShape::Fixed(4),
)];
static MENU_PROPS: [PropDef; 1] = [prop(
    "default_value",
    "Default Value",
    DEFAULT_DESC,
    PropKind::String {
        default: "",
        max_bytes: 0,
    },
    Subtype::None,
    ArrayShape::Scalar,
)];

macro_rules! item_struct {
    ($name:ident, $id:literal, $ui:literal, $desc:literal, $props:expr) => {
        #[doc = concat!("`", $id, "` (hand-written, see the module docs).")]
        pub static $name: StructDef = StructDef {
            identifier: $id,
            base: Some(&frap_spec_rna::NODE_TREE_INTERFACE_SOCKET),
            name: $ui,
            description: $desc,
            props: $props,
            funcs: &[],
            name_property: Some("name"),
            is_id: false,
        };
    };
}

item_struct!(
    NODE_TREE_INTERFACE_SOCKET_FLOAT,
    "NodeTreeInterfaceSocketFloat",
    "Float Node Socket Interface",
    "Floating-point number socket of a node",
    &FLOAT_PROPS
);
item_struct!(
    NODE_TREE_INTERFACE_SOCKET_INT,
    "NodeTreeInterfaceSocketInt",
    "Integer Node Socket Interface",
    "Integer number socket of a node",
    &INT_PROPS
);
item_struct!(
    NODE_TREE_INTERFACE_SOCKET_BOOL,
    "NodeTreeInterfaceSocketBool",
    "Boolean Node Socket Interface",
    "Boolean value socket of a node",
    &BOOL_PROPS
);
item_struct!(
    NODE_TREE_INTERFACE_SOCKET_VECTOR,
    "NodeTreeInterfaceSocketVector",
    "Vector Node Socket Interface",
    "3D vector socket of a node",
    &VECTOR_PROPS
);
item_struct!(
    NODE_TREE_INTERFACE_SOCKET_COLOR,
    "NodeTreeInterfaceSocketColor",
    "Color Node Socket Interface",
    "RGBA color socket of a node",
    &COLOR_PROPS
);
item_struct!(
    NODE_TREE_INTERFACE_SOCKET_SHADER,
    "NodeTreeInterfaceSocketShader",
    "Shader Node Socket Interface",
    "Shader socket of a node",
    &[]
);
item_struct!(
    NODE_TREE_INTERFACE_SOCKET_MENU,
    "NodeTreeInterfaceSocketMenu",
    "Menu Node Socket Interface",
    "Menu socket of a node",
    &MENU_PROPS
);

/// The RNA struct of an interface item of type `ty`.
pub fn item_srna(ty: SocketType) -> &'static StructDef {
    match ty {
        SocketType::Value => &NODE_TREE_INTERFACE_SOCKET_FLOAT,
        SocketType::Int => &NODE_TREE_INTERFACE_SOCKET_INT,
        SocketType::Boolean => &NODE_TREE_INTERFACE_SOCKET_BOOL,
        SocketType::Vector => &NODE_TREE_INTERFACE_SOCKET_VECTOR,
        SocketType::Rgba => &NODE_TREE_INTERFACE_SOCKET_COLOR,
        SocketType::Shader => &NODE_TREE_INTERFACE_SOCKET_SHADER,
        SocketType::Menu => &NODE_TREE_INTERFACE_SOCKET_MENU,
        _ => &frap_spec_rna::NODE_TREE_INTERFACE_SOCKET,
    }
}

/// The index of the interface item a pointer path ends on (`…interface.items_tree[i]` or
/// `…interface.items_tree['Name']`), for hooks and RNA functions.
pub fn item_index_of(t: &NodeTree, path: &[Seg]) -> Option<usize> {
    match path {
        [.., Seg::Prop("interface"), Seg::Prop("items_tree"), seg] => match seg {
            Seg::Index(i) => usize::try_from(*i).ok().filter(|i| *i < t.interface.len()),
            Seg::Key(k) => t.interface.iter().position(|s| s.name == k.as_str()),
            Seg::Prop(_) => None,
        },
        _ => None,
    }
}

// ------------------------------------------------------------------------- interface

static SOCKET_TYPE_ITEMS: [EnumItem; 9] = [
    EnumItem {
        identifier: "NodeSocketFloat",
        name: "Float",
        description: "Floating-point number socket of a node",
        value: 0,
    },
    EnumItem {
        identifier: "NodeSocketInt",
        name: "Integer",
        description: "Integer number socket of a node",
        value: 1,
    },
    EnumItem {
        identifier: "NodeSocketBool",
        name: "Boolean",
        description: "Boolean value socket of a node",
        value: 2,
    },
    EnumItem {
        identifier: "NodeSocketVector",
        name: "Vector",
        description: "3D vector socket of a node",
        value: 3,
    },
    EnumItem {
        identifier: "NodeSocketColor",
        name: "Color",
        description: "RGBA color socket of a node",
        value: 4,
    },
    EnumItem {
        identifier: "NodeSocketMenu",
        name: "Menu",
        description: "Menu socket of a node",
        value: 5,
    },
    EnumItem {
        identifier: "NodeSocketShader",
        name: "Shader",
        description: "Shader socket of a node",
        value: 6,
    },
    EnumItem {
        identifier: "NodeSocketBundle",
        name: "Bundle",
        description: "Bundle socket of a node",
        value: 7,
    },
    EnumItem {
        identifier: "NodeSocketClosure",
        name: "Closure",
        description: "Closure socket of a node",
        value: 8,
    },
];

/// `new_socket` with the `socket_type` items of a shader tree. Blender fills them from a
/// callback (`rna_NodeTreeInterfaceSocket_socket_type_itemf`); the class-level dump has only the
/// placeholder 'DEFAULT', so the generated `FuncDef` would refuse every real type. Items, names
/// and descriptions verified on 5.2.4 (TypeError list, `UILayout.enum_item_name/description`);
/// an omitted `socket_type` gives a float socket.
static NEW_SOCKET: [FuncDef; 1] = [FuncDef {
    identifier: "new_socket",
    description: "Add a new socket to the interface",
    params: &[
        PropDef {
            identifier: "name",
            name: "Name",
            description: "Name of the socket",
            kind: PropKind::String {
                default: "",
                max_bytes: 0,
            },
            subtype: Subtype::None,
            unit: Unit::None,
            array: ArrayShape::Scalar,
            flags: PropFlags::REQUIRED.union(PropFlags::NEVER_NULL),
            deprecated: None,
        },
        PropDef {
            identifier: "description",
            name: "Description",
            description: "Description of the socket",
            kind: PropKind::String {
                default: "",
                max_bytes: 0,
            },
            subtype: Subtype::None,
            unit: Unit::None,
            array: ArrayShape::Scalar,
            flags: PropFlags::NEVER_NULL,
            deprecated: None,
        },
        PropDef {
            identifier: "in_out",
            name: "Input/Output Type",
            description: "Create an input or output socket",
            kind: PropKind::Enum {
                items: &frap_spec_rna::generated::enums::E175,
                default: EnumDefault::Item("INPUT"),
                dynamic: None,
            },
            subtype: Subtype::None,
            unit: Unit::None,
            array: ArrayShape::Scalar,
            flags: PropFlags::ANIMATABLE,
            deprecated: None,
        },
        PropDef {
            identifier: "socket_type",
            name: "Socket Type",
            description: "Type of socket generated on nodes",
            kind: PropKind::Enum {
                items: &SOCKET_TYPE_ITEMS,
                default: EnumDefault::Item("NodeSocketFloat"),
                dynamic: None,
            },
            subtype: Subtype::None,
            unit: Unit::None,
            array: ArrayShape::Scalar,
            flags: PropFlags::ANIMATABLE,
            deprecated: None,
        },
        PropDef {
            identifier: "parent",
            name: "Parent",
            description: "Panel to add the socket in",
            kind: PropKind::Pointer {
                fixed_type: "NodeTreeInterfacePanel",
                is_id: false,
            },
            subtype: Subtype::None,
            unit: Unit::None,
            array: ArrayShape::Scalar,
            flags: PropFlags::READONLY,
            deprecated: None,
        },
    ],
    returns: &[PropDef {
        identifier: "item",
        name: "Socket",
        description: "New socket",
        kind: PropKind::Pointer {
            fixed_type: "NodeTreeInterfaceSocket",
            is_id: false,
        },
        subtype: Subtype::None,
        unit: Unit::None,
        array: ArrayShape::Scalar,
        flags: PropFlags::READONLY.union(PropFlags::OUTPUT),
        deprecated: None,
    }],
    keyword_only_from: None,
    flags: FuncFlags::empty(),
}];

/// `NodeTreeInterface` as frap serves it: the generated struct as base (every property and
/// the other functions) with the corrected `new_socket` in front, so `find_func` finds it first.
pub static NODE_TREE_INTERFACE: StructDef = StructDef {
    identifier: "NodeTreeInterface",
    base: Some(&frap_spec_rna::NODE_TREE_INTERFACE),
    name: "Node Tree Interface",
    description: "Declaration of sockets and ui panels of a node group",
    props: &[],
    funcs: &NEW_SOCKET,
    name_property: None,
    is_id: false,
};

/// `node_tree.interface` (read proxy).
pub struct InterfaceRef<'a> {
    /// The tree.
    pub tree: &'a NodeTree,
}

/// `node_tree.interface` (write proxy).
pub struct InterfaceMut<'a> {
    /// The tree.
    pub tree: &'a mut NodeTree,
}

fn interface_get<'a>(t: &'a NodeTree, prop: &'static PropDef) -> Result<Field<'a>, RnaError> {
    match prop.identifier {
        "items_tree" => Ok(Field::Coll(CollRef::Owned(Box::new(ItemsColl { tree: t })))),
        // Not stored: 0 until the UI selects an item (frap has no UI).
        "active_index" => val(Value::Int(0)),
        "active" => match t.interface.is_empty() {
            true => val(Value::None),
            false => Ok(Field::Struct(StructRef::Owned(Box::new(ItemRef {
                tree: t,
                i: 0,
            })))),
        },
        id => Err(unbound(&NODE_TREE_INTERFACE, id)),
    }
}

impl Rna for InterfaceRef<'_> {
    fn srna(&self) -> &'static StructDef {
        &NODE_TREE_INTERFACE
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        interface_get(self.tree, prop)
    }
}

impl Rna for InterfaceMut<'_> {
    fn srna(&self) -> &'static StructDef {
        &NODE_TREE_INTERFACE
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        interface_get(self.tree, prop)
    }
}

impl RnaMut for InterfaceMut<'_> {
    fn set(&mut self, prop: &'static PropDef, _v: Value) -> Result<SetOutcome, RnaError> {
        let srna = &NODE_TREE_INTERFACE;
        Err(match prop.identifier {
            id if prop.is_readonly() => read_only(srna, id),
            id => unbound(srna, id),
        })
    }
    fn struct_mut(&mut self, prop: &'static PropDef) -> Result<RnaMutGuard<'_>, RnaError> {
        match prop.identifier {
            "active" if !self.tree.interface.is_empty() => Ok(Box::new(ItemMut {
                tree: &mut *self.tree,
                i: 0,
            })),
            id => Err(wrong_access(&NODE_TREE_INTERFACE, id, "a struct")),
        }
    }
    fn coll_mut(&mut self, prop: &'static PropDef) -> Result<RnaCollMutGuard<'_>, RnaError> {
        match prop.identifier {
            "items_tree" => Ok(Box::new(ItemsCollMut {
                tree: &mut *self.tree,
            })),
            id => Err(wrong_access(&NODE_TREE_INTERFACE, id, "a collection")),
        }
    }
}

// ------------------------------------------------------------------------ items_tree

/// `interface.items_tree` (keyed by item name).
pub struct ItemsColl<'a> {
    /// The tree.
    pub tree: &'a NodeTree,
}

/// Mutable `interface.items_tree`.
pub struct ItemsCollMut<'a> {
    /// The tree.
    pub tree: &'a mut NodeTree,
}

macro_rules! items_coll {
    ($t:ty) => {
        impl RnaColl for $t {
            fn item_type(&self) -> &'static StructDef {
                &frap_spec_rna::NODE_TREE_INTERFACE_SOCKET
            }
            fn len(&self) -> usize {
                self.tree.interface.len()
            }
            fn key_of(&self, i: usize) -> Option<SmolStr> {
                self.tree.interface.get(i).map(|s| s.name.clone())
            }
            fn find(&self, key: &str) -> Option<usize> {
                self.tree.interface.iter().position(|s| s.name == key)
            }
            fn item(&self, i: usize) -> Result<Item<'_>, RnaError> {
                let len = self.tree.interface.len();
                if i >= len {
                    return Err(index_out_of_range(i, len));
                }
                Ok(Item::Struct(StructRef::Owned(Box::new(ItemRef {
                    tree: &*self.tree,
                    i,
                }))))
            }
            /// Verified: `items_tree['A']` reprs as `items_tree[0]`.
            fn item_seg(&self, i: usize) -> Seg {
                Seg::Index(i as i64)
            }
        }
    };
}

items_coll!(ItemsColl<'_>);
items_coll!(ItemsCollMut<'_>);

impl RnaCollMut for ItemsCollMut<'_> {
    fn item_mut(&mut self, i: usize) -> Result<RnaMutGuard<'_>, RnaError> {
        let len = self.tree.interface.len();
        if i >= len {
            return Err(index_out_of_range(i, len));
        }
        Ok(Box::new(ItemMut {
            tree: &mut *self.tree,
            i,
        }))
    }
}

// ------------------------------------------------------------------------------ items

/// An interface socket item (read proxy).
pub struct ItemRef<'a> {
    /// The tree.
    pub tree: &'a NodeTree,
    /// Index in `tree.interface`.
    pub i: usize,
}

/// An interface socket item (write proxy).
pub struct ItemMut<'a> {
    /// The tree.
    pub tree: &'a mut NodeTree,
    /// Index in `tree.interface`.
    pub i: usize,
}

fn in_out_str(io: InOut) -> &'static str {
    match io {
        InOut::Input => "INPUT",
        InOut::Output => "OUTPUT",
    }
}

fn item_get(
    it: &InterfaceSocket,
    i: usize,
    prop: &'static PropDef,
) -> Result<Field<'static>, RnaError> {
    let srna = item_srna(it.socket_type);
    let b = |x: bool| val(Value::Bool(x));
    let e = |x: &'static str| val(Value::Enum(x.into()));
    match prop.identifier {
        "name" => str_val(it.name.clone()),
        "identifier" => str_val(it.identifier.clone()),
        "description" | "default_attribute_name" => str_val(""),
        "socket_type" => e(interface_socket_idname(it.socket_type)),
        "bl_socket_idname" => str_val(interface_socket_idname(it.socket_type)),
        "in_out" => e(in_out_str(it.in_out)),
        "item_type" => e("SOCKET"),
        "index" | "position" => val(Value::Int(i as i64)),
        "hide_value"
        | "hide_in_modifier"
        | "force_non_field"
        | "is_inspect_output"
        | "is_panel_toggle"
        | "layer_selection_field"
        | "menu_expanded"
        | "optional_label" => b(false),
        "attribute_domain" => e("POINT"),
        "structure_type" => e("AUTO"),
        "default_input" => e("VALUE"),
        "subtype" => e("NONE"),
        "dimensions" => val(Value::Int(3)),
        "min_value" | "max_value" => {
            let max = prop.identifier == "max_value";
            match it.socket_type {
                SocketType::Int => val(Value::Int(if max {
                    i64::from(i32::MAX)
                } else {
                    i64::from(i32::MIN)
                })),
                _ => val(Value::Float(if max { f32::MAX } else { f32::MIN })),
            }
        }
        "default_value" => match &it.default_value {
            SocketValue::Float(f) => val(Value::Float(*f)),
            SocketValue::Int(x) => val(Value::Int(i64::from(*x))),
            SocketValue::Bool(x) => b(*x),
            SocketValue::Vector(v) => val(floats(prop, &v[..3])),
            SocketValue::Color(c) => val(floats(prop, c)),
            SocketValue::Menu(s) | SocketValue::String(s) => str_val(s.clone()),
            _ => Err(frap_rna::traits::no_attribute(srna, "default_value")),
        },
        id => Err(unbound(srna, id)),
    }
}

impl Rna for ItemRef<'_> {
    fn srna(&self) -> &'static StructDef {
        item_srna(self.tree.interface[self.i].socket_type)
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        item_get(&self.tree.interface[self.i], self.i, prop)
    }
}

impl Rna for ItemMut<'_> {
    fn srna(&self) -> &'static StructDef {
        item_srna(self.tree.interface[self.i].socket_type)
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        item_get(&self.tree.interface[self.i], self.i, prop)
    }
}

impl RnaMut for ItemMut<'_> {
    fn set(&mut self, prop: &'static PropDef, v: Value) -> Result<SetOutcome, RnaError> {
        let srna = self.srna();
        match prop.identifier {
            "name" => {
                let name = as_str(prop, &v)?.clone();
                self.tree.interface_rename(self.i, &name);
            }
            "default_value" => {
                let it = &mut self.tree.interface[self.i];
                match &mut it.default_value {
                    SocketValue::Float(f) => *f = as_f32(prop, &v)?,
                    SocketValue::Int(x) => match v {
                        Value::Int(i) => *x = i32::try_from(i).map_err(|_| internal(prop, &v))?,
                        _ => return Err(internal(prop, &v)),
                    },
                    SocketValue::Bool(b) => *b = as_bool(prop, &v)?,
                    SocketValue::Vector(x) => {
                        let f = as_floats(prop, &v)?;
                        if f.len() != 3 {
                            return Err(internal(prop, &v));
                        }
                        x[..3].copy_from_slice(f);
                    }
                    SocketValue::Color(x) => {
                        let f = as_floats(prop, &v)?;
                        if f.len() != 4 {
                            return Err(internal(prop, &v));
                        }
                        x.copy_from_slice(f);
                    }
                    SocketValue::Menu(x) | SocketValue::String(x) => {
                        *x = as_str(prop, &v)?.clone();
                    }
                    _ => return Err(frap_rna::traits::no_attribute(srna, "default_value")),
                }
            }
            id if prop.is_readonly() => return Err(read_only(srna, id)),
            id => return Err(unbound(srna, id)),
        }
        Ok(SetOutcome::Done)
    }
    fn struct_mut(&mut self, prop: &'static PropDef) -> Result<RnaMutGuard<'_>, RnaError> {
        Err(wrong_access(self.srna(), prop.identifier, "a struct"))
    }
    fn coll_mut(&mut self, prop: &'static PropDef) -> Result<RnaCollMutGuard<'_>, RnaError> {
        Err(wrong_access(self.srna(), prop.identifier, "a collection"))
    }
}
