//! GLB-to-construction adapter. No GPU state enters the geometry/document model.
use crate::asset;
use glam::{DMat3, DMat4, DVec3};
use qx_geometry::{
    GeometryError, Result,
    construction::{Bounds, ConstructionAssets, Mesh, MeshVertex},
    math::Vec3,
};
use qx_style::construction::Profile;
use std::{
    cell::RefCell,
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
};

type Key = (String, [u64; 3]);
/// Bounded worker-owned cache. Reload is explicit; material edits need no import.
pub struct Sources {
    root: PathBuf,
    meshes: RefCell<BTreeMap<Key, Arc<Mesh>>>,
    sections: RefCell<BTreeMap<Key, Profile>>,
}
impl Sources {
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.into(),
            meshes: Default::default(),
            sections: Default::default(),
        }
    }
    pub fn clear(&self) {
        self.meshes.borrow_mut().clear();
        self.sections.borrow_mut().clear();
    }
    fn mesh(&self, path: &str, scale: [f64; 3]) -> Result<Arc<Mesh>> {
        let key = (path.to_owned(), scale.map(f64::to_bits));
        if let Some(mesh) = self.meshes.borrow().get(&key) {
            return Ok(mesh.clone());
        }
        if !scale
            .iter()
            .all(|v| v.is_finite() && *v >= 1e-5 && *v <= 1000.)
        {
            return Err(GeometryError("Invalid construction source scale".into()));
        }
        let import = || -> std::result::Result<Mesh, String> {
            let file =
                qx_style::library::resolve_existing(&self.root, path).map_err(|e| e.to_string())?;
            let model = asset::import_glb(&asset::read_glb(&file)?)?;
            // One component has one explicit appearance slot. Do not collapse
            // differently assigned primitive materials into one unnamed finish.
            if model.primitives.len() != 1 {
                return Err("Construction source must have one material primitive; split components explicitly".into());
            }
            let mut vertices = vec![];
            let mut indices = vec![];
            for node in &model.nodes {
                let primitive = &model.primitives[node.primitive];
                if vertices.len() + primitive.vertices.len() > 16384
                    || indices.len() + primitive.indices.len() > 65536
                {
                    return Err("Construction source exceeds 16384 vertices / 65536 indices".into());
                }
                let transform = DMat4::from_scale(DVec3::from_array(scale))
                    * DMat4::from_cols_array_2d(&node.transform);
                let normal_matrix = DMat3::from_mat4(transform).inverse().transpose();
                let convert = |p: DVec3| Vec3::new(p.z, p.y, -p.x);
                let base = vertices.len() as u32;
                for v in &primitive.vertices {
                    let p =
                        transform.transform_point3(DVec3::from_array(v.position.map(f64::from)));
                    let n =
                        (normal_matrix * DVec3::from_array(v.normal.map(f64::from))).normalize();
                    vertices.push(MeshVertex {
                        position_m: convert(p),
                        normal: convert(n),
                        uv: v.uv.map(f64::from),
                        station_m: 0.,
                        source_station_m: 0.,
                    });
                }
                for t in primitive.indices.as_chunks::<3>().0 {
                    let t = if transform.determinant() < 0. {
                        [t[2], t[1], t[0]]
                    } else {
                        [t[0], t[1], t[2]]
                    };
                    indices.extend(t.map(|i| base + i));
                }
            }
            let mut bounds = Bounds::point(
                vertices
                    .first()
                    .ok_or("Empty construction source")?
                    .position_m,
            );
            for v in &vertices {
                bounds.include(v.position_m);
            }
            let mesh = Mesh {
                vertices,
                indices,
                bounds,
            };
            mesh.validate().map_err(|e| e.to_string())?;
            Ok(mesh)
        };
        let mesh = Arc::new(
            import().map_err(|e| GeometryError(format!("Construction source {path}: {e}")))?,
        );
        if self.meshes.borrow().len() >= 64 {
            self.clear();
        }
        self.meshes.borrow_mut().insert(key, mesh.clone());
        Ok(mesh)
    }
    /// Exact transformed source for comparison/evidence, including UVs/normals.
    pub fn original(&self, path: &str, scale: [f64; 3]) -> Result<Arc<Mesh>> {
        self.mesh(path, scale)
    }
}
impl ConstructionAssets for Sources {
    fn section(&self, path: &str, scale: [f64; 3]) -> Result<Profile> {
        let key = (path.to_owned(), scale.map(f64::to_bits));
        if let Some(profile) = self.sections.borrow().get(&key) {
            return Ok(profile.clone());
        }
        let mesh = self.mesh(path, scale)?;
        let profile = qx_geometry::source_profile::extract(&mesh)
            .map_err(|e| GeometryError(format!("Cannot sweep {path}: {e}")))?;
        self.sections.borrow_mut().insert(key, profile.clone());
        Ok(profile)
    }
    fn hardware(&self, path: &str, scale: [f64; 3]) -> Result<Mesh> {
        Ok((*self.mesh(path, scale)?).clone())
    }
}
