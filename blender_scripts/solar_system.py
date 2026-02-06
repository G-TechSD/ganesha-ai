"""
Astronomically Accurate Solar System Animation
Created by Ganesha VLA + Agent K
Blender 4.0+ compatible
"""
import bpy
import math
import random

# ==================== CLEAR SCENE ====================
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.delete(use_global=True)

# Clear orphan data
for block in bpy.data.meshes:
    if block.users == 0:
        bpy.data.meshes.remove(block)
for block in bpy.data.materials:
    if block.users == 0:
        bpy.data.materials.remove(block)
for block in bpy.data.lights:
    if block.users == 0:
        bpy.data.lights.remove(block)

# ==================== SCENE SETUP ====================
scene = bpy.context.scene
scene.frame_start = 1
scene.frame_end = 600  # 20 seconds at 30fps
scene.frame_current = 1
scene.render.fps = 30
scene.render.resolution_x = 1920
scene.render.resolution_y = 1080

# Use EEVEE for fast preview (7900XT)
scene.render.engine = 'BLENDER_EEVEE'

# World - deep space
world = scene.world
if world is None:
    world = bpy.data.worlds.new("World")
    scene.world = world
world.use_nodes = True
nodes = world.node_tree.nodes
links = world.node_tree.links
nodes.clear()

bg = nodes.new('ShaderNodeBackground')
bg.inputs['Color'].default_value = (0.001, 0.001, 0.008, 1.0)
bg.inputs['Strength'].default_value = 1.0

output = nodes.new('ShaderNodeOutputWorld')
links.new(bg.outputs['Background'], output.inputs['Surface'])

# Add star noise to background
tex_coord = nodes.new('ShaderNodeTexCoord')
noise = nodes.new('ShaderNodeTexNoise')
noise.inputs['Scale'].default_value = 800.0
noise.inputs['Detail'].default_value = 16.0
noise.inputs['Roughness'].default_value = 0.9

color_ramp = nodes.new('ShaderNodeValToRGB')
color_ramp.color_ramp.elements[0].position = 0.72
color_ramp.color_ramp.elements[0].color = (0.0, 0.0, 0.0, 1.0)
color_ramp.color_ramp.elements[1].position = 0.74
color_ramp.color_ramp.elements[1].color = (1.0, 1.0, 1.0, 1.0)

mix = nodes.new('ShaderNodeMixShader')
star_emit = nodes.new('ShaderNodeEmission')
star_emit.inputs['Color'].default_value = (1.0, 1.0, 0.95, 1.0)
star_emit.inputs['Strength'].default_value = 3.0

links.new(tex_coord.outputs['Object'], noise.inputs['Vector'])
links.new(noise.outputs['Fac'], color_ramp.inputs['Fac'])
links.new(color_ramp.outputs['Color'], mix.inputs['Fac'])
links.new(bg.outputs['Background'], mix.inputs[1])
links.new(star_emit.outputs['Emission'], mix.inputs[2])
links.new(mix.outputs['Shader'], output.inputs['Surface'])

# ==================== MATERIALS ====================

def make_emission_mat(name, color, strength=5.0):
    """Glowing material for sun"""
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    nodes = mat.node_tree.nodes
    links = mat.node_tree.links
    nodes.clear()

    emit = nodes.new('ShaderNodeEmission')
    emit.inputs['Color'].default_value = (*color, 1.0)
    emit.inputs['Strength'].default_value = strength

    out = nodes.new('ShaderNodeOutputMaterial')
    links.new(emit.outputs['Emission'], out.inputs['Surface'])
    return mat

def make_planet_mat(name, base_color, noise_color=None, noise_scale=5.0,
                    roughness=0.7, noise_detail=4.0):
    """Procedural planet surface material"""
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    nodes = mat.node_tree.nodes
    links = mat.node_tree.links
    nodes.clear()

    out = nodes.new('ShaderNodeOutputMaterial')
    bsdf = nodes.new('ShaderNodeBsdfPrincipled')
    bsdf.inputs['Roughness'].default_value = roughness
    links.new(bsdf.outputs['BSDF'], out.inputs['Surface'])

    if noise_color:
        # Two-tone procedural texture
        tex_coord = nodes.new('ShaderNodeTexCoord')
        noise = nodes.new('ShaderNodeTexNoise')
        noise.inputs['Scale'].default_value = noise_scale
        noise.inputs['Detail'].default_value = noise_detail

        mix_rgb = nodes.new('ShaderNodeMix')
        mix_rgb.data_type = 'RGBA'
        mix_rgb.inputs[6].default_value = (*base_color, 1.0)   # A
        mix_rgb.inputs[7].default_value = (*noise_color, 1.0)  # B

        links.new(tex_coord.outputs['Object'], noise.inputs['Vector'])
        links.new(noise.outputs['Fac'], mix_rgb.inputs['Factor'])
        links.new(mix_rgb.outputs[2], bsdf.inputs['Base Color'])
    else:
        bsdf.inputs['Base Color'].default_value = (*base_color, 1.0)

    return mat

def make_ring_mat(name, color, alpha=0.6):
    """Semi-transparent ring material"""
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    mat.blend_method = 'BLEND' if hasattr(mat, 'blend_method') else None
    nodes = mat.node_tree.nodes
    links = mat.node_tree.links
    nodes.clear()

    out = nodes.new('ShaderNodeOutputMaterial')
    bsdf = nodes.new('ShaderNodeBsdfPrincipled')
    bsdf.inputs['Base Color'].default_value = (*color, 1.0)
    bsdf.inputs['Roughness'].default_value = 0.8
    bsdf.inputs['Alpha'].default_value = alpha

    links.new(bsdf.outputs['BSDF'], out.inputs['Surface'])
    return mat

# ==================== SOLAR SYSTEM DATA ====================
# Astronomically accurate ratios, artistically scaled for visibility
#
# Real data:
#   Sun radius: 696,340 km
#   Earth radius: 6,371 km (Sun is 109x bigger)
#   Earth-Sun distance: 149.6 million km (1 AU)
#
# Our scale:
#   Sun = 3.0 BU radius (artistic, not 109x Earth)
#   Planets exaggerated ~10-50x for visibility
#   Distances compressed with sqrt scaling
#   Orbital periods proportional to real Kepler ratios

PLANET_DATA = {
    # name: (radius_BU, distance_BU, orbital_period_frames,
    #        axial_tilt_deg, base_color, noise_color, noise_scale)
    "Mercury": (0.12, 5.5, 145, 0.03,
                (0.55, 0.52, 0.50), (0.40, 0.38, 0.35), 12.0),
    "Venus":   (0.30, 7.8, 370, 177.4,
                (0.90, 0.75, 0.40), (0.85, 0.65, 0.30), 6.0),
    "Earth":   (0.32, 10.5, 600, 23.44,
                (0.15, 0.35, 0.75), (0.20, 0.55, 0.25), 4.0),
    "Mars":    (0.18, 13.5, 1128, 25.19,
                (0.75, 0.30, 0.15), (0.60, 0.25, 0.10), 8.0),
    "Jupiter": (1.40, 22.0, 7116, 3.13,
                (0.80, 0.60, 0.40), (0.65, 0.45, 0.30), 3.0),
    "Saturn":  (1.15, 30.0, 17640, 26.73,
                (0.85, 0.75, 0.50), (0.75, 0.65, 0.40), 4.0),
    "Uranus":  (0.60, 40.0, 50400, 97.77,
                (0.55, 0.75, 0.85), (0.45, 0.65, 0.80), 5.0),
    "Neptune": (0.55, 50.0, 98400, 28.32,
                (0.20, 0.30, 0.80), (0.15, 0.25, 0.65), 6.0),
}

# Rings data: (inner_radius_mult, outer_radius_mult, color)
RING_DATA = {
    "Saturn": (1.5, 2.5, (0.82, 0.73, 0.55)),
    "Uranus": (1.3, 1.8, (0.50, 0.55, 0.60)),
}

# Moon data: (name, radius, distance_from_planet, orbital_period_frames, color)
MOON_DATA = {
    "Earth": [("Moon", 0.08, 0.8, 30, (0.65, 0.65, 0.65))],
}

# ==================== CREATE SUN ====================

bpy.ops.mesh.primitive_uv_sphere_add(
    radius=3.0, segments=64, ring_count=32, location=(0, 0, 0)
)
sun = bpy.context.active_object
sun.name = "Sun"
bpy.ops.object.shade_smooth()

sun_mat = make_emission_mat("Sun_mat", (1.0, 0.85, 0.4), strength=12.0)
sun.data.materials.append(sun_mat)

# Sun light
bpy.ops.object.light_add(type='POINT', location=(0, 0, 0))
sun_light = bpy.context.active_object
sun_light.name = "SunLight"
sun_light.data.energy = 80000
sun_light.data.color = (1.0, 0.95, 0.85)
sun_light.data.shadow_soft_size = 3.0

# Sun glow (secondary dimmer light for ambient)
bpy.ops.object.light_add(type='POINT', location=(0, 0, 0))
glow = bpy.context.active_object
glow.name = "SunGlow"
glow.data.energy = 5000
glow.data.color = (1.0, 0.9, 0.7)
glow.data.shadow_soft_size = 50.0

# ==================== CREATE PLANETS ====================

planet_objects = {}

for name, (radius, distance, period, tilt,
           base_col, noise_col, n_scale) in PLANET_DATA.items():

    # Orbit parent (empty at origin, rotates for orbital motion)
    bpy.ops.object.empty_add(type='PLAIN_AXES', location=(0, 0, 0))
    orbit_empty = bpy.context.active_object
    orbit_empty.name = f"{name}_Orbit"
    orbit_empty.empty_display_size = 0.5

    # Planet sphere
    bpy.ops.mesh.primitive_uv_sphere_add(
        radius=radius, segments=32, ring_count=16,
        location=(distance, 0, 0)
    )
    planet = bpy.context.active_object
    planet.name = name
    bpy.ops.object.shade_smooth()

    # Material
    mat = make_planet_mat(f"{name}_mat", base_col, noise_col, n_scale)
    planet.data.materials.append(mat)

    # Axial tilt
    planet.rotation_euler[0] = math.radians(tilt)

    # Parent planet to orbit empty
    planet.parent = orbit_empty

    # --- Orbital animation (orbit empty Z rotation) ---
    orbit_empty.rotation_euler[2] = 0.0
    orbit_empty.keyframe_insert(data_path="rotation_euler", frame=1, index=2)
    orbit_empty.rotation_euler[2] = math.radians(360)
    orbit_empty.keyframe_insert(data_path="rotation_euler", frame=period, index=2)

    # Linear interpolation + cycles modifier
    if orbit_empty.animation_data and orbit_empty.animation_data.action:
        for fc in orbit_empty.animation_data.action.fcurves:
            for kfp in fc.keyframe_points:
                kfp.interpolation = 'LINEAR'
            fc.modifiers.new(type='CYCLES')

    # --- Self-rotation animation ---
    rot_period = max(30, int(period * 0.02))  # Faster spin than orbit
    planet.rotation_euler[2] = 0.0
    planet.keyframe_insert(data_path="rotation_euler", frame=1, index=2)
    planet.rotation_euler[2] = math.radians(360)
    planet.keyframe_insert(data_path="rotation_euler", frame=rot_period, index=2)

    if planet.animation_data and planet.animation_data.action:
        for fc in planet.animation_data.action.fcurves:
            for kfp in fc.keyframe_points:
                kfp.interpolation = 'LINEAR'
            fc.modifiers.new(type='CYCLES')

    # --- Rings ---
    if name in RING_DATA:
        inner_m, outer_m, ring_col = RING_DATA[name]
        inner_r = radius * inner_m
        outer_r = radius * outer_m
        mid_r = (inner_r + outer_r) / 2
        thickness = (outer_r - inner_r) / 2

        bpy.ops.mesh.primitive_torus_add(
            major_radius=mid_r,
            minor_radius=thickness,
            major_segments=64,
            minor_segments=12,
            location=(distance, 0, 0)
        )
        ring = bpy.context.active_object
        ring.name = f"{name}_Rings"
        ring.scale[2] = 0.03  # Flatten to disk
        bpy.ops.object.shade_smooth()

        ring_mat = make_ring_mat(f"{name}_ring_mat", ring_col)
        ring.data.materials.append(ring_mat)
        ring.parent = orbit_empty

        # Match planet tilt
        ring.rotation_euler[0] = math.radians(tilt)

    # --- Moons ---
    if name in MOON_DATA:
        for moon_name, m_radius, m_dist, m_period, m_color in MOON_DATA[name]:
            # Moon orbit parent (relative to planet position)
            bpy.ops.object.empty_add(
                type='PLAIN_AXES',
                location=(distance, 0, 0)
            )
            moon_orbit = bpy.context.active_object
            moon_orbit.name = f"{moon_name}_Orbit"
            moon_orbit.parent = orbit_empty

            bpy.ops.mesh.primitive_uv_sphere_add(
                radius=m_radius, segments=16, ring_count=8,
                location=(distance + m_dist, 0, 0)
            )
            moon = bpy.context.active_object
            moon.name = moon_name
            bpy.ops.object.shade_smooth()

            moon_mat = make_planet_mat(f"{moon_name}_mat", m_color)
            moon.data.materials.append(moon_mat)
            moon.parent = moon_orbit

            # Moon orbital animation
            moon_orbit.rotation_euler[2] = 0.0
            moon_orbit.keyframe_insert(data_path="rotation_euler", frame=1, index=2)
            moon_orbit.rotation_euler[2] = math.radians(360)
            moon_orbit.keyframe_insert(data_path="rotation_euler", frame=m_period, index=2)

            if moon_orbit.animation_data and moon_orbit.animation_data.action:
                for fc in moon_orbit.animation_data.action.fcurves:
                    for kfp in fc.keyframe_points:
                        kfp.interpolation = 'LINEAR'
                    fc.modifiers.new(type='CYCLES')

    planet_objects[name] = (planet, orbit_empty)

    # --- Orbital path visualization ---
    bpy.ops.mesh.primitive_circle_add(
        radius=distance, vertices=128,
        fill_type='NOTHING', location=(0, 0, 0)
    )
    path = bpy.context.active_object
    path.name = f"{name}_Path"
    path_mat = make_emission_mat(f"{name}_path_mat", (0.15, 0.15, 0.25), strength=0.3)
    path.data.materials.append(path_mat)

# ==================== ASTEROID BELT ====================
# Between Mars (13.5) and Jupiter (22.0)

random.seed(42)

# Create one asteroid mesh to instance
bpy.ops.mesh.primitive_ico_sphere_add(radius=1.0, subdivisions=1, location=(0, 0, 0))
asteroid_template = bpy.context.active_object
asteroid_template.name = "AsteroidTemplate"
ast_mat = make_planet_mat("asteroid_mat", (0.45, 0.40, 0.35), (0.35, 0.30, 0.25), 15.0, 0.95)
asteroid_template.data.materials.append(ast_mat)

# Hide template
asteroid_template.hide_viewport = True
asteroid_template.hide_render = True

# Create asteroid instances
for i in range(150):
    angle = random.uniform(0, 2 * math.pi)
    dist = random.gauss(17.5, 1.5)  # Gaussian around belt center
    dist = max(15.0, min(20.0, dist))
    z_off = random.gauss(0, 0.3)
    size = random.uniform(0.03, 0.12)

    x = dist * math.cos(angle)
    y = dist * math.sin(angle)

    # Instance the template mesh
    asteroid = asteroid_template.copy()
    asteroid.data = asteroid_template.data
    asteroid.name = f"Belt_{i:03d}"
    asteroid.location = (x, y, z_off)
    asteroid.scale = (size, size * random.uniform(0.6, 1.0), size * random.uniform(0.6, 1.0))
    asteroid.rotation_euler = (
        random.uniform(0, math.pi),
        random.uniform(0, math.pi),
        random.uniform(0, math.pi)
    )
    asteroid.hide_viewport = False
    asteroid.hide_render = False
    bpy.context.collection.objects.link(asteroid)

    # Orbit parent
    bpy.ops.object.empty_add(type='PLAIN_AXES', location=(0, 0, 0))
    a_orbit = bpy.context.active_object
    a_orbit.name = f"BeltOrbit_{i:03d}"
    a_orbit.empty_display_size = 0.1
    asteroid.parent = a_orbit

    # Orbital animation (Kepler: period proportional to distance^1.5)
    period = int(600 * (dist / 10.5) ** 1.5)  # Earth=600 at dist 10.5
    a_orbit.rotation_euler[2] = 0.0
    a_orbit.keyframe_insert(data_path="rotation_euler", frame=1, index=2)
    a_orbit.rotation_euler[2] = math.radians(360)
    a_orbit.keyframe_insert(data_path="rotation_euler", frame=period, index=2)

    if a_orbit.animation_data and a_orbit.animation_data.action:
        for fc in a_orbit.animation_data.action.fcurves:
            for kfp in fc.keyframe_points:
                kfp.interpolation = 'LINEAR'
            fc.modifiers.new(type='CYCLES')

# ==================== STRAY ASTEROIDS ====================
# Fast-moving asteroids on hyperbolic-like trajectories

for i in range(8):
    size = random.uniform(0.08, 0.25)

    stray = asteroid_template.copy()
    stray.data = asteroid_template.data
    stray.name = f"Stray_{i}"
    stray.scale = (size, size * 0.7, size * 0.8)
    stray.hide_viewport = False
    stray.hide_render = False
    bpy.context.collection.objects.link(stray)

    # Random trajectory cutting through the system
    # Entry point (random edge of system)
    entry_angle = random.uniform(0, 2 * math.pi)
    entry_dist = random.uniform(55, 65)
    entry_z = random.uniform(-8, 8)

    # Closest approach (passes near inner system)
    closest = random.uniform(3, 25)
    mid_angle = entry_angle + random.uniform(1.5, 3.0)
    mid_z = random.uniform(-3, 3)

    # Exit point
    exit_angle = mid_angle + random.uniform(1.0, 2.5)
    exit_dist = random.uniform(55, 65)
    exit_z = random.uniform(-8, 8)

    start_frame = random.randint(1, 350)
    transit_time = random.randint(100, 250)

    # Three keyframes: entry, closest approach, exit
    stray.location = (
        entry_dist * math.cos(entry_angle),
        entry_dist * math.sin(entry_angle),
        entry_z
    )
    stray.keyframe_insert(data_path="location", frame=start_frame)

    stray.location = (
        closest * math.cos(mid_angle),
        closest * math.sin(mid_angle),
        mid_z
    )
    stray.keyframe_insert(data_path="location", frame=start_frame + transit_time // 2)

    stray.location = (
        exit_dist * math.cos(exit_angle),
        exit_dist * math.sin(exit_angle),
        exit_z
    )
    stray.keyframe_insert(data_path="location", frame=start_frame + transit_time)

    # Tumbling rotation
    stray.rotation_euler = (0, 0, 0)
    stray.keyframe_insert(data_path="rotation_euler", frame=1)
    stray.rotation_euler = (
        random.uniform(10, 30),
        random.uniform(10, 30),
        random.uniform(10, 30)
    )
    stray.keyframe_insert(data_path="rotation_euler", frame=600)

    if stray.animation_data and stray.animation_data.action:
        for fc in stray.animation_data.action.fcurves:
            for kfp in fc.keyframe_points:
                kfp.interpolation = 'LINEAR'

# ==================== CAMERA ====================

bpy.ops.object.camera_add(location=(35, -55, 30))
camera = bpy.context.active_object
camera.name = "SolarCamera"
camera.data.lens = 28  # Wide angle to see more
camera.data.clip_end = 500
scene.camera = camera

# Track to sun
track = camera.constraints.new('TRACK_TO')
track.target = sun
track.track_axis = 'TRACK_NEGATIVE_Z'
track.up_axis = 'UP_Y'

# Slow camera orbit
bpy.ops.object.empty_add(type='PLAIN_AXES', location=(0, 0, 0))
cam_orbit = bpy.context.active_object
cam_orbit.name = "CameraOrbit"
camera.parent = cam_orbit

cam_orbit.rotation_euler[2] = 0.0
cam_orbit.keyframe_insert(data_path="rotation_euler", frame=1, index=2)
cam_orbit.rotation_euler[2] = math.radians(60)  # 60 degree sweep over animation
cam_orbit.keyframe_insert(data_path="rotation_euler", frame=600, index=2)

if cam_orbit.animation_data and cam_orbit.animation_data.action:
    for fc in cam_orbit.animation_data.action.fcurves:
        for kfp in fc.keyframe_points:
            kfp.interpolation = 'LINEAR'

# ==================== FINAL SETUP ====================

# Set viewport shading to Material Preview and camera view
# Use override context to ensure it works when run via --python
def setup_viewport():
    for window in bpy.context.window_manager.windows:
        for area in window.screen.areas:
            if area.type == 'VIEW_3D':
                for space in area.spaces:
                    if space.type == 'VIEW_3D':
                        # Material Preview mode
                        space.shading.type = 'MATERIAL'
                        space.shading.use_scene_lights = True
                        space.shading.use_scene_world = False
                        space.clip_end = 500
                        # Camera view
                        space.region_3d.view_perspective = 'CAMERA'

# Try immediate setup
try:
    setup_viewport()
except:
    pass

# Also register a timer to set it after UI is ready
def delayed_setup():
    try:
        setup_viewport()
    except:
        pass
    return None  # Don't repeat

bpy.app.timers.register(delayed_setup, first_interval=0.5)

# Deselect all for clean view
bpy.ops.object.select_all(action='DESELECT')
scene.frame_current = 1

# Render a preview frame to file for verification
scene.render.filepath = "/tmp/solar_system_render.png"
scene.render.resolution_percentage = 50

print("\n" + "=" * 60)
print("  SOLAR SYSTEM CREATED SUCCESSFULLY")
print("=" * 60)
print(f"  Objects: {len(bpy.data.objects)}")
print(f"  Planets: {len(PLANET_DATA)}")
print(f"  Belt asteroids: 150")
print(f"  Stray asteroids: 8")
print(f"  Animation: {scene.frame_end} frames @ {scene.render.fps}fps")
print(f"  Duration: {scene.frame_end / scene.render.fps:.1f} seconds")
print("=" * 60)
print("  Press SPACE to play animation")
print("  Press F12 to render a frame")
print("=" * 60 + "\n")
