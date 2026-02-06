"""Render cat preview without denoiser"""
import bpy
import os

# Open the saved cat file
blend_path = os.path.expanduser("~/Documents/opus_4.5_cat1.blend")
bpy.ops.wm.open_mainfile(filepath=blend_path)

# Disable denoiser
bpy.context.scene.cycles.use_denoising = False

# Render preview
preview_path = os.path.expanduser("~/Documents/opus_4.5_cat1_preview.png")
bpy.context.scene.render.filepath = preview_path
bpy.context.scene.cycles.samples = 64
bpy.ops.render.render(write_still=True)

print(f"Preview rendered: {preview_path}")
