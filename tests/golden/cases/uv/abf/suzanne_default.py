# WP 8.1: ABF++ on the default Suzanne cut along the seams of her default UV map. The islands
# match Blender's within 4.1e-7 after aligning each one, but the face island is wider than tall
# and ends axis-aligned after p_chart_rotate_minimum_area: the packer's pre-rotation then turns
# it by +90 or -90 degrees on the sign of a residual angle of about 1e-7 (a knife edge of
# angle_wrap at pi/2), and frap's ABF++ (sub-ulp differences from Blender's: faer's sparse LU
# instead of Eigen's, ~1e-7 in the UVs) lands on the other side: that island is turned by 180
# degrees. Checked at T3 (deviations §2, WP 8.1); suzanne_seams.py is the T2 case.
# golden: startup=empty compare=objects,context,mesh:Suzanne tier=T3 check=uv01,uv_no_overlap
import bpy
bpy.ops.mesh.primitive_monkey_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.seams_from_islands()
bpy.ops.uv.unwrap(method='ANGLE_BASED')
bpy.ops.object.mode_set(mode='OBJECT')
