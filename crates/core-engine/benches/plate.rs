#![expect(
    clippy::expect_used,
    reason = "a broken fixture must fail the run loudly"
)]

use std::io::Cursor;
use std::sync::Arc;

use core_analysis::{Cured, Measured, cure};
use core_engine::{Cutting, Model, Plate, Run};
use core_format::ExposurePlan;
use core_geometry::{Mesh, Scalar, Transform, Vec3};
use core_pipeline::{PanelOverrides, SlicedFormat};
use core_raster::{LayerRuns, RasterSettings, Rasterizer, ScanlineRasterizer};
use core_slicer::{Layer, SliceError};
use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use printer_profiles::{Catalogue, MaterialProfile};
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

criterion_group!(benches, plates, stages);
criterion_main!(benches);
