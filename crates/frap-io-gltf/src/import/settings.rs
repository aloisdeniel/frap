// SPDX-License-Identifier: GPL-3.0-or-later
//! Importer settings: the 20 `import_scene.gltf` properties as plain values (defaults are
//! Blender's, read from the generated spec), mirroring the add-on's `import_settings` dict.

use frap_spec_ops_io::import_scene::ImportSceneGltf;
pub use frap_spec_ops_io::import_scene::gltf::{
    BoneHeuristic, ExportImportConvertLightingMode, ImportShading,
};

/// `import_scene.gltf` settings (every property but `filepath`/`directory`/`files`/
/// `filter_glob`, which name the input and are handled by the caller).
#[derive(Clone, Debug, PartialEq)]
pub struct ImportSettings {
    /// `export_import_convert_lighting_mode` (`'SPEC'`)
    pub export_import_convert_lighting_mode: ExportImportConvertLightingMode,
    /// `loglevel` (0)
    pub loglevel: i32,
    /// `import_pack_images` (True)
    pub import_pack_images: bool,
    /// `merge_vertices` (False)
    pub merge_vertices: bool,
    /// `import_shading` (`'NORMALS'`)
    pub import_shading: ImportShading,
    /// `bone_heuristic` (`'BLENDER'`; skins arrive in M6)
    pub bone_heuristic: BoneHeuristic,
    /// `disable_bone_shape` (False)
    pub disable_bone_shape: bool,
    /// `bone_shape_scale_factor` (1.0)
    pub bone_shape_scale_factor: f32,
    /// `guess_original_bind_pose` (True)
    pub guess_original_bind_pose: bool,
    /// `import_webp_texture` (False)
    pub import_webp_texture: bool,
    /// `import_unused_materials` (False)
    pub import_unused_materials: bool,
    /// `import_select_created_objects` (True)
    pub import_select_created_objects: bool,
    /// `import_scene_extras` (True)
    pub import_scene_extras: bool,
    /// `import_scene_as_collection` (True)
    pub import_scene_as_collection: bool,
    /// `import_merge_material_slots` (True)
    pub import_merge_material_slots: bool,
    /// `import_point_as_pointcloud` (False)
    pub import_point_as_pointcloud: bool,
}

impl Default for ImportSettings {
    /// Blender's defaults (verified against the dumped operator spec in the tests).
    fn default() -> Self {
        ImportSettings {
            export_import_convert_lighting_mode: ExportImportConvertLightingMode::Spec,
            loglevel: 0,
            import_pack_images: true,
            merge_vertices: false,
            import_shading: ImportShading::Normals,
            bone_heuristic: BoneHeuristic::Blender,
            disable_bone_shape: false,
            bone_shape_scale_factor: 1.0,
            guess_original_bind_pose: true,
            import_webp_texture: false,
            import_unused_materials: false,
            import_select_created_objects: true,
            import_scene_extras: true,
            import_scene_as_collection: true,
            import_merge_material_slots: true,
            import_point_as_pointcloud: false,
        }
    }
}

impl From<&ImportSceneGltf> for ImportSettings {
    fn from(p: &ImportSceneGltf) -> Self {
        ImportSettings {
            export_import_convert_lighting_mode: p.export_import_convert_lighting_mode.value,
            loglevel: p.loglevel.value,
            import_pack_images: p.import_pack_images.value,
            merge_vertices: p.merge_vertices.value,
            import_shading: p.import_shading.value,
            bone_heuristic: p.bone_heuristic.value,
            disable_bone_shape: p.disable_bone_shape.value,
            bone_shape_scale_factor: p.bone_shape_scale_factor.value,
            guess_original_bind_pose: p.guess_original_bind_pose.value,
            import_webp_texture: p.import_webp_texture.value,
            import_unused_materials: p.import_unused_materials.value,
            import_select_created_objects: p.import_select_created_objects.value,
            import_scene_extras: p.import_scene_extras.value,
            import_scene_as_collection: p.import_scene_as_collection.value,
            import_merge_material_slots: p.import_merge_material_slots.value,
            import_point_as_pointcloud: p.import_point_as_pointcloud.value,
        }
    }
}

impl ImportSettings {
    /// Applies one setting by its RNA identifier and identifier-valued enum (`"FLAT"`),
    /// as the dumps and tests spell them. Returns false for an unknown name or value.
    pub fn set(&mut self, name: &str, value: &serde_json::Value) -> bool {
        let b = value.as_bool();
        let s = value.as_str();
        match (name, b, s) {
            ("import_pack_images", Some(v), _) => self.import_pack_images = v,
            ("merge_vertices", Some(v), _) => self.merge_vertices = v,
            ("disable_bone_shape", Some(v), _) => self.disable_bone_shape = v,
            ("guess_original_bind_pose", Some(v), _) => self.guess_original_bind_pose = v,
            ("import_webp_texture", Some(v), _) => self.import_webp_texture = v,
            ("import_unused_materials", Some(v), _) => self.import_unused_materials = v,
            ("import_select_created_objects", Some(v), _) => self.import_select_created_objects = v,
            ("import_scene_extras", Some(v), _) => self.import_scene_extras = v,
            ("import_scene_as_collection", Some(v), _) => self.import_scene_as_collection = v,
            ("import_merge_material_slots", Some(v), _) => self.import_merge_material_slots = v,
            ("import_point_as_pointcloud", Some(v), _) => self.import_point_as_pointcloud = v,
            ("import_shading", _, Some(v)) => match ImportShading::from_identifier(v) {
                Some(v) => self.import_shading = v,
                None => return false,
            },
            ("bone_heuristic", _, Some(v)) => match BoneHeuristic::from_identifier(v) {
                Some(v) => self.bone_heuristic = v,
                None => return false,
            },
            ("export_import_convert_lighting_mode", _, Some(v)) => {
                match ExportImportConvertLightingMode::from_identifier(v) {
                    Some(v) => self.export_import_convert_lighting_mode = v,
                    None => return false,
                }
            }
            ("loglevel", _, _) => match value.as_i64() {
                Some(v) => self.loglevel = v as i32,
                None => return false,
            },
            ("bone_shape_scale_factor", _, _) => match value.as_f64() {
                Some(v) => self.bone_shape_scale_factor = v as f32,
                None => return false,
            },
            _ => return false,
        }
        true
    }
}
