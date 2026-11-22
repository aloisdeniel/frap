# WP 8.4: undo across the Edit Mode boundary (headless-extended, D-145; the Blender side pushes
# the steps frap pushes with #@blender ed.undo_push lines). A material made in Edit Mode is not
# the edit data (no step), and it makes the next Edit Mesh step push a skipped "MemFile
# Internal (pre)" step first (`WITH_GLOBAL_UNDO_ENSURE_UPDATED`, a datablock was added since
# the last memfile step). Undoing that Edit Mesh step steps over the skipped one and, before
# the "Edit Mode" step, loads the memfile step it was made on, so the material goes
# (`WITH_GLOBAL_UNDO_CORRECT_ORDER`); undoing "Edit Mode" leaves Edit Mode; redoing both comes
# back through the skipped step, material included.
# golden: startup=factory compare=objects,context,materials,mesh:Cube.001 tier=T1 tol=1e-5
import bpy
#@blender bpy.ops.ed.undo_push(message="Original")
bpy.ops.mesh.primitive_cube_add(size=1, location=(3, 0, 0))
#@blender bpy.ops.ed.undo_push(message="Add Cube")
bpy.ops.object.mode_set(mode='EDIT')
#@blender bpy.ops.ed.undo_push(message="Edit Mode")
bpy.data.materials.new("Undone")
bpy.ops.mesh.select_all(action='DESELECT')
#@blender bpy.ops.ed.undo_push(message="(De)select All")
bpy.ops.ed.undo()
bpy.ops.ed.undo()
bpy.ops.ed.undo()
bpy.ops.ed.redo()
bpy.ops.ed.redo()
bpy.ops.ed.redo()
bpy.data.materials['Undone']
bpy.ops.ed.undo()
bpy.data.materials['Undone']
bpy.ops.ed.undo()
bpy.ops.ed.redo()
