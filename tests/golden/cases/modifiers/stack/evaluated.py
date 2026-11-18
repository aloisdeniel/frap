# WP 4.1 (T1): evaluated_get(depsgraph).to_mesh() and Object.to_mesh() read the evaluated
# mesh (modifiers applied, always fresh) while the data keeps the original; show_viewport off
# skips a modifier; show_in_editmode decides in Edit Mode; a linked duplicate without
# modifiers evaluates to the shared data.
# golden: startup=factory compare=objects,mesh:Cube,evaluated:Cube,evaluated:Cube2,evaluated:Cylinder,evaluated:Cone tier=T1
import bpy
bpy.ops.object.shade_auto_smooth(angle=0.4)
bpy.data.objects.new('Cube2', bpy.data.meshes['Cube'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Cube2'])
bpy.data.objects['Cube'].to_mesh().attributes['sharp_edge'].data[0].value
bpy.data.objects['Cube'].evaluated_get(bpy.context.evaluated_depsgraph_get()).to_mesh().attributes['sharp_edge'].data[0].value
bpy.data.meshes['Cube'].attributes['sharp_edge']
bpy.data.objects['Cube'].to_mesh().name
bpy.data.objects['Cube'].to_mesh_clear()
bpy.ops.mesh.primitive_cone_add(vertices=6, location=(6, 0, 0))
bpy.ops.object.shade_auto_smooth(angle=1.2)
bpy.data.objects['Cone'].modifiers['Smooth by Angle'].show_viewport = False
bpy.ops.mesh.primitive_cylinder_add(vertices=8, location=(3, 0, 0))
bpy.ops.object.shade_auto_smooth(angle=1.0)
bpy.data.objects['Cylinder'].modifiers['Smooth by Angle'].show_in_editmode = False
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.transform.resize(value=(1, 1, 2))
