//! One 16x16m flat quad static item with a single game material. Usage:
//! quad_item LINK PHYS IDENT OUT
use mapgeom::static_item::bake::{assign_lightmap_uvs, make_visuals, visual_layout, Corner};
use mapgeom::static_item::build::{assemble, BuildOpts, Merged, MergedVisual};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let phys: u8 = a[2].parse().unwrap();
    let mut m = Merged::default();
    let slot = m.material_slot(&a[1], phys);
    let mut tris = Vec::new();
    // quad corners (v1-diagonal fan): (0,0,0),(16,0,0),(16,0,16),(0,0,16) at y=1
    let pts = [[0.0, 1.0, 0.0], [16.0, 1.0, 0.0], [16.0, 1.0, 16.0], [0.0, 1.0, 16.0]];
    let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    // upward fan (v0,v3,v2),(v0,v2,v1): cross products point +y.
    let corner = |i: usize| Corner { pos: pts[i], normal: [0.0, 1.0, 0.0], uv: uvs[i], uv1: uvs[i] };
    tris.push([corner(0), corner(3), corner(2)]);
    tris.push([corner(0), corner(2), corner(1)]);
    assign_lightmap_uvs(&mut tris);
    for v in make_visuals(&tris, visual_layout("Stadium\\Media\\Material\\RoadTech"), "range") {
        m.visuals.push(MergedVisual { visual: v, material: slot });
    }
    // collision: same two tris
    m.add_surface_mesh(&pts, &[mapgeom::static_item::surface::Triangle { indices: [0, 3, 2], material_id: phys, u03: 0, surface_index: 0 }, mapgeom::static_item::surface::Triangle { indices: [0, 2, 1], material_id: phys, u03: 0, surface_index: 0 }], &mapgeom::geom::IDENTITY, 1.0);
    let opts = BuildOpts { ident: a[3].clone(), author: a[3].clone(), scale: 1.0, collection: 26, editors: false };
    let f = assemble(&m, &opts).unwrap();
    std::fs::write(&a[4], mapgeom::static_item::file::write_file(&f)).unwrap();
    println!("wrote {} ({} visuals)", &a[4], f.item.static_object().unwrap().solid2().unwrap().visuals.len());
}
