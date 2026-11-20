// SPDX-License-Identifier: GPL-3.0-or-later
//! Keying (WP 6.4, ARCHITECTURE §18): `bpy_struct.keyframe_insert(data_path, *, index=-1,
//! frame=None, group="", options=set(), keytype='KEYFRAME') -> bool` and `keyframe_delete`,
//! shared by the RNA functions and the `anim.keyframe_*` operators. Creates `<IDName>Action`,
//! the slot (`OB<name>`, `KE<name>` in the object's action for shape-key values, `NT<name>`
//! for an embedded node tree, addressed through `Root::Embedded`), one layer, one keyframe
//! strip, the group (`Object Transforms`, or the bone name) and BEZIER / AUTO_CLAMPED keys
//! from the preferences; handles recalculated. Stub until WP 6.4 lands (M5+M6 W0).
