//! Copy solid2 filetime + prelight u02/u04 from SRC into DST item. Usage:
//! mutprelight DST.Item.Gbx SRC.Item.Gbx OUT.Item.Gbx
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let dst = std::fs::read(&a[1]).unwrap();
    let src = std::fs::read(&a[2]).unwrap();
    let mut f = mapgeom::static_item::file::parse_file(&dst).unwrap();
    let g = mapgeom::static_item::file::parse_file(&src).unwrap();
    let (ft, pu02, pu04) = {
        let gs = g.item.static_object().unwrap().solid2().unwrap();
        (gs.file_write_time, gs.pre_light_gen.as_ref().map(|p| p.u02), gs.pre_light_gen.as_ref().map(|p| p.u04))
    };
    let mc = f.item.model_mut().unwrap();
    if let Some(n) = mc.entity_model.inline.as_deref_mut() {
        if let mapgeom::static_item::Node::EntityModel(em) = n {
            if let Some(sn) = em.static_object.inline.as_deref_mut() {
                if let mapgeom::static_item::Node::StaticObject(so) = sn {
                    if let Some(snn) = so.mesh.inline.as_deref_mut() {
                        if let mapgeom::static_item::Node::Solid2(s) = snn {
                            s.file_write_time = ft;
                            if let Some(p) = s.pre_light_gen.as_mut() {
                                if let Some(u) = pu02 {
                                    p.u02 = u;
                                }
                                if let Some(u) = pu04 {
                                    p.u04 = u;
                                }
                            }
                            println!("transplanted ft={ft} u02={pu02:?}");
                        }
                    }
                }
            }
        }
    }
    std::fs::write(&a[3], mapgeom::static_item::file::write_file(&f)).unwrap();
    println!("wrote {}", &a[3]);
}
