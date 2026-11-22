# WP 8.4: undo and redo in mesh Edit Mode (headless-extended, D-149; the Blender side pushes
# the steps frap pushes with #@blender ed.undo_push lines). object.mode_set pushes through
# the toggle operator it calls ("Edit Mode", an Edit Mesh step); every edit-mode operator
# pushes an Edit Mesh step that restores the edit BMesh only: the object location written in
# Edit Mode (an Object, not the edit data: no step) survives the edit-mode undos. Undoing the
# "Edit Mode" step goes back to the Object Mode memfile step; redo enters Edit Mode again with
# the BMesh of each step (positions, topology, selection, select mode). A step's BMesh comes
# back through a Mesh (`undomesh_to_editmesh`): the face deleted before the step leaves no
# free slot, so the subdivision after the undo appends its elements as in Blender.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1 tol=1e-5
import bpy
#@blender bpy.ops.ed.undo_push(message="Original")
bpy.ops.object.mode_set(mode='EDIT')
#@blender bpy.ops.ed.undo_push(message="Edit Mode")
bpy.ops.mesh.select_all(action='DESELECT')
#@blender bpy.ops.ed.undo_push(message="(De)select All")
bpy.ops.frap.select_elements(type='FACE', indices=[2])
#@blender bpy.ops.ed.undo_push(message="Select Elements")
bpy.ops.transform.translate(value=(0, 0, 1))
#@blender bpy.ops.ed.undo_push(message="Move")
bpy.data.objects['Cube'].location.z = 2.0
bpy.ops.mesh.select_mode(type='EDGE')
#@blender bpy.ops.ed.undo_push(message="Select Mode")
bpy.ops.mesh.subdivide(number_cuts=2)
#@blender bpy.ops.ed.undo_push(message="Subdivide")
bpy.ops.ed.undo()
bpy.ops.ed.undo()
bpy.ops.ed.undo()
bpy.ops.ed.redo()
bpy.ops.ed.redo()
bpy.ops.ed.undo()
bpy.ops.ed.undo()
bpy.ops.mesh.delete(type='FACE')
#@blender bpy.ops.ed.undo_push(message="Delete")
bpy.ops.mesh.select_all(action='SELECT')
#@blender bpy.ops.ed.undo_push(message="(De)select All")
bpy.ops.ed.redo()
bpy.ops.ed.undo()
bpy.ops.mesh.select_all(action='SELECT')
#@blender bpy.ops.ed.undo_push(message="(De)select All")
bpy.ops.mesh.subdivide()
#@blender bpy.ops.ed.undo_push(message="Subdivide")
bpy.ops.ed.undo()
bpy.ops.ed.redo()
