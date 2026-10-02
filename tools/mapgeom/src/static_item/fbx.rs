//! An ASCII FBX 7.4 writer for the NadeoImporter route to a 3D car skin
//! (2026-09-12): one mesh, materials named the community way
//! (`DetailsDmgNormal_Details`, `SkinDmg_Skin`, …), a one-bone armature
//! (`Body`) every vertex is bound to with weight 1 — "no bones, everything
//! bound to the body". Paired with a `MeshParams.xml` of `MeshType="Vehicle"`
//! (undocumented; the shape bmx22c's SkinMaker generates), it is what
//! `NadeoImporter Mesh` turns into a `.Mesh.gbx` that `skinfix` then rewrites
//! into a `MainBody.Mesh.Gbx`.
//!
//! Units: metres (GlobalSettings UnitScaleFactor 100), Y up, +Z front — the
//! car's own frame. Kept minimal on purpose: the FBX SDK the importer links
//! (libfbxsdk.dll) reads this dialect; anything it refuses shows up in
//! `NadeoImporterLog.txt`, which is the loop.

use super::skin::Part;

fn f(x: f32) -> String {
    let s = format!("{}", x as f64);
    if s.contains('.') || s.contains('e') { s } else { format!("{s}.0") }
}

fn array_f(vals: &[f32]) -> String {
    let body: Vec<String> = vals.iter().map(|v| f(*v)).collect();
    format!("*{} {{\n\t\t\ta: {}\n\t\t}}", vals.len(), body.join(","))
}

fn array_i(vals: &[i64]) -> String {
    let body: Vec<String> = vals.iter().map(|v| v.to_string()).collect();
    format!("*{} {{\n\t\t\ta: {}\n\t\t}}", vals.len(), body.join(","))
}

const IDENTITY16: [f32; 16] = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0];

/// The FBX text. `mesh_name` is the object's name; materials are the parts'
/// `texset` names mapped through `material_name` (already the community form,
/// e.g. `DetailsDmgNormal_Details`).
pub fn fbx(parts: &[Part], material_name: &dyn Fn(&str) -> String, mesh_name: &str) -> String {
    // merge every part into one geometry, one material index per part
    let mut mats: Vec<String> = Vec::new();
    let mut verts: Vec<f32> = Vec::new();
    let mut poly: Vec<i64> = Vec::new();
    let mut normals: Vec<f32> = Vec::new();
    let mut uvs: Vec<f32> = Vec::new();
    let mut uv_idx: Vec<i64> = Vec::new();
    let mut poly_mat: Vec<i64> = Vec::new();
    let mut base: i64 = 0;
    for p in parts {
        let mname = material_name(&p.texset);
        let mi = match mats.iter().position(|m| *m == mname) {
            Some(i) => i,
            None => {
                mats.push(mname);
                mats.len() - 1
            }
        };
        for v in &p.pos {
            verts.extend_from_slice(v);
        }
        for uv in &p.uv {
            // FBX v runs upward; DDS/TM v runs downward
            uvs.push(uv[0]);
            uvs.push(1.0 - uv[1]);
        }
        for tri in p.idx.chunks(3) {
            if tri.len() < 3 {
                continue;
            }
            let (a, b, c) = (tri[0] as i64 + base, tri[1] as i64 + base, tri[2] as i64 + base);
            poly.push(a);
            poly.push(b);
            poly.push(-(c + 1));
            for &i in tri {
                let n = p.nrm.get(i as usize).copied().unwrap_or([0.0, 1.0, 0.0]);
                normals.extend_from_slice(&n);
                uv_idx.push(i as i64 + base);
            }
            poly_mat.push(mi as i64);
        }
        base += p.pos.len() as i64;
    }
    let nverts = (verts.len() / 3) as i64;
    let mut s = String::new();
    s.push_str("; FBX 7.4.0 project file\n; Created by mapgeom skin-fbx\n\n");
    s.push_str("FBXHeaderExtension:  {\n\tFBXHeaderVersion: 1003\n\tFBXVersion: 7400\n\tCreationTimeStamp:  {\n\t\tVersion: 1000\n\t\tYear: 2026\n\t\tMonth: 9\n\t\tDay: 12\n\t\tHour: 12\n\t\tMinute: 0\n\t\tSecond: 0\n\t\tMillisecond: 0\n\t}\n\tCreator: \"mapgeom skin-fbx\"\n}\n");
    s.push_str("GlobalSettings:  {\n\tVersion: 1000\n\tProperties70:  {\n\t\tP: \"UpAxis\", \"int\", \"Integer\", \"\",1\n\t\tP: \"UpAxisSign\", \"int\", \"Integer\", \"\",1\n\t\tP: \"FrontAxis\", \"int\", \"Integer\", \"\",2\n\t\tP: \"FrontAxisSign\", \"int\", \"Integer\", \"\",1\n\t\tP: \"CoordAxis\", \"int\", \"Integer\", \"\",0\n\t\tP: \"CoordAxisSign\", \"int\", \"Integer\", \"\",1\n\t\tP: \"OriginalUpAxis\", \"int\", \"Integer\", \"\",1\n\t\tP: \"OriginalUpAxisSign\", \"int\", \"Integer\", \"\",1\n\t\tP: \"UnitScaleFactor\", \"double\", \"Number\", \"\",100\n\t\tP: \"OriginalUnitScaleFactor\", \"double\", \"Number\", \"\",100\n\t\tP: \"AmbientColor\", \"ColorRGB\", \"Color\", \"\",0,0,0\n\t\tP: \"DefaultCamera\", \"KString\", \"\", \"\", \"Producer Perspective\"\n\t\tP: \"TimeMode\", \"enum\", \"\", \"\",11\n\t\tP: \"TimeSpanStart\", \"KTime\", \"Time\", \"\",0\n\t\tP: \"TimeSpanStop\", \"KTime\", \"Time\", \"\",46186158000\n\t\tP: \"CustomFrameRate\", \"double\", \"Number\", \"\",24\n\t}\n}\n");
    s.push_str("Documents:  {\n\tCount: 1\n\tDocument: 9000, \"\", \"Scene\" {\n\t\tProperties70:  {\n\t\t\tP: \"SourceObject\", \"object\", \"\", \"\"\n\t\t\tP: \"ActiveAnimStackName\", \"KString\", \"\", \"\", \"\"\n\t\t}\n\t\tRootNode: 0\n\t}\n}\n");
    s.push_str("References:  {\n}\n");
    let n_models = 3; // mesh, armature, bone
    s.push_str(&format!("Definitions:  {{\n\tVersion: 100\n\tCount: {}\n\tObjectType: \"GlobalSettings\" {{\n\t\tCount: 1\n\t}}\n\tObjectType: \"Model\" {{\n\t\tCount: {n_models}\n\t}}\n\tObjectType: \"Geometry\" {{\n\t\tCount: 1\n\t}}\n\tObjectType: \"Material\" {{\n\t\tCount: {}\n\t}}\n\tObjectType: \"Deformer\" {{\n\t\tCount: 2\n\t}}\n\tObjectType: \"Pose\" {{\n\t\tCount: 1\n\t}}\n\tObjectType: \"NodeAttribute\" {{\n\t\tCount: 2\n\t}}\n}}\n", 1 + n_models + 1 + mats.len() + 2 + 1 + 2, mats.len()));
    s.push_str("Objects:  {\n");
    // geometry
    s.push_str(&format!("\tGeometry: 1000, \"Geometry::{mesh_name}\", \"Mesh\" {{\n"));
    s.push_str(&format!("\t\tVertices: {}\n", array_f(&verts)));
    s.push_str(&format!("\t\tPolygonVertexIndex: {}\n", array_i(&poly)));
    s.push_str("\t\tGeometryVersion: 124\n");
    s.push_str(&format!("\t\tLayerElementNormal: 0 {{\n\t\t\tVersion: 101\n\t\t\tName: \"\"\n\t\t\tMappingInformationType: \"ByPolygonVertex\"\n\t\t\tReferenceInformationType: \"Direct\"\n\t\t\tNormals: {}\n\t\t}}\n", array_f(&normals)));
    s.push_str(&format!("\t\tLayerElementUV: 0 {{\n\t\t\tVersion: 101\n\t\t\tName: \"BaseMaterial\"\n\t\t\tMappingInformationType: \"ByPolygonVertex\"\n\t\t\tReferenceInformationType: \"IndexToDirect\"\n\t\t\tUV: {}\n\t\t\tUVIndex: {}\n\t\t}}\n", array_f(&uvs), array_i(&uv_idx)));
    s.push_str(&format!("\t\tLayerElementMaterial: 0 {{\n\t\t\tVersion: 101\n\t\t\tName: \"\"\n\t\t\tMappingInformationType: \"ByPolygon\"\n\t\t\tReferenceInformationType: \"IndexToDirect\"\n\t\t\tMaterials: {}\n\t\t}}\n", array_i(&poly_mat)));
    s.push_str("\t\tLayer: 0 {\n\t\t\tVersion: 100\n\t\t\tLayerElement:  {\n\t\t\t\tType: \"LayerElementNormal\"\n\t\t\t\tTypedIndex: 0\n\t\t\t}\n\t\t\tLayerElement:  {\n\t\t\t\tType: \"LayerElementMaterial\"\n\t\t\t\tTypedIndex: 0\n\t\t\t}\n\t\t\tLayerElement:  {\n\t\t\t\tType: \"LayerElementUV\"\n\t\t\t\tTypedIndex: 0\n\t\t\t}\n\t\t}\n\t}\n");
    // models
    s.push_str(&format!("\tModel: 2000, \"Model::{mesh_name}\", \"Mesh\" {{\n\t\tVersion: 232\n\t\tProperties70:  {{\n\t\t\tP: \"Lcl Translation\", \"Lcl Translation\", \"\", \"A\",0,0,0\n\t\t\tP: \"Lcl Rotation\", \"Lcl Rotation\", \"\", \"A\",0,0,0\n\t\t\tP: \"Lcl Scaling\", \"Lcl Scaling\", \"\", \"A\",1,1,1\n\t\t\tP: \"DefaultAttributeIndex\", \"int\", \"Integer\", \"\",0\n\t\t\tP: \"InheritType\", \"enum\", \"\", \"\",1\n\t\t}}\n\t\tShading: T\n\t\tCulling: \"CullingOff\"\n\t}}\n"));
    s.push_str("\tModel: 3000, \"Model::Armature\", \"Null\" {\n\t\tVersion: 232\n\t\tProperties70:  {\n\t\t\tP: \"Lcl Translation\", \"Lcl Translation\", \"\", \"A\",0,0,0\n\t\t\tP: \"Lcl Rotation\", \"Lcl Rotation\", \"\", \"A\",0,0,0\n\t\t\tP: \"Lcl Scaling\", \"Lcl Scaling\", \"\", \"A\",1,1,1\n\t\t\tP: \"DefaultAttributeIndex\", \"int\", \"Integer\", \"\",0\n\t\t\tP: \"InheritType\", \"enum\", \"\", \"\",1\n\t\t}\n\t\tShading: Y\n\t\tCulling: \"CullingOff\"\n\t}\n");
    s.push_str("\tNodeAttribute: 3001, \"NodeAttribute::Armature\", \"Null\" {\n\t\tTypeFlags: \"Null\"\n\t}\n");
    s.push_str("\tModel: 3100, \"Model::Body\", \"LimbNode\" {\n\t\tVersion: 232\n\t\tProperties70:  {\n\t\t\tP: \"Lcl Translation\", \"Lcl Translation\", \"\", \"A\",0,0,0\n\t\t\tP: \"Lcl Rotation\", \"Lcl Rotation\", \"\", \"A\",0,0,0\n\t\t\tP: \"Lcl Scaling\", \"Lcl Scaling\", \"\", \"A\",1,1,1\n\t\t\tP: \"DefaultAttributeIndex\", \"int\", \"Integer\", \"\",0\n\t\t\tP: \"InheritType\", \"enum\", \"\", \"\",1\n\t\t}\n\t\tShading: Y\n\t\tCulling: \"CullingOff\"\n\t}\n");
    s.push_str("\tNodeAttribute: 3101, \"NodeAttribute::Body\", \"LimbNode\" {\n\t\tProperties70:  {\n\t\t\tP: \"Size\", \"double\", \"Number\", \"\",1\n\t\t}\n\t\tTypeFlags: \"Skeleton\"\n\t}\n");
    // materials
    for (i, m) in mats.iter().enumerate() {
        s.push_str(&format!("\tMaterial: {}, \"Material::{m}\", \"\" {{\n\t\tVersion: 102\n\t\tShadingModel: \"phong\"\n\t\tMultiLayer: 0\n\t\tProperties70:  {{\n\t\t\tP: \"DiffuseColor\", \"Color\", \"\", \"A\",0.8,0.8,0.8\n\t\t\tP: \"Diffuse\", \"Vector3D\", \"Vector\", \"\",0.8,0.8,0.8\n\t\t\tP: \"Emissive\", \"Vector3D\", \"Vector\", \"\",0,0,0\n\t\t\tP: \"Ambient\", \"Vector3D\", \"Vector\", \"\",0,0,0\n\t\t\tP: \"Specular\", \"Vector3D\", \"Vector\", \"\",0.2,0.2,0.2\n\t\t\tP: \"Shininess\", \"double\", \"Number\", \"\",20\n\t\t\tP: \"Opacity\", \"double\", \"Number\", \"\",1\n\t\t}}\n\t}}\n", 4000 + i));
    }
    // skin: every vertex to Body, weight 1
    let idx: Vec<i64> = (0..nverts).collect();
    let w: Vec<f32> = vec![1.0; nverts as usize];
    s.push_str("\tDeformer: 5000, \"Deformer::Armature\", \"Skin\" {\n\t\tVersion: 101\n\t\tLink_DeformAcuracy: 50\n\t\tSkinningType: \"Linear\"\n\t}\n");
    s.push_str(&format!("\tDeformer: 5100, \"SubDeformer::Body\", \"Cluster\" {{\n\t\tVersion: 100\n\t\tUserData: \"\", \"\"\n\t\tIndexes: {}\n\t\tWeights: {}\n\t\tTransform: {}\n\t\tTransformLink: {}\n\t}}\n", array_i(&idx), array_f(&w), array_f(&IDENTITY16), array_f(&IDENTITY16)));
    s.push_str(&format!("\tPose: 6000, \"Pose::BindPose\", \"BindPose\" {{\n\t\tType: \"BindPose\"\n\t\tVersion: 100\n\t\tNbPoseNodes: 3\n\t\tPoseNode:  {{\n\t\t\tNode: 2000\n\t\t\tMatrix: {}\n\t\t}}\n\t\tPoseNode:  {{\n\t\t\tNode: 3000\n\t\t\tMatrix: {}\n\t\t}}\n\t\tPoseNode:  {{\n\t\t\tNode: 3100\n\t\t\tMatrix: {}\n\t\t}}\n\t}}\n", array_f(&IDENTITY16), array_f(&IDENTITY16), array_f(&IDENTITY16)));
    s.push_str("}\n");
    // connections
    s.push_str("Connections:  {\n");
    s.push_str("\tC: \"OO\",2000,0\n\tC: \"OO\",3000,0\n\tC: \"OO\",3100,3000\n\tC: \"OO\",3001,3000\n\tC: \"OO\",3101,3100\n\tC: \"OO\",1000,2000\n");
    for i in 0..mats.len() {
        s.push_str(&format!("\tC: \"OO\",{},2000\n", 4000 + i));
    }
    s.push_str("\tC: \"OO\",5000,1000\n\tC: \"OO\",5100,5000\n\tC: \"OO\",3100,5100\n");
    s.push_str("}\n");
    s.push_str("Takes:  {\n\tCurrent: \"\"\n}\n");
    s
}

/// The `MeshParams.xml` SkinMaker generates for a vehicle: MeshType Vehicle,
/// one Material per FBX material with its shading Model = the name's prefix.
pub fn mesh_params(fbx_file: &str, material_names: &[String]) -> String {
    let mut s = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
    s.push_str(&format!("<MeshParams MeshType=\"Vehicle\" SkelSocketPrefix=\"_\" FbxFile=\"{fbx_file}\">\n  <Materials>\n"));
    for m in material_names {
        let model = m.split('_').next().unwrap_or("DetailsDmgNormal");
        s.push_str(&format!("    <Material Name=\"{m}\" Model=\"{model}\" />\n"));
    }
    s.push_str("  </Materials>\n  <Constants />\n  <UvAnims />\n  <VisibleIds />\n  <Color />\n</MeshParams>\n");
    s
}
