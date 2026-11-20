# WP 6.5: armature deform of a mesh in Edit Mode. The ARMATURE modifier runs on the edit mesh
# only with show_in_editmode (off by default, as parent_set leaves it), and the armature-parent
# virtual modifier never runs in Edit Mode (its eModifierMode_Editmode flag is off). The linked
# duplicate shares the edit mesh, so it is evaluated as in Edit Mode as well: its own copy of
# the modifier (show_in_editmode off) and its virtual modifier leave it undeformed.
# golden: startup=factory tier=T1 tol=1e-5 compare=evaluated:Cube,evaluated:Cube.001
import bpy
bpy.ops.object.armature_add(location=(0, 0, -1))
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.armatures['Armature'].edit_bones['Bone'].tail = (0, 0, 2.0)
bpy.data.armatures['Armature'].edit_bones['Bone'].head_radius = 1.0
bpy.data.armatures['Armature'].edit_bones['Bone'].tail_radius = 1.0
bpy.data.armatures['Armature'].edit_bones['Bone'].envelope_distance = 1.5
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion = (0.9, 0.2, 0.3, 0.0)
bpy.data.objects['Cube'].vertex_groups.new(name='Bone')
bpy.data.objects['Cube'].vertex_groups['Bone'].add([0, 1, 2, 3], 0.75, 'REPLACE')
bpy.data.objects['Cube'].modifiers.new('Rig', 'ARMATURE')
bpy.data.objects['Cube'].modifiers['Rig'].object = bpy.data.objects['Armature']
bpy.data.objects['Cube'].modifiers['Rig'].show_in_editmode
bpy.data.objects['Cube'].parent = bpy.data.objects['Armature']
bpy.data.objects['Cube'].parent_type = 'ARMATURE'
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.duplicate(linked=True)
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.objects['Cube'].modifiers['Rig'].show_in_editmode = True
