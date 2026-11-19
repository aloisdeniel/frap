# WP 3.10: `scene.new(type='FULL_COPY')` while the Cube is in Edit Mode: the edits are written
# to the mesh first (`ED_editors_flush_edits`), so the copied mesh has them; the original stays
# in Edit Mode in its scene, the copy is in Object Mode and the new scene's context mode is
# OBJECT.
# golden: startup=factory tier=T1 compare=objects,mesh:Cube.001,context tol=1e-5
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.transform.translate(value=(0, 0, 1))
bpy.ops.mesh.subdivide()
bpy.ops.scene.new(type='FULL_COPY')
bpy.context.mode
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.context.mode, None))
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.data.objects['Cube'].mode, None))
bpy.context.scene.collection.objects.link(bpy.data.objects.new(bpy.data.objects['Cube.001'].mode, None))
bpy.context.window.scene = bpy.data.scenes['Scene']
bpy.context.mode
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.window.scene = bpy.data.scenes['Scene.001']
