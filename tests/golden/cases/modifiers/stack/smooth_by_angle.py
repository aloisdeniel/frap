# WP 4.1 (T1): the "Smooth by Angle" NODES modifier equals Blender's on many shapes: manifold
# and boundary edges, n-gons, existing sharp edges (kept unless Ignore Sharpness), flat faces
# (their edges stay sharp unless Ignore Sharpness), angle 0 and pi, and a modifier hidden in
# the viewport (not evaluated). Angles stay away from the edge angles of the shapes (an edge
# angle equal to the limit depends on float rounding, e.g. 30 degrees on a 12x6 UV sphere). The UV sphere is in smooth_by_angle_sphere.py
# (canonical order, D-19).
# golden: startup=factory compare=evaluated:Icosphere,evaluated:Suzanne,evaluated:Cone,evaluated:Torus,evaluated:Grid,evaluated:Cube,evaluated:Circle,evaluated:Cylinder tier=T1
import bpy
bpy.data.objects['Cube'].select_set(False)
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=2)
bpy.ops.object.shade_auto_smooth(angle=0.2)
bpy.ops.mesh.primitive_monkey_add(location=(6, 0, 0))
bpy.ops.object.shade_auto_smooth(angle=0.9)
bpy.ops.mesh.primitive_cone_add(vertices=7, location=(9, 0, 0))
bpy.ops.object.shade_auto_smooth(angle=0.7)
bpy.ops.mesh.primitive_torus_add(location=(12, 0, 0))
bpy.ops.object.shade_auto_smooth(angle=0.0)
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=3, location=(15, 0, 0))
bpy.ops.transform.translate(value=(0, 0, 0))
bpy.ops.object.shade_auto_smooth(angle=3.14159)
bpy.ops.mesh.primitive_circle_add(vertices=8, fill_type='NGON', location=(18, 0, 0))
bpy.ops.object.shade_auto_smooth()
bpy.data.meshes['Cube'].edges[0].use_edge_sharp = True
bpy.data.meshes['Cube'].edges[5].use_edge_sharp = True
bpy.data.objects['Cube'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.shade_auto_smooth(angle=1.7)
bpy.data.meshes['Cube'].polygons[2].use_smooth = False
bpy.ops.mesh.primitive_cylinder_add(vertices=10, location=(21, 0, 0))
bpy.ops.object.shade_auto_smooth(angle=1.0)
bpy.data.meshes['Cylinder'].polygons[0].use_smooth = False
bpy.data.meshes['Cylinder'].edges[3].use_edge_sharp = True
bpy.data.objects['Cylinder'].modifiers['Smooth by Angle'].properties.inputs.Socket_1.value = True
bpy.data.objects['Grid'].modifiers['Smooth by Angle'].show_viewport = False
