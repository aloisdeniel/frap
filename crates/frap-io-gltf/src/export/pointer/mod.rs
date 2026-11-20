// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_animation_pointer` export (WP 6.9, `export/pointer/**`): material (embedded node tree,
//! `NT` slots), light and camera property animation as `pointer` channels
//! (`/materials/{i}/…`, `/extensions/KHR_lights_punctual/lights/{i}/…`, `/cameras/{i}/…`),
//! ported from io_scene_gltf2 5.2.41 (`export_pointer_animation`). Called by the animation
//! exporter (WP 6.7) for the channels it does not map to TRS/weights. M5+M6 W0 stub.
