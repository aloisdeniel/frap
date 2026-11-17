# WP 1.23: read_homefile(use_empty=True) gives the empty scene; read_factory_settings and
# read_homefile bring the factory scene back, unsaved.
# Paths are in the work directory: Blender's save does not create directories.
# golden: startup=factory tier=T1 compare=objects,materials,context
import bpy
bpy.ops.mesh.primitive_cone_add()
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.mesh.primitive_plane_add()
bpy.ops.wm.read_factory_settings()
bpy.data.objects['Cube'].location = (3, 0, 0)
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.wm.read_homefile()
