# WP 2.11: bevel faces take material 1 (clamped to the slots), seams and sharp edges
# are extended (mark_seam, mark_sharp), face strength is set on all faces.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.data.objects['Cube'].data.materials.append(bpy.data.materials.new('Second'))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.bevel(offset=0.2, segments=2, material=5, mark_seam=True, mark_sharp=True, face_strength_mode='ALL')
