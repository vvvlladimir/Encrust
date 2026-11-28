#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

use std::io::Cursor;
use std::sync::Arc;

use core_analysis::{Cured, Measured, cure};
use core_engine::project::inner_faces;
use core_engine::{
    Cutting, DrainSearch, Model, NeedsAsked, Plate, QUICK_CHECK_BIN, Run, SupportPlan,
    analysis_slicing, needs_on, suggest_drains, suggest_drains_heard,
};
use core_format::ExposurePlan;
use core_geometry::{Bvh, Mesh, Scalar, Transform, Vec3};
use core_pipeline::{PanelOverrides, SlicedFormat};
use core_raster::{LayerRuns, RasterSettings, Rasterizer, ScanlineRasterizer};
use core_slicer::{Layer, PlaneSliceEngine, SliceEngine, SliceError, SliceSettings};
use core_supports::{WallSide, low_points};
use core_volume::{Cancel, HoleSize, HollowSettings, hollow};
use criterion::measurement::WallTime;
use criterion::{BatchSize, BenchmarkGroup, Criterion, criterion_group, criterion_main};
use printer_profiles::{Catalogue, MaterialProfile, SupportProfile};
use rayon::prelude::*;

/// Roughly 80k triangles over 20 mm of height: a detailed miniature at print resolution.
fn sphere(radius: Scalar, rings: usize, segments: usize) -> Mesh {
    let mut vertices = vec![Vec3::new(0.0, 0.0, radius)];
    for ring in 1..rings {
        let theta = std::f32::consts::PI * ring as Scalar / rings as Scalar;
        let (sin_theta, cos_theta) = theta.sin_cos();
        for segment in 0..segments {
            let phi = std::f32::consts::TAU * segment as Scalar / segments as Scalar;
            let (sin_phi, cos_phi) = phi.sin_cos();
            vertices.push(Vec3::new(
                radius * sin_theta * cos_phi,
                radius * sin_theta * sin_phi,
                radius * cos_theta,
            ));
        }
    }
    let south = vertices.len() as u32;
    vertices.push(Vec3::new(0.0, 0.0, -radius));

    let at = |ring: usize, segment: usize| (1 + (ring - 1) * segments + segment % segments) as u32;
    let mut faces = Vec::new();
    for segment in 0..segments {
        faces.push([0, at(1, segment), at(1, segment + 1)]);
    }
    for ring in 1..rings - 1 {
        for segment in 0..segments {
            faces.push([
                at(ring, segment),
                at(ring + 1, segment),
                at(ring + 1, segment + 1),
            ]);
            faces.push([
                at(ring, segment),
                at(ring + 1, segment + 1),
                at(ring, segment + 1),
            ]);
        }
    }
    for segment in 0..segments {
        faces.push([at(rings - 1, segment), south, at(rings - 1, segment + 1)]);
    }
    Mesh::new(vertices, faces)
}

const RADIUS_MM: Scalar = 10.0;
/// Five copies to a row, 4 mm apart: twenty fill 118 x 94 mm of the panel.
const ROW: usize = 5;
const PITCH_MM: Scalar = 2.0 * RADIUS_MM + 4.0;

/// `copies` of one sphere, sharing its mesh the way a duplicated model does, standing
/// on the plate in rows.
fn plate_of(copies: usize) -> Plate {
    let mesh = Arc::new(sphere(RADIUS_MM, 200, 200));
    let models = (0..copies)
        .map(|copy| {
            let at = Vec3::new(
                30.0 + PITCH_MM * (copy % ROW) as Scalar,
                15.0 + PITCH_MM * (copy / ROW) as Scalar,
                RADIUS_MM,
            );
            Model::placed(Arc::clone(&mesh), Transform::from_translation(at))
        })
        .collect();
    let catalogue = Catalogue::bundled().expect("the shipped profiles parse");
    Plate {
        models,
        printer: catalogue
            .shipped_printer("elegoo-saturn-4-ultra")
            .expect("the printer is shipped"),
        material: MaterialProfile::default(),
        panel: PanelOverrides::default(),
        cutting: Cutting::uniform(0.05),
        exposure: ExposurePlan::default(),
        remove_islands: false,
        hanging_passes: 0,
        format: SlicedFormat::Goo,
        raster_window: 16,
        created_unix_s: 0,
    }
}

fn plates(c: &mut Criterion) {
    let mut group = c.benchmark_group("plate");
    group.sample_size(10);

    for copies in [1, 20] {
        let plate = plate_of(copies);
        group.bench_function(format!("bake_{copies}_spheres"), |b| {
            b.iter(|| Run::of(&plate).expect("the plate has geometry"));
        });

        let run = Run::of(&plate).expect("the plate has geometry");
        group.bench_function(format!("measure_{copies}_spheres"), |b| {
            b.iter(|| run.measure(&mut ()).expect("the stack measures"));
        });
        group.bench_function(format!("quick_check_{copies}_spheres"), |b| {
            b.iter(|| {
                run.measure_binned(QUICK_CHECK_BIN, &mut ())
                    .expect("the stack measures")
            });
        });
        group.bench_function(format!("write_goo_{copies}_spheres"), |b| {
            b.iter(|| {
                let mut sink = Cursor::new(Vec::new());
                run.write("plate", &mut sink, &mut ())
                    .expect("the stack writes");
                sink
            });
        });
    }
    group.finish();
}

/// The stages `measure` runs, one at a time, over the whole stack held at once.
fn stages(c: &mut Criterion) {
    let mut group = c.benchmark_group("plate_stage");
    group.sample_size(10);

    for copies in [1, 20] {
        let plate = plate_of(copies);
        let run = Run::of(&plate).expect("the plate has geometry");
        let panel = *run.panel();
        group.bench_function(format!("slice_{copies}_spheres"), |b| {
            b.iter(|| {
                run.windows()
                    .stream(run.solid(), |sliced| {
                        std::hint::black_box(sliced);
                        Ok::<_, SliceError>(())
                    })
                    .expect("the plate slices");
            });
        });

        let layers = run
            .windows()
            .cut(run.solid(), 0..run.layer_count())
            .expect("the plate slices")
            .layers;
        group.bench_function(format!("raster_{copies}_spheres"), |b| {
            b.iter(|| rastered(&layers, &panel));
        });

        let runs = rastered(&layers, &panel);
        group.bench_function(format!("cure_{copies}_spheres"), |b| {
            b.iter(|| cured(&runs, &panel));
        });

        let cured = cured(&runs, &panel);
        group.bench_function(format!("fold_{copies}_spheres"), |b| {
            b.iter_batched(
                || cured.clone(),
                |cured| {
                    let mut measured = Measured::new(plate.material.bottom_layers as usize);
                    for layer in cured {
                        measured.push(layer, plate.cutting.layer_height_mm);
                    }
                    measured.finish()
                },
                BatchSize::LargeInput,
            );
        });
    }
    group.finish();
}

fn rastered(layers: &[Layer], panel: &RasterSettings) -> Vec<LayerRuns> {
    layers
        .par_iter()
        .map(|layer| {
            ScanlineRasterizer
                .rasterize(layer, panel)
                .expect("the plate fits the panel")
                .runs
        })
        .collect()
}

fn cured(runs: &[LayerRuns], panel: &RasterSettings) -> Vec<Cured> {
    runs.par_iter()
        .map(|runs| cure(runs, panel.pitch))
        .collect()
}

/// `--drain-at auto` on a 40 mm ball hollowed to a 2 mm wall, standing on the plate, at a
/// print's layer: each scan cuts the whole stack, so the search costs its scans.
fn drains(c: &mut Criterion) {
    let ball = core_geometry::ball(Vec3::new(0.0, 0.0, 20.0), 20.0, 64, 96);
    let asked = HollowSettings::for_wall(2.0);
    let shell = hollow(&ball, &Bvh::build(&ball), &asked, Cancel::never()).expect("a ball hollows");
    let search = DrainSearch {
        mesh: &shell.mesh,
        cuts: None,
        size: HoleSize::round(3.0, 3.0, 1.0),
        wall: Some((2.0, shell.voxel_mm)),
        vents: true,
        plate_z: Some(0.0),
        layer_height_mm: 0.05,
    };
    let mut group = c.benchmark_group("drains");
    group.sample_size(10);
    group.bench_function("auto_40mm_ball", |b| {
        b.iter(|| suggest_drains(&search).expect("the shell slices"));
    });
    group.finish();
}

/// Machine layer heights, mm, around the analysis step (the middle one).
const STEPS_MM: [Scalar; 3] = [0.02, 0.05, 0.1];

/// A 20 mm cube hollowed to a 2 mm wall, standing on the plate.
fn hollow_cube() -> (Mesh, std::ops::Range<usize>, Scalar) {
    let cube = core_geometry::cuboid(Vec3::ZERO, Vec3::splat(20.0));
    let asked = HollowSettings::for_wall(2.0);
    let shell = hollow(&cube, &Bvh::build(&cube), &asked, Cancel::never()).expect("a cube hollows");
    (shell.mesh, shell.cavity, shell.voxel_mm)
}

/// Where the shell needs holding up, slicing it at each step first: the cost of the support
/// run's model analysis, by the step it reads at.
fn analysis_needs(group: &mut BenchmarkGroup<'_, WallTime>) {
    let (mesh, cavity, _) = hollow_cube();
    let bvh = Arc::new(Bvh::build(&mesh));
    let plan = SupportPlan::of(SupportProfile::default());
    let lows = low_points(&mesh, Transform::default());
    let inner = inner_faces(&cavity, mesh.faces.len());
    for step in STEPS_MM {
        let settings = SliceSettings {
            layer_height: step,
            ..analysis_slicing()
        };
        group.bench_function(format!("needs_cube_20mm_at_{step}"), |b| {
            b.iter(|| {
                let stack = PlaneSliceEngine
                    .slice(&mesh, &settings)
                    .expect("the shell slices");
                let asked = NeedsAsked {
                    stack: &stack,
                    surface: &mesh,
                    bvh: &bvh,
                    layer_height_mm: step,
                    plan: &plan,
                    lows: &lows,
                    painted: None,
                    shown: None,
                    seeds: &[],
                    side: Some(WallSide {
                        bvh: Arc::clone(&bvh),
                        inner: inner.clone(),
                        into_model: core_geometry::Mat4::IDENTITY,
                        inside: false,
                    }),
                };
                needs_on(asked, &[], &mut |_| true).expect("the shell needs resolve")
            });
        });
    }
}

/// The drain search on the same shell, its scans cut at each step.
fn analysis_drains(group: &mut BenchmarkGroup<'_, WallTime>) {
    let (mesh, _, voxel_mm) = hollow_cube();
    for step in STEPS_MM {
        let search = DrainSearch {
            mesh: &mesh,
            cuts: None,
            size: HoleSize::round(3.0, 3.0, 1.0),
            wall: Some((2.0, voxel_mm)),
            vents: true,
            plate_z: Some(0.0),
            layer_height_mm: step,
        };
        group.bench_function(format!("drains_cube_20mm_at_{step}"), |b| {
            b.iter(|| suggest_drains_heard(&search, |_| true).expect("the shell slices"));
        });
    }
}

fn analysis(c: &mut Criterion) {
    let mut group = c.benchmark_group("analysis");
    group.sample_size(10);
    analysis_needs(&mut group);
    analysis_drains(&mut group);
    group.finish();
}

criterion_group!(benches, plates, stages, drains, analysis);
criterion_main!(benches);
