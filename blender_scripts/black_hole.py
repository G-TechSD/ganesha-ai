"""
Black Hole - Journey Through the Event Horizon
Created by Ganesha VLA + Agent K
Blender 4.0+ compatible

Physically inspired visualization:
- Accretion disk with hot gas emission
- Event horizon (Schwarzschild black hole)
- Photon sphere glow
- Gravitational lensing approximation
- Relativistic jets
- Camera flies INTO the event horizon
"""
import bpy
import math
import random
from mathutils import Vector, Euler

# ==================== CLEAR SCENE ====================
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.delete(use_global=True)

for block in bpy.data.meshes:
    if block.users == 0:
        bpy.data.meshes.remove(block)
for block in bpy.data.materials:
    if block.users == 0:
        bpy.data.materials.remove(block)
for block in bpy.data.lights:
    if block.users == 0:
        bpy.data.lights.remove(block)
for block in bpy.data.particles:
    if block.users == 0:
        bpy.data.particles.remove(block)

# ==================== SCENE CONFIG ====================
scene = bpy.context.scene
scene.frame_start = 1
scene.frame_end = 720  # 24 seconds at 30fps
scene.frame_current = 1
scene.render.fps = 30
scene.render.resolution_x = 1920
scene.render.resolution_y = 1080
scene.render.engine = 'BLENDER_EEVEE'

# EEVEE settings for volumetrics and bloom
eevee = scene.eevee
eevee.use_bloom = True
eevee.bloom_threshold = 0.8
eevee.bloom_intensity = 0.5
eevee.bloom_radius = 6.5
eevee.bloom_knee = 0.5
eevee.use_volumetric_lights = True
eevee.volumetric_tile_size = '4'
eevee.volumetric_samples = 128
eevee.volumetric_end = 200.0

# ==================== WORLD - DEEP SPACE ====================
world = scene.world or bpy.data.worlds.new("World")
scene.world = world
world.use_nodes = True
wn = world.node_tree.nodes
wl = world.node_tree.links
wn.clear()

# Star field background
tex_coord = wn.new('ShaderNodeTexCoord')
star_noise = wn.new('ShaderNodeTexNoise')
star_noise.inputs['Scale'].default_value = 1200.0
star_noise.inputs['Detail'].default_value = 16.0
star_noise.inputs['Roughness'].default_value = 1.0

star_ramp = wn.new('ShaderNodeValToRGB')
star_ramp.color_ramp.elements[0].position = 0.76
star_ramp.color_ramp.elements[0].color = (0.0, 0.0, 0.0, 1.0)
star_ramp.color_ramp.elements[1].position = 0.78
star_ramp.color_ramp.elements[1].color = (1.0, 1.0, 0.95, 1.0)

# Distant nebula glow
nebula_noise = wn.new('ShaderNodeTexNoise')
nebula_noise.inputs['Scale'].default_value = 2.0
nebula_noise.inputs['Detail'].default_value = 8.0
nebula_noise.inputs['Distortion'].default_value = 3.0

nebula_ramp = wn.new('ShaderNodeValToRGB')
nebula_ramp.color_ramp.elements[0].position = 0.4
nebula_ramp.color_ramp.elements[0].color = (0.0, 0.0, 0.02, 1.0)
nebula_ramp.color_ramp.elements[1].position = 0.7
nebula_ramp.color_ramp.elements[1].color = (0.08, 0.02, 0.15, 1.0)

add_shader = wn.new('ShaderNodeAddShader')
bg_dark = wn.new('ShaderNodeBackground')
bg_dark.inputs['Strength'].default_value = 1.0

bg_stars = wn.new('ShaderNodeEmission')
bg_stars.inputs['Strength'].default_value = 5.0

bg_nebula = wn.new('ShaderNodeEmission')
bg_nebula.inputs['Strength'].default_value = 0.3

mix_bg = wn.new('ShaderNodeMixShader')
output_w = wn.new('ShaderNodeOutputWorld')

wl.new(tex_coord.outputs['Object'], star_noise.inputs['Vector'])
wl.new(tex_coord.outputs['Object'], nebula_noise.inputs['Vector'])
wl.new(star_noise.outputs['Fac'], star_ramp.inputs['Fac'])
wl.new(star_ramp.outputs['Color'], bg_stars.inputs['Color'])
wl.new(nebula_noise.outputs['Fac'], nebula_ramp.inputs['Fac'])
wl.new(nebula_ramp.outputs['Color'], bg_dark.inputs['Color'])
wl.new(nebula_ramp.outputs['Color'], bg_nebula.inputs['Color'])
wl.new(bg_dark.outputs['Background'], mix_bg.inputs[1])
wl.new(bg_stars.outputs['Emission'], mix_bg.inputs[2])
wl.new(star_ramp.outputs['Color'], mix_bg.inputs['Fac'])
wl.new(mix_bg.outputs['Shader'], add_shader.inputs[0])
wl.new(bg_nebula.outputs['Emission'], add_shader.inputs[1])
wl.new(add_shader.outputs['Shader'], output_w.inputs['Surface'])

# ==================== EVENT HORIZON ====================
# The black hole itself - perfect absorber

bpy.ops.mesh.primitive_uv_sphere_add(
    radius=2.0, segments=64, ring_count=32, location=(0, 0, 0)
)
event_horizon = bpy.context.active_object
event_horizon.name = "EventHorizon"
bpy.ops.object.shade_smooth()

# Holdout/pure black material
eh_mat = bpy.data.materials.new("EventHorizon_mat")
eh_mat.use_nodes = True
ehn = eh_mat.node_tree.nodes
ehl = eh_mat.node_tree.links
ehn.clear()
out = ehn.new('ShaderNodeOutputMaterial')
# Pure black emission (absorbs all light)
black = ehn.new('ShaderNodeEmission')
black.inputs['Color'].default_value = (0.0, 0.0, 0.0, 1.0)
black.inputs['Strength'].default_value = 0.0
ehl.new(black.outputs['Emission'], out.inputs['Surface'])
event_horizon.data.materials.append(eh_mat)

# ==================== PHOTON SPHERE ====================
# Glowing shell just outside event horizon (r = 1.5 * rs)

bpy.ops.mesh.primitive_uv_sphere_add(
    radius=2.8, segments=48, ring_count=24, location=(0, 0, 0)
)
photon_sphere = bpy.context.active_object
photon_sphere.name = "PhotonSphere"
bpy.ops.object.shade_smooth()

ps_mat = bpy.data.materials.new("PhotonSphere_mat")
ps_mat.use_nodes = True
ps_mat.blend_method = 'BLEND' if hasattr(ps_mat, 'blend_method') else None
psn = ps_mat.node_tree.nodes
psl = ps_mat.node_tree.links
psn.clear()

ps_out = psn.new('ShaderNodeOutputMaterial')
ps_emit = psn.new('ShaderNodeEmission')
ps_emit.inputs['Color'].default_value = (0.4, 0.15, 0.02, 1.0)
ps_emit.inputs['Strength'].default_value = 2.0

# Fresnel for edge glow effect
ps_fresnel = psn.new('ShaderNodeFresnel')
ps_fresnel.inputs['IOR'].default_value = 1.1

ps_mix = psn.new('ShaderNodeMixShader')
ps_transparent = psn.new('ShaderNodeBsdfTransparent')

psl.new(ps_fresnel.outputs['Fac'], ps_mix.inputs['Fac'])
psl.new(ps_transparent.outputs['BSDF'], ps_mix.inputs[1])
psl.new(ps_emit.outputs['Emission'], ps_mix.inputs[2])
psl.new(ps_mix.outputs['Shader'], ps_out.inputs['Surface'])
photon_sphere.data.materials.append(ps_mat)

# ==================== ACCRETION DISK ====================
# Hot gas spiraling into the black hole
# Multiple layers for depth and detail

def create_accretion_ring(name, inner_r, outer_r, thickness, height,
                          color_inner, color_outer, emission_str,
                          rotation_period, turbulence=2.0):
    """Create one ring of the accretion disk"""
    mid_r = (inner_r + outer_r) / 2
    tube_r = (outer_r - inner_r) / 2

    bpy.ops.mesh.primitive_torus_add(
        major_radius=mid_r,
        minor_radius=tube_r,
        major_segments=128,
        minor_segments=24,
        location=(0, 0, height)
    )
    ring = bpy.context.active_object
    ring.name = name
    ring.scale[2] = thickness
    bpy.ops.object.shade_smooth()

    # Accretion disk material - hot gas emission
    mat = bpy.data.materials.new(f"{name}_mat")
    mat.use_nodes = True
    if hasattr(mat, 'blend_method'):
        mat.blend_method = 'BLEND'
    nodes = mat.node_tree.nodes
    links = mat.node_tree.links
    nodes.clear()

    mat_out = nodes.new('ShaderNodeOutputMaterial')

    # Emission for hot gas
    emit = nodes.new('ShaderNodeEmission')
    emit.inputs['Strength'].default_value = emission_str

    # Transparent for volume feel
    transparent = nodes.new('ShaderNodeBsdfTransparent')

    # Mix based on noise (turbulent gas)
    mix = nodes.new('ShaderNodeMixShader')

    # Noise texture for turbulent gas structure
    tex_coord = nodes.new('ShaderNodeTexCoord')
    noise1 = nodes.new('ShaderNodeTexNoise')
    noise1.inputs['Scale'].default_value = turbulence
    noise1.inputs['Detail'].default_value = 12.0
    noise1.inputs['Roughness'].default_value = 0.7
    noise1.inputs['Distortion'].default_value = 1.5

    # Second noise for color variation
    noise2 = nodes.new('ShaderNodeTexNoise')
    noise2.inputs['Scale'].default_value = turbulence * 3
    noise2.inputs['Detail'].default_value = 8.0

    # Color ramp for hot gas colors (inner=blue-white, outer=red-orange)
    color_ramp = nodes.new('ShaderNodeValToRGB')
    color_ramp.color_ramp.elements[0].position = 0.0
    color_ramp.color_ramp.elements[0].color = (*color_inner, 1.0)
    # Add middle element
    mid_elem = color_ramp.color_ramp.elements.new(0.5)
    mid_elem.color = (1.0, 0.6, 0.1, 1.0)  # Orange
    color_ramp.color_ramp.elements[-1].position = 1.0
    color_ramp.color_ramp.elements[-1].color = (*color_outer, 1.0)

    # Opacity ramp
    opacity_ramp = nodes.new('ShaderNodeValToRGB')
    opacity_ramp.color_ramp.elements[0].position = 0.3
    opacity_ramp.color_ramp.elements[0].color = (0.0, 0.0, 0.0, 1.0)
    opacity_ramp.color_ramp.elements[1].position = 0.6
    opacity_ramp.color_ramp.elements[1].color = (1.0, 1.0, 1.0, 1.0)

    links.new(tex_coord.outputs['Object'], noise1.inputs['Vector'])
    links.new(tex_coord.outputs['Object'], noise2.inputs['Vector'])
    links.new(noise2.outputs['Fac'], color_ramp.inputs['Fac'])
    links.new(color_ramp.outputs['Color'], emit.inputs['Color'])
    links.new(noise1.outputs['Fac'], opacity_ramp.inputs['Fac'])
    links.new(opacity_ramp.outputs['Color'], mix.inputs['Fac'])
    links.new(transparent.outputs['BSDF'], mix.inputs[1])
    links.new(emit.outputs['Emission'], mix.inputs[2])
    links.new(mix.outputs['Shader'], mat_out.inputs['Surface'])

    ring.data.materials.append(mat)

    # Rotation animation (differential rotation - inner faster)
    ring.rotation_euler[2] = 0.0
    ring.keyframe_insert(data_path="rotation_euler", frame=1, index=2)
    ring.rotation_euler[2] = math.radians(360)
    ring.keyframe_insert(data_path="rotation_euler", frame=rotation_period, index=2)

    if ring.animation_data and ring.animation_data.action:
        for fc in ring.animation_data.action.fcurves:
            for kfp in fc.keyframe_points:
                kfp.interpolation = 'LINEAR'
            fc.modifiers.new(type='CYCLES')

    return ring


# Inner disk - hottest, fastest, blue-white
create_accretion_ring(
    "AccretionDisk_Inner", 3.0, 6.0, 0.06, 0.0,
    (0.7, 0.8, 1.0), (1.0, 0.5, 0.1), 15.0,
    rotation_period=120, turbulence=4.0
)

# Middle disk - hot, orange-yellow
create_accretion_ring(
    "AccretionDisk_Mid", 5.5, 10.0, 0.04, 0.1,
    (1.0, 0.7, 0.2), (0.9, 0.3, 0.05), 8.0,
    rotation_period=240, turbulence=3.0
)

# Outer disk - cooler, red-orange, slower
create_accretion_ring(
    "AccretionDisk_Outer", 9.0, 16.0, 0.03, -0.1,
    (1.0, 0.4, 0.05), (0.6, 0.1, 0.02), 4.0,
    rotation_period=480, turbulence=2.0
)

# Wispy outer ring
create_accretion_ring(
    "AccretionDisk_Wisp", 14.0, 22.0, 0.02, 0.2,
    (0.8, 0.2, 0.05), (0.3, 0.05, 0.02), 2.0,
    rotation_period=720, turbulence=1.5
)

# Thin bright inner edge (ISCO - innermost stable circular orbit)
create_accretion_ring(
    "ISCO_Ring", 2.8, 3.5, 0.08, 0.0,
    (0.8, 0.9, 1.0), (0.6, 0.7, 1.0), 25.0,
    rotation_period=80, turbulence=6.0
)

# ==================== GRAVITATIONAL LENSING RING ====================
# Einstein ring - light bent around the black hole
# Approximated as a bright torus at the photon sphere radius

bpy.ops.mesh.primitive_torus_add(
    major_radius=3.2,
    minor_radius=0.08,
    major_segments=128,
    minor_segments=16,
    location=(0, 0, 0)
)
einstein_ring = bpy.context.active_object
einstein_ring.name = "EinsteinRing"
bpy.ops.object.shade_smooth()

er_mat = bpy.data.materials.new("EinsteinRing_mat")
er_mat.use_nodes = True
if hasattr(er_mat, 'blend_method'):
    er_mat.blend_method = 'BLEND'
ern = er_mat.node_tree.nodes
erl = er_mat.node_tree.links
ern.clear()

er_out = ern.new('ShaderNodeOutputMaterial')
er_emit = ern.new('ShaderNodeEmission')
er_emit.inputs['Color'].default_value = (0.9, 0.95, 1.0, 1.0)
er_emit.inputs['Strength'].default_value = 30.0

er_fresnel = ern.new('ShaderNodeFresnel')
er_fresnel.inputs['IOR'].default_value = 1.5
er_trans = ern.new('ShaderNodeBsdfTransparent')
er_mix = ern.new('ShaderNodeMixShader')

erl.new(er_fresnel.outputs['Fac'], er_mix.inputs['Fac'])
erl.new(er_trans.outputs['BSDF'], er_mix.inputs[1])
erl.new(er_emit.outputs['Emission'], er_mix.inputs[2])
erl.new(er_mix.outputs['Shader'], er_out.inputs['Surface'])
einstein_ring.data.materials.append(er_mat)

# Vertical Einstein ring (light from behind, bent over the top)
bpy.ops.mesh.primitive_torus_add(
    major_radius=3.2,
    minor_radius=0.06,
    major_segments=128,
    minor_segments=16,
    location=(0, 0, 0)
)
er_vertical = bpy.context.active_object
er_vertical.name = "EinsteinRing_Vertical"
er_vertical.rotation_euler[0] = math.radians(90)
bpy.ops.object.shade_smooth()
er_vertical.data.materials.append(er_mat)

# ==================== RELATIVISTIC JETS ====================
# Bipolar jets shooting from the poles

def create_jet(name, direction_z, color):
    """Create a relativistic jet (cone of emission)"""
    # Cone for jet shape
    bpy.ops.mesh.primitive_cone_add(
        vertices=32, radius1=0.8, radius2=3.0, depth=25.0,
        location=(0, 0, direction_z * 14.0)
    )
    jet = bpy.context.active_object
    jet.name = name
    if direction_z < 0:
        jet.rotation_euler[0] = math.radians(180)
    bpy.ops.object.shade_smooth()

    # Jet emission material
    j_mat = bpy.data.materials.new(f"{name}_mat")
    j_mat.use_nodes = True
    if hasattr(j_mat, 'blend_method'):
        j_mat.blend_method = 'BLEND'
    jn = j_mat.node_tree.nodes
    jl = j_mat.node_tree.links
    jn.clear()

    j_out = jn.new('ShaderNodeOutputMaterial')
    j_emit = jn.new('ShaderNodeEmission')
    j_emit.inputs['Strength'].default_value = 6.0

    j_trans = jn.new('ShaderNodeBsdfTransparent')
    j_mix = jn.new('ShaderNodeMixShader')

    # Gradient along jet (stronger near base)
    j_texcoord = jn.new('ShaderNodeTexCoord')
    j_gradient = jn.new('ShaderNodeTexGradient')

    j_color_ramp = jn.new('ShaderNodeValToRGB')
    j_color_ramp.color_ramp.elements[0].position = 0.0
    j_color_ramp.color_ramp.elements[0].color = (*color, 1.0)
    mid = j_color_ramp.color_ramp.elements.new(0.3)
    mid.color = (0.5, 0.3, 1.0, 1.0)
    j_color_ramp.color_ramp.elements[-1].position = 1.0
    j_color_ramp.color_ramp.elements[-1].color = (0.1, 0.02, 0.3, 1.0)

    # Opacity gradient
    j_opacity = jn.new('ShaderNodeValToRGB')
    j_opacity.color_ramp.elements[0].position = 0.0
    j_opacity.color_ramp.elements[0].color = (1.0, 1.0, 1.0, 1.0)
    j_opacity.color_ramp.elements[1].position = 0.8
    j_opacity.color_ramp.elements[1].color = (0.0, 0.0, 0.0, 1.0)

    # Noise for turbulence
    j_noise = jn.new('ShaderNodeTexNoise')
    j_noise.inputs['Scale'].default_value = 3.0
    j_noise.inputs['Detail'].default_value = 6.0
    j_noise.inputs['Distortion'].default_value = 2.0

    j_multiply = jn.new('ShaderNodeMath')
    j_multiply.operation = 'MULTIPLY'

    jl.new(j_texcoord.outputs['Object'], j_gradient.inputs['Vector'])
    jl.new(j_texcoord.outputs['Object'], j_noise.inputs['Vector'])
    jl.new(j_gradient.outputs['Color'], j_color_ramp.inputs['Fac'])
    jl.new(j_color_ramp.outputs['Color'], j_emit.inputs['Color'])
    jl.new(j_gradient.outputs['Color'], j_opacity.inputs['Fac'])
    jl.new(j_opacity.outputs['Color'], j_multiply.inputs[0])
    jl.new(j_noise.outputs['Fac'], j_multiply.inputs[1])
    jl.new(j_multiply.outputs['Value'], j_mix.inputs['Fac'])
    jl.new(j_trans.outputs['BSDF'], j_mix.inputs[1])
    jl.new(j_emit.outputs['Emission'], j_mix.inputs[2])
    jl.new(j_mix.outputs['Shader'], j_out.inputs['Surface'])

    jet.data.materials.append(j_mat)
    return jet

# North jet (blue-purple)
jet_north = create_jet("Jet_North", 1, (0.3, 0.4, 1.0))
# South jet
jet_south = create_jet("Jet_South", -1, (0.3, 0.4, 1.0))

# ==================== INFALLING DEBRIS ====================
# Small chunks of matter spiraling into the black hole

random.seed(99)

debris_mat = bpy.data.materials.new("Debris_mat")
debris_mat.use_nodes = True
dn = debris_mat.node_tree.nodes
dl = debris_mat.node_tree.links
dn.clear()
d_out = dn.new('ShaderNodeOutputMaterial')
d_emit = dn.new('ShaderNodeEmission')
d_emit.inputs['Color'].default_value = (1.0, 0.6, 0.2, 1.0)
d_emit.inputs['Strength'].default_value = 8.0
dl.new(d_emit.outputs['Emission'], d_out.inputs['Surface'])

for i in range(30):
    size = random.uniform(0.03, 0.12)
    bpy.ops.mesh.primitive_ico_sphere_add(
        radius=size, subdivisions=2, location=(0, 0, 0)
    )
    debris = bpy.context.active_object
    debris.name = f"Debris_{i:02d}"
    debris.data.materials.append(debris_mat)

    # Spiral inward trajectory
    start_dist = random.uniform(8, 20)
    start_angle = random.uniform(0, 2 * math.pi)
    start_z = random.uniform(-1.5, 1.5)

    # Spiral parameters
    start_frame = random.randint(1, 500)
    spiral_time = random.randint(150, 400)
    num_orbits = random.uniform(1.5, 4.0)

    # Keyframes along spiral path
    steps = 12
    for s in range(steps + 1):
        t = s / steps
        # Radius decreases, angle increases
        r = start_dist * (1 - t * 0.85)  # Spiral in to 15% of start
        angle = start_angle + t * num_orbits * 2 * math.pi
        z = start_z * (1 - t)  # Flatten toward disk plane

        frame = start_frame + int(t * spiral_time)
        debris.location = (
            r * math.cos(angle),
            r * math.sin(angle),
            z
        )
        debris.keyframe_insert(data_path="location", frame=frame)

    # Hide after reaching center
    debris.hide_viewport = False
    debris.hide_render = False

    if debris.animation_data and debris.animation_data.action:
        for fc in debris.animation_data.action.fcurves:
            for kfp in fc.keyframe_points:
                kfp.interpolation = 'BEZIER'

# ==================== LIGHT SOURCES ====================
# Rim light to illuminate accretion disk edges

bpy.ops.object.light_add(type='POINT', location=(0, 0, 0))
center_light = bpy.context.active_object
center_light.name = "CenterGlow"
center_light.data.energy = 5000
center_light.data.color = (1.0, 0.7, 0.3)
center_light.data.shadow_soft_size = 2.0

# Top rim light
bpy.ops.object.light_add(type='AREA', location=(0, 0, 15))
top_light = bpy.context.active_object
top_light.name = "TopRim"
top_light.data.energy = 2000
top_light.data.color = (0.4, 0.5, 1.0)
top_light.data.size = 8
top_light.rotation_euler[0] = math.radians(180)

# Bottom rim
bpy.ops.object.light_add(type='AREA', location=(0, 0, -15))
bot_light = bpy.context.active_object
bot_light.name = "BotRim"
bot_light.data.energy = 2000
bot_light.data.color = (0.4, 0.5, 1.0)
bot_light.data.size = 8

# ==================== CAMERA - JOURNEY INTO THE HOLE ====================

bpy.ops.object.camera_add(location=(0, -60, 12))
camera = bpy.context.active_object
camera.name = "JourneyCamera"
camera.data.lens = 24  # Wide angle for dramatic perspective
camera.data.clip_start = 0.01
camera.data.clip_end = 500
scene.camera = camera

# Camera path: approach, orbit, dive in
# Phase 1 (1-200): Distant approach, seeing full structure
# Phase 2 (200-400): Close orbit around the disk plane
# Phase 3 (400-550): Spiral in toward event horizon
# Phase 4 (550-720): Cross event horizon, inside

keyframes = [
    # (frame, x, y, z)
    (1,    0,   -60,   12),    # Far away, above disk plane
    (60,   15,  -50,   8),     # Drift right, descending
    (120,  25,  -35,   4),     # Coming around, near disk level
    (200,  20,  -20,   2),     # Close approach, disk level
    (280,  10,  -12,   1),     # Spiraling in
    (360,  5,   -8,    0.5),   # Getting close to disk edge
    (440,  2,   -5,    0.2),   # Very close, seeing disk detail
    (520,  0.5, -3,    0.1),   # Almost at event horizon
    (580,  0,   -2.2,  0),     # At photon sphere
    (640,  0,   -1.5,  0),     # Crossing event horizon
    (680,  0,   -0.8,  0),     # Inside!
    (720,  0,   0,     0),     # Center of singularity
]

for frame, x, y, z in keyframes:
    camera.location = (x, y, z)
    camera.keyframe_insert(data_path="location", frame=frame)

# Camera always looks at the black hole center
track = camera.constraints.new('TRACK_TO')
track.target = event_horizon
track.track_axis = 'TRACK_NEGATIVE_Z'
track.up_axis = 'UP_Y'

# Smooth the camera path
if camera.animation_data and camera.animation_data.action:
    for fc in camera.animation_data.action.fcurves:
        for kfp in fc.keyframe_points:
            kfp.interpolation = 'BEZIER'
            kfp.handle_left_type = 'AUTO_CLAMPED'
            kfp.handle_right_type = 'AUTO_CLAMPED'

# ==================== INSIDE THE BLACK HOLE ====================
# Abstract geometry that appears as we cross the horizon
# Represents spacetime distortion

# Warped grid sphere (spaghettification visualization)
bpy.ops.mesh.primitive_uv_sphere_add(
    radius=1.5, segments=32, ring_count=16, location=(0, 0, 0)
)
inner_warp = bpy.context.active_object
inner_warp.name = "InnerWarp"
bpy.ops.object.shade_smooth()

warp_mat = bpy.data.materials.new("InnerWarp_mat")
warp_mat.use_nodes = True
if hasattr(warp_mat, 'blend_method'):
    warp_mat.blend_method = 'BLEND'
wn2 = warp_mat.node_tree.nodes
wl2 = warp_mat.node_tree.links
wn2.clear()

w_out = wn2.new('ShaderNodeOutputMaterial')
w_emit = wn2.new('ShaderNodeEmission')
w_emit.inputs['Strength'].default_value = 3.0

w_trans = wn2.new('ShaderNodeBsdfTransparent')
w_mix = wn2.new('ShaderNodeMixShader')

# Animated distortion
w_tc = wn2.new('ShaderNodeTexCoord')
w_noise = wn2.new('ShaderNodeTexNoise')
w_noise.inputs['Scale'].default_value = 8.0
w_noise.inputs['Detail'].default_value = 10.0
w_noise.inputs['Distortion'].default_value = 5.0

w_ramp = wn2.new('ShaderNodeValToRGB')
w_ramp.color_ramp.elements[0].position = 0.4
w_ramp.color_ramp.elements[0].color = (0.0, 0.0, 0.0, 1.0)
w_ramp.color_ramp.elements[1].position = 0.6
w_ramp.color_ramp.elements[1].color = (0.3, 0.1, 0.5, 1.0)

w_color = wn2.new('ShaderNodeValToRGB')
w_color.color_ramp.elements[0].position = 0.3
w_color.color_ramp.elements[0].color = (0.1, 0.0, 0.2, 1.0)
mid_c = w_color.color_ramp.elements.new(0.6)
mid_c.color = (0.0, 0.2, 0.8, 1.0)
w_color.color_ramp.elements[-1].position = 1.0
w_color.color_ramp.elements[-1].color = (1.0, 1.0, 1.0, 1.0)

wl2.new(w_tc.outputs['Object'], w_noise.inputs['Vector'])
wl2.new(w_noise.outputs['Fac'], w_ramp.inputs['Fac'])
wl2.new(w_noise.outputs['Fac'], w_color.inputs['Fac'])
wl2.new(w_color.outputs['Color'], w_emit.inputs['Color'])
wl2.new(w_ramp.outputs['Color'], w_mix.inputs['Fac'])
wl2.new(w_trans.outputs['BSDF'], w_mix.inputs[1])
wl2.new(w_emit.outputs['Emission'], w_mix.inputs[2])
wl2.new(w_mix.outputs['Shader'], w_out.inputs['Surface'])
inner_warp.data.materials.append(warp_mat)

# Animate noise distortion to increase as camera enters
# Animate the Distortion parameter for evolving patterns
w_noise.inputs['Distortion'].default_value = 2.0
w_noise.inputs['Distortion'].keyframe_insert(data_path="default_value", frame=1)
w_noise.inputs['Distortion'].default_value = 15.0
w_noise.inputs['Distortion'].keyframe_insert(data_path="default_value", frame=720)

# ==================== LENS DISTORTION SPHERE ====================
# Glass sphere around BH to simulate gravitational lensing

bpy.ops.mesh.primitive_uv_sphere_add(
    radius=4.0, segments=48, ring_count=24, location=(0, 0, 0)
)
lens_sphere = bpy.context.active_object
lens_sphere.name = "GravLens"
bpy.ops.object.shade_smooth()

lens_mat = bpy.data.materials.new("GravLens_mat")
lens_mat.use_nodes = True
if hasattr(lens_mat, 'blend_method'):
    lens_mat.blend_method = 'BLEND'
ln = lens_mat.node_tree.nodes
ll = lens_mat.node_tree.links
ln.clear()

l_out = ln.new('ShaderNodeOutputMaterial')
l_glass = ln.new('ShaderNodeBsdfGlass')
l_glass.inputs['Color'].default_value = (1.0, 1.0, 1.0, 1.0)
l_glass.inputs['Roughness'].default_value = 0.0
l_glass.inputs['IOR'].default_value = 1.05  # Subtle lensing

l_trans = ln.new('ShaderNodeBsdfTransparent')
l_mix = ln.new('ShaderNodeMixShader')
l_mix.inputs['Fac'].default_value = 0.15  # Mostly transparent

ll.new(l_trans.outputs['BSDF'], l_mix.inputs[1])
ll.new(l_glass.outputs['BSDF'], l_mix.inputs[2])
ll.new(l_mix.outputs['Shader'], l_out.inputs['Surface'])
lens_sphere.data.materials.append(lens_mat)

# ==================== FINAL SETUP ====================

def setup_viewport():
    for window in bpy.context.window_manager.windows:
        for area in window.screen.areas:
            if area.type == 'VIEW_3D':
                for space in area.spaces:
                    if space.type == 'VIEW_3D':
                        space.shading.type = 'MATERIAL'
                        space.shading.use_scene_lights = True
                        space.shading.use_scene_world = True
                        space.clip_start = 0.01
                        space.clip_end = 500
                        space.region_3d.view_perspective = 'CAMERA'

try:
    setup_viewport()
except:
    pass

def delayed_setup():
    try:
        setup_viewport()
    except:
        pass
    return None

bpy.app.timers.register(delayed_setup, first_interval=1.0)

bpy.ops.object.select_all(action='DESELECT')
scene.frame_current = 1

print("\n" + "=" * 60)
print("  BLACK HOLE - JOURNEY THROUGH EVENT HORIZON")
print("=" * 60)
print(f"  Objects: {len(bpy.data.objects)}")
print(f"  Animation: {scene.frame_end} frames @ {scene.render.fps}fps")
print(f"  Duration: {scene.frame_end / scene.render.fps:.1f} seconds")
print("  Camera path: approach -> orbit -> dive -> singularity")
print("=" * 60)
print("  Press SPACE to play the journey")
print("=" * 60 + "\n")
