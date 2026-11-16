import bpy, math, sys, os
C=bpy.context; D=bpy.data; OUT=sys.argv[-1]
def v3d():
    w=C.window_manager.windows[0]
    a=[a for a in w.screen.areas if a.type=='VIEW_3D'][0]
    return dict(window=w,area=a,region=[r for r in a.regions if r.type=='WINDOW'][0])
def r(v): return tuple(round(x,4) for x in v)
def st(tag=''): print(f"  [{tag}] mode={C.mode} active={C.active_object.name if C.active_object else None} selected={[o.name for o in C.selected_objects]}")
def tryop(label, f):
    try: print(f"  {label}: {f()}")
    except Exception as e: print(f"  {label}: EXC {type(e).__name__}: {str(e).strip()[:300]}")
def fresh(empty=False):
    bpy.ops.wm.read_factory_settings(use_empty=empty)
print("== modifier apply order")
fresh(); cube=D.objects['Cube']
bpy.ops.object.modifier_add(type='BEVEL'); bpy.ops.object.modifier_add(type='SUBSURF')
tryop("apply non-first SUBSURF", lambda: bpy.ops.object.modifier_apply(modifier='Subdivision'))
print("  remaining", [m.name for m in cube.modifiers], len(cube.data.vertices))
tryop("apply first Bevel", lambda: bpy.ops.object.modifier_apply(modifier='Bevel'))
bpy.ops.object.modifier_add(type='MIRROR'); bpy.ops.object.modifier_add(type='HOOK'); print("  hook insert order", [m.name for m in cube.modifiers])
tryop("move Mirror to 0", lambda: bpy.ops.object.modifier_move_to_index(modifier='Mirror', index=0))
tryop("modifier_remove", lambda: bpy.ops.object.modifier_remove(modifier='Hook'))
tryop("modifier_remove missing", lambda: bpy.ops.object.modifier_remove(modifier='Nope'))
bpy.ops.object.mode_set(mode='EDIT'); tryop("modifier_apply in edit", lambda: bpy.ops.object.modifier_apply(modifier='Mirror')); bpy.ops.object.mode_set(mode='OBJECT')
print("== errors")
fresh()
cam=D.objects['Camera']; C.view_layer.objects.active=cam
tryop("mode_set EDIT on camera", lambda: bpy.ops.object.mode_set(mode='EDIT'))
tryop("mode_set POSE on camera", lambda: bpy.ops.object.mode_set(mode='POSE'))
C.view_layer.objects.active=D.objects['Cube']
tryop("enum bad", lambda: bpy.ops.mesh.primitive_cube_add(align='FOO'))
tryop("bad kw", lambda: bpy.ops.mesh.primitive_cube_add(sizee=2))
tryop("bad type", lambda: bpy.ops.mesh.primitive_cube_add(size='a'))
tryop("size clamp", lambda: (bpy.ops.mesh.primitive_cube_add(size=-5), C.object.dimensions[:]))
tryop("poll fail", lambda: bpy.ops.object.vertex_group_remove())
bpy.ops.object.select_all(action='DESELECT'); C.view_layer.objects.active=cam; cam.select_set(True)
tryop("join no mesh", lambda: bpy.ops.object.join())
fresh(True)
tryop("render no camera", lambda: bpy.ops.render.render(write_still=True))
print("== camera_add with v3d in empty scene")
with C.temp_override(**v3d()):
    bpy.ops.object.camera_add(); print("  scene.camera", C.scene.camera)
    bpy.ops.object.light_add(type='SUN'); print("  sun rot", r(C.object.rotation_euler))
print("== select_all")
fresh(); bpy.ops.object.select_all(action='TOGGLE'); st('toggle from cube selected')
bpy.ops.object.select_all(action='TOGGLE'); st('toggle again')
bpy.ops.object.select_all(action='INVERT'); st('invert')
bpy.ops.object.select_by_type(type='MESH'); st('by_type MESH extend=False')
bpy.ops.object.select_pattern(pattern='C*'); st('pattern C*')
print("== delete")
fresh(); cube=D.objects['Cube']
bpy.ops.mesh.primitive_cone_add(); cone=C.object; cone.parent=cube
bpy.ops.object.select_all(action='DESELECT'); cube.select_set(True); C.view_layer.objects.active=cube
bpy.ops.object.delete(); st('after delete'); print("  meshes", [(m.name,m.users) for m in D.meshes], "cone parent", cone.parent, 'cone world', r(cone.matrix_world.translation))
print("== transforms")
fresh(); cube=D.objects['Cube']
bpy.ops.transform.rotate(value=0.5, orient_axis='Z'); print("  rotate value=0.5 Z ->", r(cube.rotation_euler))
bpy.ops.transform.translate(value=(1,0,0)); print("  translate", r(cube.location))
bpy.ops.transform.resize(value=(2,1,1)); print("  resize", r(cube.scale))
bpy.ops.transform.translate(value=(1,0,0), orient_type='LOCAL'); print("  translate LOCAL", r(cube.location))
bpy.ops.mesh.primitive_cube_add(location=(4,0,0)); D.objects['Cube'].select_set(True)
c2=C.object; print('  pivot', C.scene.tool_settings.transform_pivot_point)
bpy.ops.transform.resize(value=(2,2,2)); print("  multi resize locs", r(D.objects['Cube'].location), r(c2.location))
bpy.ops.transform.mirror(constraint_axis=(True,False,False)); print("  mirror X", r(c2.location), r(c2.scale))
bpy.ops.object.select_all(action='DESELECT'); c2.select_set(True); C.view_layer.objects.active=c2
bpy.ops.object.mode_set(mode='EDIT'); import bmesh
bpy.ops.mesh.select_all(action='DESELECT'); bm=bmesh.from_edit_mesh(c2.data); bm.verts.ensure_lookup_table(); bm.verts[0].select=True; bm.select_flush_mode(); bmesh.update_edit_mesh(c2.data)
co0=bm.verts[0].co.copy(); bpy.ops.transform.translate(value=(0,0,1)); bm=bmesh.from_edit_mesh(c2.data); bm.verts.ensure_lookup_table(); print("  edit translate vert0 local", r(co0),'->', r(bm.verts[0].co), "(obj scale", r(c2.scale),")")
bpy.ops.object.mode_set(mode='OBJECT')
tryop("transform.transform mode=TRANSLATION", lambda: (bpy.ops.transform.transform(mode='TRANSLATION', value=(1,0,0,0)), r(c2.location)))
print("== snap")
fresh(); cube=D.objects['Cube']; cube.location=(1,2,3)
with C.temp_override(**v3d()):
    bpy.ops.view3d.snap_cursor_to_selected(); print("  cursor->selected", r(C.scene.cursor.location))
    bpy.ops.view3d.snap_cursor_to_center(); print("  cursor->center", r(C.scene.cursor.location))
    C.scene.cursor.location=(0.4,0.6,0); bpy.ops.view3d.snap_cursor_to_grid(); print("  cursor->grid", r(C.scene.cursor.location))
    bpy.ops.view3d.snap_selected_to_cursor(); print("  selected->cursor", r(cube.location))
print("== convert / shape keys / vgroups / materials")
fresh(); cube=D.objects['Cube']
bpy.ops.object.modifier_add(type='SUBSURF'); bpy.ops.object.convert(target='MESH'); print("  convert applies mods", len(cube.data.vertices), [m.name for m in cube.modifiers])
bpy.ops.object.shape_key_add(from_mix=False); bpy.ops.object.shape_key_add(from_mix=False); print("  shape keys", [k.name for k in cube.data.shape_keys.key_blocks], cube.active_shape_key_index, cube.data.shape_keys.name, cube.data.shape_keys.use_relative)
bpy.ops.object.vertex_group_add(); bpy.ops.object.vertex_group_add(); print("  vgroups", [g.name for g in cube.vertex_groups], cube.vertex_groups.active_index)
print("  mats before", [s.material.name if s.material else None for s in cube.material_slots])
bpy.ops.object.material_slot_add(); print("  after slot_add", [s.material.name if s.material else None for s in cube.material_slots], cube.active_material_index)
tryop("material.new", lambda: bpy.ops.material.new()); print("  mats", [m.name for m in D.materials], [s.material.name if s.material else None for s in cube.material_slots], 'use_nodes', D.materials[-1].use_nodes, [n.name for n in D.materials[-1].node_tree.nodes])
print("== image.new")
tryop("image.new", lambda: bpy.ops.image.new()); im=D.images[-1]; print("  ", im.name, tuple(im.size), im.generated_type, tuple(im.generated_color), im.use_generated_float, im.alpha_mode, im.colorspace_settings.name, im.source)
print("== render")
fresh(); C.scene.render.engine='BLENDER_WORKBENCH'; C.scene.render.resolution_x=64; C.scene.render.resolution_y=48
C.scene.render.filepath=os.path.join(OUT,'shot')
tryop("render still", lambda: bpy.ops.render.render(write_still=True)); print("  files", sorted(os.listdir(OUT)))
C.scene.frame_end=2; C.scene.render.filepath=os.path.join(OUT,'anim_')
tryop("render anim", lambda: bpy.ops.render.render(animation=True)); print("  files", sorted(os.listdir(OUT)))
C.scene.render.filepath=os.path.join(OUT,'hash_###_x')
tryop("render anim hashes", lambda: bpy.ops.render.render(animation=True, frame_start=1, frame_end=1) if 'frame_start' in bpy.ops.render.render.get_rna_type().properties else bpy.ops.render.render(animation=True)); print("  files", sorted(os.listdir(OUT)))
print("  image 'Render Result'", 'Render Result' in D.images)
print("== export gltf")
tryop("export glb default", lambda: bpy.ops.export_scene.gltf(filepath=os.path.join(OUT,'a.glb')))
tryop("export gltf sep", lambda: bpy.ops.export_scene.gltf(filepath=os.path.join(OUT,'b'), export_format='GLTF_SEPARATE'))
print("  files", sorted(os.listdir(OUT)))
tryop("import", lambda: bpy.ops.import_scene.gltf(filepath=os.path.join(OUT,'a.glb'))); st('after import'); print("  objs", [o.name for o in D.objects])
print("== save/open")
tryop("save_as", lambda: bpy.ops.wm.save_as_mainfile(filepath=os.path.join(OUT,'x.blend')))
print("  filepath", D.filepath, D.is_dirty)
tryop("read_homefile use_empty", lambda: bpy.ops.wm.read_homefile(use_empty=True)); print("  objs", len(D.objects), D.filepath)
tryop("open", lambda: bpy.ops.wm.open_mainfile(filepath=os.path.join(OUT,'x.blend'))); print("  objs", len(D.objects), D.filepath)
print("== undo in bg")
tryop("ed.undo", lambda: bpy.ops.ed.undo())
print("== context members")
fresh()
def ctxdump(tag):
    keys=[k for k in dir(C) if not k.startswith('_') and k not in ('bl_rna','rna_type','copy','path_resolve','temp_override','evaluated_depsgraph_get','property','id')]
    vals={}
    for k in keys:
        try:
            v=getattr(C,k)
            if callable(v): continue
            if isinstance(v,list): v=f"list[{len(v)}]"+(f" e.g. {type(v[0]).__name__}" if v else '')
            else: v=type(v).__name__ if v is not None and not isinstance(v,(str,int,float,bool)) else v
            vals[k]=v
        except Exception as e: vals[k]='EXC'
    print(f"  [{tag}]", vals)
ctxdump('bg object mode')
with C.temp_override(**v3d()): ctxdump('v3d object mode')
bpy.ops.object.mode_set(mode='EDIT')
with C.temp_override(**v3d()): ctxdump('v3d edit mode')
