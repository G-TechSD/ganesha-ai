"""Fix camera angle for cat and re-render"""
import bpy
import os
import math

# Open the saved cat file
blend_path = os.path.expanduser("~/Documents/opus_4.5_cat1.blend")
bpy.ops.wm.open_mainfile(filepath=blend_path)

# Find or create camera
cam = None
for obj in bpy.data.objects:
    if obj.type == 'CAMERA':
        cam = obj
        break

if not cam:
    bpy.ops.object.camera_add(location=(3, -3, 2))
    cam = bpy.context.active_object

# Position camera to see full cat (side view, slightly elevated)
cam.location = (2.5, -2.5, 1.5)
cam.rotation_euler = (math.radians(65), 0, math.radians(45))
bpy.context.scene.camera = cam

# Add ground plane
bpy.ops.mesh.primitive_plane_add(size=10, location=(0, 0, 0))
ground = bpy.context.active_object
ground.name = "Ground"

# Ground material
ground_mat = bpy.data.materials.new(name="Ground_Mat")
ground_mat.use_nodes = True
ground_mat.node_tree.nodes["Principled BSDF"].inputs['Base Color'].default_value = (0.3, 0.35, 0.3, 1)
ground_mat.node_tree.nodes["Principled BSDF"].inputs['Roughness'].default_value = 0.9
ground.data.materials.append(ground_mat)

# Disable denoiser
bpy.context.scene.cycles.use_denoising = False

# Save updated blend
bpy.ops.wm.save_as_mainfile(filepath=blend_path)

# Render preview
preview_path = os.path.expanduser("~/Documents/opus_4.5_cat1_preview.png")
bpy.context.scene.render.filepath = preview_path
bpy.context.scene.cycles.samples = 128
bpy.ops.render.render(write_still=True)

print(f"Fixed cat saved and rendered: {preview_path}")
