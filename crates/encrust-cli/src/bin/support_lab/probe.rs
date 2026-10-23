//! TEMP diagnostics.
use core_geometry::{Bvh, Ray, Transform, Vec3, raycast_placed, transform_mesh};
use core_supports::{Placed, Profiles, SupportPoint, columns, grow};
use printer_profiles::SupportProfile;

fn inside(m: &core_geometry::Mesh, bvh: &Bvh, p: Vec3) -> bool {
    let dirs = [
        Vec3::new(0.0123, 0.0071, 1.0),
        Vec3::new(1.0, 0.013, 0.021),
        Vec3::new(-0.017, 1.0, 0.011),
        Vec3::new(0.0123, 0.0071, -1.0),
        Vec3::new(-1.0, 0.013, 0.021),
    ];
    dirs.iter()
        .filter(|d| inside_along(m, bvh, p, d.normalize()))
        .count()
        >= 3
}

pub fn votes(m: &core_geometry::Mesh, bvh: &Bvh, p: Vec3) -> usize {
    let dirs = [
        Vec3::new(0.0123, 0.0071, 1.0),
        Vec3::new(1.0, 0.013, 0.021),
        Vec3::new(-0.017, 1.0, 0.011),
        Vec3::new(0.0123, 0.0071, -1.0),
        Vec3::new(-1.0, 0.013, 0.021),
    ];
    dirs.iter()
        .filter(|d| inside_along(m, bvh, p, d.normalize()))
        .count()
}

fn inside_along(m: &core_geometry::Mesh, bvh: &Bvh, p: Vec3, dir: Vec3) -> bool {
    let ray = Ray::new(p, dir);
    let mut n = 0;
    let mut from = p;
    for _ in 0..64 {
        let r = Ray::new(from, ray.direction);
        let Some(h) = raycast_placed(m, bvh, Transform::default(), &r) else {
            break;
        };
        n += 1;
        from = h.point + ray.direction * 1e-4;
    }
    n % 2 == 1
}

pub fn project_probe(path: &str) {
    let project = core_engine::project::load(std::path::Path::new(path)).unwrap();
    let object = &project.manifest.objects[0];
    let model = transform_mesh(&project.models[0].source, object.transform);
    let table: Vec<SupportProfile> = project
        .manifest
        .supports
        .groups
        .iter()
        .map(|g| g.profile.clone())
        .collect();
    let matrix = object.transform.to_matrix();
    let mut points: Vec<SupportPoint> = object
        .supports
        .points
        .iter()
        .map(|p| SupportPoint {
            contact: matrix.transform_point3(p.contact),
            group: p.group,
        })
        .collect();
    for t in &object.supports.frozen {
        for c in t.contacts() {
            points.push(SupportPoint {
                contact: matrix.transform_point3(c),
                group: t.group(),
            });
        }
    }
    let bvh = Bvh::build(&model);
    let placed = Placed::new(&model, &bvh, Transform::default());
    let profiles = Profiles::new(&table).unwrap();
    let cols = columns(&points, &placed, profiles);
    let trees = grow(&cols, &placed, profiles);
    for (ti, t) in trees.iter().enumerate() {
        let l = t.landing();
        // Every strut, root to landing included.
        let mut segs: Vec<(Vec3, Vec3)> = t
            .nodes()
            .iter()
            .filter_map(|n| n.parent.map(|p| (n.position, t.nodes()[p].position)))
            .collect();
        segs.push((t.root().position, l.base));
        let mut worst: Option<(Vec3, usize)> = None;
        for (a, b) in &segs {
            let len = (*b - *a).length();
            let steps = (len / 0.05).ceil() as usize;
            for i in 1..steps.saturating_sub(1) {
                let p = a.lerp(*b, i as f32 / steps as f32);
                // Skip within 0.4 mm of either end (tips and landings bite).
                if (p - *a).length() < 0.4 || (p - *b).length() < 0.4 {
                    continue;
                }
                if inside(&model, &bvh, p) {
                    worst = Some((p, worst.map_or(1, |w| w.1 + 1)));
                }
            }
        }
        if let Some((p, n)) = worst {
            println!(
                "TREE {ti} tips {} on_model {} landing {:.3?} inside samples {n} e.g. {p:.3?}",
                t.tip_count(),
                l.on_model,
                l.base
            );
            for (ni, nd) in t.nodes().iter().enumerate() {
                println!(
                    "   node {ni} {:.3?} r {:.2} parent {:?}",
                    nd.position, nd.radius_mm, nd.parent
                );
            }
        }
    }
    for (ti, t) in trees.iter().enumerate() {
        let l = t.landing();
        if l.on_model {
            println!(
                "ONMODEL {ti} tips {} on_support {} base {:.3?} root {:.3?} r {:.2}",
                t.tip_count(),
                l.on_support,
                l.base,
                t.root().position,
                t.root().radius_mm
            );
            for (ni, nd) in t.nodes().iter().enumerate() {
                println!(
                    "   node {ni} {:.3?} r {:.2} parent {:?}",
                    nd.position, nd.radius_mm, nd.parent
                );
            }
        }
    }
    let (a, b) = (
        Vec3::new(106.342, 71.062, 20.567),
        Vec3::new(106.342, 67.729, 23.900),
    );
    for i in 0..=40 {
        let p = a.lerp(b, i as f32 / 40.0);
        println!("ARM {p:.3?} votes {}", votes(&model, &bvh, p));
    }
    let (a, b) = (
        Vec3::new(106.342, 67.729, 23.900),
        Vec3::new(105.950, 67.150, 25.774),
    );
    for i in 0..=20 {
        let p = a.lerp(b, i as f32 / 20.0);
        println!("TOP {p:.3?} votes {}", votes(&model, &bvh, p));
    }
    let off = object.transform.translation;
    let (a, b) = (
        Vec3::new(-3.4517212, 7.3962135, 19.775696) + off,
        Vec3::new(-3.4517212, 7.3962135, 24.475368) + off,
    );
    for i in 0..=47 {
        let p = a.lerp(b, i as f32 / 47.0);
        println!("FROZEN {p:.3?} votes {}", votes(&model, &bvh, p));
    }
    let profile = &table[0];
    for neck in [
        Vec3::new(111.186, 59.394, 22.457),
        Vec3::new(110.218, 59.442, 22.534),
    ] {
        for r in [0.6f32, 0.12] {
            println!(
                "NECK {neck:.3?} r {r} landing {:?}",
                core_supports::landing(&placed, neck, r, 1, profile)
            );
        }
    }
    for from in [
        Vec3::new(110.150, 59.350, 24.167),
        Vec3::new(111.186, 59.394, 22.457),
    ] {
        let ray = Ray::new(from - Vec3::Z * 1e-3, -Vec3::Z);
        let h = raycast_placed(&model, &bvh, Transform::default(), &ray);
        println!(
            "DOWN {from:.3?} hit {:?}",
            h.map(|h| (
                h.distance,
                h.normal,
                votes(&model, &bvh, h.point - Vec3::Z * 0.05)
            ))
        );
        println!(
            "  r0.24 landing {:?}",
            core_supports::landing(&placed, from, 0.12, 1, profile).is_some()
        );
    }
    println!("trees {}", trees.len());
}
