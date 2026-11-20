// SPDX-License-Identifier: GPL-3.0-or-later
//! Pose matrices (WP 6.2): `BKE_pose_where_is` for an armature object — each pose channel's
//! armature-space matrix from its rest matrix, its local transform (`location`,
//! `rotation_mode` rotation, `scale`) and its parent's, with `use_connect`,
//! `use_inherit_rotation`, `inherit_scale` and `use_local_location`; `pose_position='REST'`
//! gives the rest pose. The ARMATURE modifier (WP 6.5) and glTF skins (WP 6.8) read the result
//! through `EvaluatedObject::pose`.
//!
//! M5+M6 W0: [`evaluate`] is called for every evaluated object (`EvalCache`); it answers `None`
//! until WP 6.2 lands.

use std::sync::Arc;

use frap_scene::id::Object;
use frap_scene::{EvaluatedPose, HostRead};

/// The evaluated pose of `ob` (`None` for objects that are not armatures).
pub(crate) fn evaluate(host: &dyn HostRead, ob: &Object) -> Option<Arc<EvaluatedPose>> {
    let _ = (host, ob);
    None
}
