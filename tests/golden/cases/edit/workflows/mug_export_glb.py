# WP 2.13 workflow (mug, end to end): body from a cylinder (inset + extrude into a cavity),
# shaded smooth by angle in Object Mode, then exported as GLB; the exported file is compared.
# golden: startup=empty compare=objects,glb:out/mug.glb tier=T2
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=24, radius=0.5, depth=1, location=(0, 0, 0.5))
bpy.context.object.name = 'Mug'
bpy.context.object.data.name = 'Mug'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_by_normal(direction=(0, 0, 1))
bpy.ops.mesh.inset(thickness=0.05)
bpy.ops.mesh.extrude_region_move(TRANSFORM_OT_translate={"value": (0, 0, -0.9)})
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.normals_make_consistent(inside=False)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.shade_smooth_by_angle()
bpy.ops.export_scene.gltf(filepath='out/mug.glb')
