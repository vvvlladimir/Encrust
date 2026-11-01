// The render view's light: the key's soft shadow, the studio and its prefiltered
// reflections, AgX, the layers' sheen. Joined to shader.wgsl; see docs/design/viewport.md.

// How far round its edge the key wraps its light onto a curve, as a softbox that large does.
const STUDIO_WRAP = 0.35;

fn wrapped(facing: f32) -> f32 {
    return max((facing + STUDIO_WRAP) / (1.0 + STUDIO_WRAP), 0.0);
}

// The key's shadow as percentage-closer soft shadows (Fernando 2005), from a point lifted
// further the more it turns from the key. See docs/design/viewport.md.
const KEY_LIFT_MM = 0.4;
const KEY_TURNED_LIFT_MM = 2.0;
const KEY_TAPS = 12;
// Half the angle the key's softbox spans, radians, and the least penumbra, mm.
const KEY_HALF_ANGLE = 0.035;
const KEY_LEAST_SOFT_MM = 0.4;
// How far round a point blockers are looked for, mm: the penumbra of the tallest prop.
const KEY_SEARCH_MM = 12.0;
// How much nearer the key than the surface's own plane a depth must be to stand over it, mm.
const KEY_BLOCKER_GAP_MM = 1.0;

fn spiral(tap: i32) -> vec2<f32> {
    let angle = f32(tap) * 2.39996323;
    return vec2<f32>(cos(angle), sin(angle)) * sqrt((f32(tap) + 0.5) / f32(KEY_TAPS));
}

// How much of the key light reaches `world` on a surface facing `normal`: all of it outside
// the render view or off the map.
fn key_reaches(world: vec3<f32>, normal: vec3<f32>) -> f32 {
    if (!under_studio()) {
        return 1.0;
    }
    let turned = 1.0 - clamp(dot(normal, normalize(-globals.key_towards.xyz)), 0.0, 1.0);
    let lift = KEY_LIFT_MM + KEY_TURNED_LIFT_MM * turned;
    let clip = globals.key_view_projection * vec4<f32>(world + normal * lift, 1.0);
    let at = clip.xyz / clip.w;
    let uv = at.xy * vec2<f32>(0.5, -0.5) + 0.5;
    if (any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0)) || at.z > 1.0) {
        return 1.0;
    }
    // A penumbra is not seen in motion: one filtered tap.
    if (LOOK == 3u) {
        return textureSampleCompareLevel(key_shadow_map, key_shadow_sampler, uv, at.z);
    }
    // The map is orthographic: its depth and its texels are mm by fixed factors.
    let size = vec2<f32>(textureDimensions(key_shadow_map));
    let depth_mm = 1.0 / abs(globals.key_view_projection[2][2]);
    let texels_per_mm = abs(globals.key_view_projection[0][0]) * 0.5 * size.x;
    // The surface's own plane under each probe, so a slope the key falls across is not
    // taken for something standing over it (receiver plane depth bias).
    let m = globals.key_view_projection;
    let right = normalize(vec3<f32>(m[0][0], m[1][0], m[2][0]));
    let up = normalize(vec3<f32>(m[0][1], m[1][1], m[2][1]));
    let towards = normalize(vec3<f32>(m[0][2], m[1][2], m[2][2]));
    let facing = dot(normal, towards);
    var blockers = 0.0;
    var nearest = 0.0;
    for (var tap = 0; tap < KEY_TAPS; tap = tap + 1) {
        let offset_mm = spiral(tap) * KEY_SEARCH_MM;
        let probe = uv + offset_mm * texels_per_mm / size;
        let texel = vec2<i32>(clamp(probe, vec2<f32>(0.0), vec2<f32>(1.0)) * (size - 1.0));
        let stood = textureLoad(key_shadow_map, texel, 0);
        let across = right * offset_mm.x - up * offset_mm.y;
        let along_mm = clamp(-dot(normal, across) / select(facing, -1e-3, abs(facing) < 1e-3), -KEY_SEARCH_MM * 4.0, KEY_SEARCH_MM * 4.0);
        let surface = at.z + along_mm / depth_mm;
        if (stood < surface - KEY_BLOCKER_GAP_MM / depth_mm) {
            blockers = blockers + 1.0;
            nearest = nearest + stood;
        }
    }
    if (blockers < 0.5) {
        return 1.0;
    }
    let under_mm = (at.z - nearest / blockers) * depth_mm;
    let soft_mm = max(KEY_LEAST_SOFT_MM, under_mm * KEY_HALF_ANGLE * 2.0);
    let reach = soft_mm * texels_per_mm / size;
    var lit = 0.0;
    for (var tap = 0; tap < KEY_TAPS; tap = tap + 1) {
        let shifted = uv + spiral(tap) * reach;
        lit = lit + textureSampleCompareLevel(key_shadow_map, key_shadow_sampler, shifted, at.z);
    }
    return lit / f32(KEY_TAPS);
}

// A mesh as the key light sees it, for its shadow map.
@vertex
fn key_cast_vertex(in: ModelVertex) -> @builtin(position) vec4<f32> {
    let model = mat4x4<f32>(in.model_0, in.model_1, in.model_2, in.model_3);
    return globals.key_view_projection * model * vec4<f32>(in.position, 1.0);
}

// The render view lights in linear light and hands the sum to a film's curve, AgX, before it
// goes out in the gamma the target is written in. See docs/design/viewport.md.
const EXPOSURE = 1.4;
// A milder punchy look over AgX, its power alone: a resin's colour holds its
// strength instead of AgX's own muted base, its hue untouched.
const LOOK_POWER = 1.15;

fn linear_of(gamma: vec3<f32>) -> vec3<f32> {
    return pow(max(gamma, vec3<f32>(0.0)), vec3<f32>(2.2));
}

// AgX's sigmoid as a fitted polynomial, over log2 exposure.
fn agx_contrast(x: vec3<f32>) -> vec3<f32> {
    let x2 = x * x;
    let x4 = x2 * x2;
    return 15.5 * x4 * x2 - 40.14 * x4 * x + 31.96 * x4 - 6.868 * x2 * x + 0.4298 * x2 + 0.1191 * x - 0.00232;
}

// Linear sRGB light to the gamma the target takes, through AgX.
fn filmic(linear: vec3<f32>) -> vec3<f32> {
    let srgb_to_rec2020 = mat3x3<f32>(
        vec3<f32>(0.6274, 0.0691, 0.0164),
        vec3<f32>(0.3293, 0.9195, 0.0880),
        vec3<f32>(0.0433, 0.0113, 0.8956),
    );
    let rec2020_to_srgb = mat3x3<f32>(
        vec3<f32>(1.6605, -0.1246, -0.0182),
        vec3<f32>(-0.5876, 1.1329, -0.1006),
        vec3<f32>(-0.0728, -0.0083, 1.1187),
    );
    let inset = mat3x3<f32>(
        vec3<f32>(0.856627153315983, 0.137318972929847, 0.11189821299995),
        vec3<f32>(0.0951212405381588, 0.761241990602591, 0.0767994186031903),
        vec3<f32>(0.0482516061458583, 0.101439036467562, 0.811302368396859),
    );
    let outset = mat3x3<f32>(
        vec3<f32>(1.1271005818144368, -0.1413297634984383, -0.14132976349843826),
        vec3<f32>(-0.11060664309660323, 1.157823702216272, -0.11060664309660294),
        vec3<f32>(-0.016493938717834573, -0.016493938717834257, 1.2519364065950405),
    );
    let low_ev = -12.47393;
    let high_ev = 4.026069;
    var colour = inset * (srgb_to_rec2020 * (linear * EXPOSURE));
    colour = clamp((log2(max(colour, vec3<f32>(1e-10))) - low_ev) / (high_ev - low_ev), vec3<f32>(0.0), vec3<f32>(1.0));
    colour = outset * pow(max(agx_contrast(colour), vec3<f32>(0.0)), vec3<f32>(LOOK_POWER));
    colour = rec2020_to_srgb * pow(max(colour, vec3<f32>(0.0)), vec3<f32>(2.2));
    return pow(clamp(colour, vec3<f32>(0.0), vec3<f32>(1.0)), vec3<f32>(1.0 / 2.2));
}

// `lit` under the studio, for the same premultiplied `base` and `alpha`.
fn studio(base: vec3<f32>, alpha: f32, normal: vec3<f32>, world: vec3<f32>) -> vec3<f32> {
    return filmic(radiance(linear_of(base), 0.0, RESIN_ROUGHNESS, alpha, normal, normal, world));
}

// The same over layers too fine to see grooving the surface by `grooves`: reflections
// stretch across them, a satin sheen, the normal bent as KHR_materials_anisotropy bends it.
fn studio_grooved(base: vec3<f32>, alpha: f32, normal: vec3<f32>, world: vec3<f32>, grooves: f32) -> vec3<f32> {
    let view = normalize(globals.eye.xyz - world);
    let bent = bent_by_layers(normal, view, grooves);
    let sheen = layer_sheen(normal, view, world, grooves) * alpha;
    let roughness = mix(RESIN_ROUGHNESS, GROOVED_ROUGHNESS, grooves);
    return filmic(radiance(linear_of(base), 0.0, roughness, alpha, normal, bent, world) + sheen);
}

// The key's highlight off the grooves, Ward's anisotropic lobe: narrow along the layers,
// stretched across them by how deep they groove. Its share of the key, and its widths.
const SHEEN_SHARE = 0.35;
const SHEEN_ALONG = 0.12;
const SHEEN_ACROSS = 0.55;

fn layer_sheen(normal: vec3<f32>, view: vec3<f32>, world: vec3<f32>, grooves: f32) -> vec3<f32> {
    let across = across_layers(normal);
    let key = normalize(-globals.key_towards.xyz);
    let lit = dot(normal, key);
    if (grooves <= 0.0 || dot(across, across) < 0.5 || lit <= 0.0) {
        return vec3<f32>(0.0);
    }
    let along = cross(across, normal);
    let half_way = normalize(key + view);
    let width_across = SHEEN_ALONG + SHEEN_ACROSS * grooves;
    let up = max(dot(half_way, normal), 1e-3);
    let spread = pow(dot(half_way, along) / SHEEN_ALONG, 2.0) + pow(dot(half_way, across) / width_across, 2.0);
    let lobe = exp(-spread / (up * up)) / (4.0 * 3.14159265 * SHEEN_ALONG * width_across * sqrt(lit * max(dot(normal, view), 1e-3)));
    let reaches = key_reaches(world, normal);
    return linear_of(globals.key_color.rgb) * SHEEN_SHARE * grooves * lobe * lit * reaches;
}

// How far the layers' grooves bend the normal a reflection is taken about, at their most.
const SHEEN_BEND = 0.65;

// KHR_materials_anisotropy's bent normal: towards the one facing the eye in the plane the
// grooves stretch the light in, up the surface across the layers.
fn bent_by_layers(normal: vec3<f32>, view: vec3<f32>, grooves: f32) -> vec3<f32> {
    let stretch = across_layers(normal);
    if (grooves <= 0.0 || dot(stretch, stretch) < 0.5) {
        return normal;
    }
    let along = cross(stretch, view);
    let facing = cross(along, stretch);
    return normalize(mix(normal, facing, grooves * SHEEN_BEND));
}

// A surface under the studio: `metal` 0.0 a dielectric reflecting white to 1.0 a metal
// reflecting its own colour, `roughness` 0.0 a mirror to 1.0 chalk.
fn finish(base: vec3<f32>, metal: f32, roughness: f32, normal: vec3<f32>, world: vec3<f32>) -> vec3<f32> {
    return filmic(radiance(linear_of(base), metal, roughness, 1.0, normal, normal, world));
}

// How much of the key a thin part of a resin lets through to the eye, 1.0 for none of its
// thickness: set by `glow_fragment` for the fragment in hand, 0.0 everywhere else.
var<private> glow_through: f32 = 0.0;

// Light through a thin part, after Barré-Brisebois and Bouchard (GDC 2011), carried through
// `exp(-d / GLOW_MM)` of a part `d` thick. See docs/design/viewport.md.
const GLOW_MM = 2.5;
const GLOW_DISTORTION = 0.2;
const GLOW_POWER = 4.0;
const GLOW_SCALE = 1.2;
const GLOW_AMBIENT = 0.08;

// The studio every surface sees round itself, linear light: a pale sky and three softboxes,
// key, overhead and warm fill. Baked into `environment_map`; see docs/design/viewport.md.
const KEY_BOX = 3.0;
const TOP_BOX = 1.1;
const FILL_BOX = 0.5;
// Half the angles the softboxes span, radians.
const KEY_BOX_RAD = 0.24;
const TOP_BOX_RAD = 0.42;
const FILL_BOX_RAD = 0.3;

fn softbox_disc(towards: vec3<f32>, centre: vec3<f32>, half_angle: f32) -> f32 {
    let off = acos(clamp(dot(towards, centre), -1.0, 1.0));
    return 1.0 - smoothstep(half_angle * 0.85, half_angle * 1.1, off);
}

fn studio_sky(towards: vec3<f32>) -> vec3<f32> {
    let cove = linear_of(COVE_COLOUR);
    var seen = cove * 0.3;
    if (towards.z >= 0.0) {
        seen = mix(cove * 0.55, linear_of(globals.sky.rgb) * 0.7, smoothstep(0.0, 0.7, towards.z));
    }
    let key = normalize(-globals.key_towards.xyz);
    let fill = normalize(-globals.fill_towards.xyz);
    seen = seen + linear_of(globals.key_color.rgb) * KEY_BOX * softbox_disc(towards, key, KEY_BOX_RAD);
    seen = seen + linear_of(globals.sky.rgb) * TOP_BOX * softbox_disc(towards, vec3<f32>(0.0, 0.0, 1.0), TOP_BOX_RAD);
    return seen + linear_of(globals.fill_color.rgb) * FILL_BOX * softbox_disc(towards, fill, FILL_BOX_RAD);
}

// The map's mips, roughness 0.0 to 1.0 evenly between. Must match MIPS in environment.rs.
const ENVIRONMENT_MIPS = 6.0;
const PREFILTER_SAMPLES = 128u;

struct EnvironmentMip {
    roughness_and_size: vec4<f32>,
};

@group(1) @binding(0) var<uniform> environment_mip: EnvironmentMip;

fn equirect_of(towards: vec3<f32>) -> vec2<f32> {
    let around = atan2(towards.y, towards.x);
    let down = acos(clamp(towards.z, -1.0, 1.0));
    return vec2<f32>(around / 6.2831853 + 0.5, down / 3.14159265);
}

fn towards_of(equirect: vec2<f32>) -> vec3<f32> {
    let around = (equirect.x - 0.5) * 6.2831853;
    let down = equirect.y * 3.14159265;
    return vec3<f32>(sin(down) * cos(around), sin(down) * sin(around), cos(down));
}

fn hammersley(index: u32, count: u32) -> vec2<f32> {
    return vec2<f32>(f32(index) / f32(count), f32(reverseBits(index)) * 2.3283064365386963e-10);
}

// A half-way vector about `normal` drawn from GGX's distribution at `roughness` (Karis 2013).
fn ggx_half_way(at: vec2<f32>, normal: vec3<f32>, roughness: f32) -> vec3<f32> {
    let a = roughness * roughness;
    let around = 6.2831853 * at.x;
    let cos_up = sqrt((1.0 - at.y) / (1.0 + (a * a - 1.0) * at.y));
    let sin_up = sqrt(1.0 - cos_up * cos_up);
    let up = select(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 0.0, 1.0), abs(normal.z) < 0.999);
    let side = normalize(cross(up, normal));
    let other = cross(normal, side);
    return normalize(side * sin_up * cos(around) + other * sin_up * sin(around) + normal * cos_up);
}

// One texel of one mip: the studio seen by a surface of that mip's roughness facing it,
// the normal, the eye and the reflection taken as one, as the split sum does.
@fragment
fn prefilter_fragment(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let roughness = environment_mip.roughness_and_size.x;
    let normal = towards_of(position.xy / environment_mip.roughness_and_size.yz);
    if (roughness < 1e-3) {
        return vec4<f32>(studio_sky(normal), 1.0);
    }
    var gathered = vec3<f32>(0.0);
    var weight = 0.0;
    for (var index = 0u; index < PREFILTER_SAMPLES; index = index + 1u) {
        let half_way = ggx_half_way(hammersley(index, PREFILTER_SAMPLES), normal, roughness);
        let light = normalize(2.0 * dot(normal, half_way) * half_way - normal);
        let facing = dot(normal, light);
        if (facing > 0.0) {
            gathered = gathered + studio_sky(light) * facing;
            weight = weight + facing;
        }
    }
    return vec4<f32>(gathered / max(weight, 1e-4), 1.0);
}

// The studio seen along `towards` by a surface of `roughness`.
fn environment(towards: vec3<f32>, roughness: f32) -> vec3<f32> {
    let lod = clamp(roughness, 0.0, 1.0) * (ENVIRONMENT_MIPS - 1.0);
    return textureSampleLevel(environment_map, environment_sampler, equirect_of(towards), lod).rgb;
}

// The split sum's second half without its table: Karis's analytic fit (2014) of the share of
// the prefiltered light a surface of reflectance `f0` returns.
fn reflected_share(f0: vec3<f32>, roughness: f32, facing: f32) -> vec3<f32> {
    let c0 = vec4<f32>(-1.0, -0.0275, -0.572, 0.022);
    let c1 = vec4<f32>(1.0, 0.0425, 1.04, -0.04);
    let r = roughness * c0 + c1;
    let a004 = min(r.x * r.x, exp2(-9.28 * facing)) * r.x + r.y;
    let ab = vec2<f32>(-1.04, 1.04) * a004 + r.zw;
    return f0 * ab.x + ab.y;
}

// How rough a resin print is, and a groove of its layers at its deepest.
const RESIN_ROUGHNESS = 0.32;
const GROOVED_ROUGHNESS = 0.55;
// How much of the room's light a surface takes in, and of the key's straight on.
const ROOM_SHARE = 0.42;
const KEY_SHARE_STUDIO = 1.05;
// How much of its reflection a surface keeps where the key's shadow falls on it.
const SHADED_REFLECTION = 0.55;

// The light leaving a surface of linear `base` before the curve, lit about `normal` and
// reflecting about `reflecting`; `strength` scales a premultiplied surface's reflection.
fn radiance(base: vec3<f32>, metal: f32, roughness: f32, strength: f32, normal: vec3<f32>, reflecting: vec3<f32>, world: vec3<f32>) -> vec3<f32> {
    let view = normalize(globals.eye.xyz - world);
    let key = normalize(-globals.key_towards.xyz);
    let key_colour = linear_of(globals.key_color.rgb);
    let reaches = key_reaches(world, normal);
    let diffuse = environment(normal, 1.0) * ROOM_SHARE
        + key_colour * KEY_SHARE_STUDIO * wrapped(dot(normal, key)) * reaches;
    let mirror = reflect(-view, reflecting);
    let facing = max(dot(reflecting, view), 1e-3);
    let f0 = mix(vec3<f32>(0.04), base, metal);
    let reflected = environment(mirror, roughness) * reflected_share(f0, roughness, facing)
        * mix(SHADED_REFLECTION, 1.0, reaches);
    let behind = pow(clamp(dot(view, -(key + normal * GLOW_DISTORTION)), 0.0, 1.0), GLOW_POWER);
    let glow = key_colour * (behind * GLOW_SCALE + GLOW_AMBIENT) * glow_through;
    return base * (1.0 - metal) * (diffuse + glow) + reflected * strength;
}
