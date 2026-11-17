# WP 1.23: save_as_mainfile then open_mainfile restores data, selection, the active object and
# the cursor; later edits are discarded.
# Paths are in the work directory: Blender's save does not create directories.
# golden: startup=factory tier=T1 compare=objects,materials,mesh:Cube,context tol=1e-6
import bpy
bpy.ops.mesh.primitive_cone_add(vertices=7, location=(2, 0, 0))
bpy.data.objects['Cube'].location = (0.1, 0.2, 0.3)
bpy.data.objects['Cube'].parent = bpy.data.objects['Cone']
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value = 0.5
bpy.context.scene.cursor.location = (1, 2, 3)
bpy.context.scene.frame_current = 7
bpy.data.objects['Light'].select_set(True)
bpy.ops.wm.save_as_mainfile(filepath='a.frap')
bpy.data.objects['Cube'].location = (9, 9, 9)
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects.remove(bpy.data.objects['Camera'])
bpy.ops.wm.open_mainfile(filepath='a.frap')
