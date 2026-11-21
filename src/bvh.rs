//! CPU-side BVH for triangle meshes, flattened for the GPU traversal in `bricius_render.wgsl`.
//!
//! Layout of one GPU node (`GpuBvhNode`, 2 x vec4 = 32 bytes, storage binding 7):
//!   `min_first`: xyz = AABB min, w = u32 bits (as f32):
//!                  interior -> index of the RIGHT child (global node index); the left child is `self + 1`
//!                  leaf     -> index of the first triangle (global index into the binding-4 triangle buffer)
//!   `max_count`: xyz = AABB max, w = u32 bits (as f32): triangle count (0 = interior node, 1..=LEAF_SIZE = leaf)
//! Nodes are laid out depth-first, so the left child directly follows its parent. Triangles of a leaf are
//! contiguous in the (reordered) triangle buffer. All indices are global across meshes: a mesh's primitive
//! stores its root node index in `GpuPrimitive.extra.y` (and its triangle count in `extra.z`).

use glam::Vec3;

use crate::model_loader::LoadedMesh;
use crate::scene::{GpuBvhNode, GpuTriangle};

pub const LEAF_SIZE: usize = 4;
/// Beyond this depth we always split at the median so the tree depth stays bounded (GPU stack is 64 deep).
const SAH_MAX_DEPTH: usize = 16;
const BINS: usize = 12;

#[derive(Clone, Copy, Debug)]
struct Aabb {
    min: Vec3,
    max: Vec3,
}

impl Aabb {
    const EMPTY: Aabb = Aabb { min: Vec3::splat(f32::MAX), max: Vec3::splat(f32::MIN) };
    fn grow(&mut self, p: Vec3) {
        self.min = self.min.min(p);
        self.max = self.max.max(p);
    }
    fn merge(&mut self, o: &Aabb) {
        self.min = self.min.min(o.min);
        self.max = self.max.max(o.max);
    }
    fn area(&self) -> f32 {
        let e = (self.max - self.min).max(Vec3::ZERO);
        2.0 * (e.x * e.y + e.y * e.z + e.z * e.x)
    }
}

/// Local (per-mesh) BVH node: indices are relative to the mesh's own node / triangle arrays.
#[derive(Clone, Copy, Debug)]
pub struct BvhNode {
    pub min: Vec3,
    pub max: Vec3,
    /// interior: right child index; leaf: first triangle (in the reordered triangle list)
    pub first_or_right: u32,
    /// 0 for interior nodes
    pub count: u32,
}

pub struct MeshBvh {
    pub nodes: Vec<BvhNode>,
    /// `order[i]` = index of the original triangle stored at reordered position `i`
    pub order: Vec<u32>,
}

struct Builder<'a> {
    tris: &'a [[Vec3; 3]],
    bounds: Vec<Aabb>,
    centroids: Vec<Vec3>,
    order: Vec<u32>,
    nodes: Vec<BvhNode>,
}

pub fn build(tris: &[[Vec3; 3]]) -> MeshBvh {
    let n = tris.len();
    let mut b = Builder {
        tris,
        bounds: tris
            .iter()
            .map(|t| {
                let mut a = Aabb::EMPTY;
                for p in t {
                    a.grow(*p);
                }
                a
            })
            .collect(),
        centroids: tris.iter().map(|t| (t[0] + t[1] + t[2]) / 3.0).collect(),
        order: (0..n as u32).collect(),
        nodes: Vec::with_capacity(2 * n / LEAF_SIZE + 2),
    };
    if n > 0 {
        b.build_node(0, n, 0);
    }
    let _ = b.tris;
    MeshBvh { nodes: b.nodes, order: b.order }
}

impl Builder<'_> {
    fn build_node(&mut self, start: usize, end: usize, depth: usize) -> u32 {
        let idx = self.nodes.len() as u32;
        let mut bb = Aabb::EMPTY;
        let mut cb = Aabb::EMPTY;
        for &t in &self.order[start..end] {
            bb.merge(&self.bounds[t as usize]);
            cb.grow(self.centroids[t as usize]);
        }
        self.nodes.push(BvhNode { min: bb.min, max: bb.max, first_or_right: start as u32, count: (end - start) as u32 });
        let count = end - start;
        if count <= LEAF_SIZE {
            return idx;
        }

        let ext = cb.max - cb.min;
        let axis = if ext.x >= ext.y && ext.x >= ext.z { 0 } else if ext.y >= ext.z { 1 } else { 2 };
        let mut mid = start + count / 2;
        let mut use_median = depth >= SAH_MAX_DEPTH || ext[axis] <= 1e-12;

        if !use_median {
            // Binned SAH along the widest centroid axis
            let lo = cb.min[axis];
            let scale = BINS as f32 / ext[axis];
            let mut bin_box = [Aabb::EMPTY; BINS];
            let mut bin_cnt = [0usize; BINS];
            for &t in &self.order[start..end] {
                let k = (((self.centroids[t as usize][axis] - lo) * scale) as usize).min(BINS - 1);
                bin_cnt[k] += 1;
                bin_box[k].merge(&self.bounds[t as usize]);
            }
            let mut left_area = [0.0f32; BINS - 1];
            let mut left_cnt = [0usize; BINS - 1];
            let mut acc = Aabb::EMPTY;
            let mut c = 0;
            for i in 0..BINS - 1 {
                acc.merge(&bin_box[i]);
                c += bin_cnt[i];
                left_area[i] = if c > 0 { acc.area() } else { 0.0 };
                left_cnt[i] = c;
            }
            let mut best = (f32::MAX, 0usize);
            let mut acc = Aabb::EMPTY;
            let mut c = 0;
            for i in (1..BINS).rev() {
                acc.merge(&bin_box[i]);
                c += bin_cnt[i];
                let l = left_cnt[i - 1];
                if l == 0 || c == 0 {
                    continue;
                }
                let cost = left_area[i - 1] * l as f32 + acc.area() * c as f32;
                if cost < best.0 {
                    best = (cost, i - 1);
                }
            }
            if best.0 == f32::MAX {
                use_median = true;
            } else {
                // Partition in place around the chosen bin boundary
                let split = best.1;
                let slice = &mut self.order[start..end];
                let cents = &self.centroids;
                let mut i = 0usize;
                let mut j = slice.len();
                while i < j {
                    let k = (((cents[slice[i] as usize][axis] - lo) * scale) as usize).min(BINS - 1);
                    if k <= split {
                        i += 1;
                    } else {
                        j -= 1;
                        slice.swap(i, j);
                    }
                }
                mid = start + i;
                if mid == start || mid == end {
                    use_median = true;
                    mid = start + count / 2;
                }
            }
        }
        if use_median {
            mid = start + count / 2;
            let cents = &self.centroids;
            self.order[start..end].select_nth_unstable_by(count / 2, |a, b| {
                cents[*a as usize][axis].partial_cmp(&cents[*b as usize][axis]).unwrap_or(std::cmp::Ordering::Equal)
            });
        }

        self.build_node(start, mid, depth + 1); // left child lands at idx + 1
        let right = self.build_node(mid, end, depth + 1);
        let n = &mut self.nodes[idx as usize];
        n.first_or_right = right;
        n.count = 0;
        idx
    }
}

#[cfg_attr(not(test), allow(dead_code))]
/// Moller-Trumbore, returns t (> t_min) of the hit, like the shader.
fn ray_tri(ro: Vec3, rd: Vec3, t: &[Vec3; 3], t_min: f32) -> Option<f32> {
    let e1 = t[1] - t[0];
    let e2 = t[2] - t[0];
    let p = rd.cross(e2);
    let det = e1.dot(p);
    if det.abs() <= 1e-5 {
        return None;
    }
    let inv = 1.0 / det;
    let tv = ro - t[0];
    let u = tv.dot(p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = tv.cross(e1);
    let v = rd.dot(q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let th = e2.dot(q) * inv;
    (th > t_min).then_some(th)
}

impl MeshBvh {
    #[cfg_attr(not(test), allow(dead_code))]
    /// Closest hit distance using BVH traversal (mirrors the shader). `tris` are the ORIGINAL triangles.
    pub fn intersect(&self, tris: &[[Vec3; 3]], ro: Vec3, rd: Vec3, t_min: f32) -> Option<f32> {
        if self.nodes.is_empty() {
            return None;
        }
        let inv = Vec3::new(1.0 / rd.x, 1.0 / rd.y, 1.0 / rd.z);
        let mut best = f32::MAX;
        let mut stack = vec![0u32];
        while let Some(ni) = stack.pop() {
            let n = &self.nodes[ni as usize];
            let t0 = (n.min - ro) * inv;
            let t1 = (n.max - ro) * inv;
            let tn = t0.min(t1).max_element();
            let tf = t0.max(t1).min_element();
            if tf < t_min.max(tn) || tn >= best {
                continue;
            }
            if n.count > 0 {
                for k in n.first_or_right..n.first_or_right + n.count {
                    if let Some(t) = ray_tri(ro, rd, &tris[self.order[k as usize] as usize], t_min) {
                        best = best.min(t);
                    }
                }
            } else {
                stack.push(n.first_or_right);
                stack.push(ni + 1);
            }
        }
        (best < f32::MAX).then_some(best)
    }
}

/// Everything the GPU needs for all meshes.
pub struct PackedMeshes {
    pub triangles: Vec<GpuTriangle>,
    pub nodes: Vec<GpuBvhNode>,
    /// per mesh: (root node index, triangle count)
    pub roots: Vec<(u32, u32)>,
}

fn mesh_triangles(mesh: &LoadedMesh) -> (Vec<[Vec3; 3]>, Vec<Vec3>) {
    let mut tris = Vec::new();
    let mut normals = Vec::new();
    for t in 0..mesh.indices.len() / 3 {
        let i0 = mesh.indices[t * 3] as usize;
        let i1 = mesh.indices[t * 3 + 1] as usize;
        let i2 = mesh.indices[t * 3 + 2] as usize;
        if i0 < mesh.vertices.len() && i1 < mesh.vertices.len() && i2 < mesh.vertices.len() {
            tris.push([Vec3::from(mesh.vertices[i0]), Vec3::from(mesh.vertices[i1]), Vec3::from(mesh.vertices[i2])]);
            normals.push(Vec3::from(if i0 < mesh.normals.len() { mesh.normals[i0] } else { [0.0, 1.0, 0.0] }));
        }
    }
    (tris, normals)
}

pub fn pack_meshes(meshes: &[LoadedMesh]) -> PackedMeshes {
    let mut triangles: Vec<GpuTriangle> = Vec::new();
    let mut nodes: Vec<GpuBvhNode> = Vec::new();
    let mut roots = Vec::new();
    for (mesh_idx, mesh) in meshes.iter().enumerate() {
        let (tris, normals) = mesh_triangles(mesh);
        let bvh = build(&tris);
        let node_base = nodes.len() as u32;
        let tri_base = triangles.len() as u32;
        for &o in &bvh.order {
            let t = &tris[o as usize];
            let n = normals[o as usize];
            triangles.push(GpuTriangle {
                v0: [t[0].x, t[0].y, t[0].z, mesh_idx as f32],
                v1: [t[1].x, t[1].y, t[1].z, 0.0],
                v2: [t[2].x, t[2].y, t[2].z, 0.0],
                normal: [n.x, n.y, n.z, 0.0],
            });
        }
        for n in &bvh.nodes {
            // small padding so rays grazing a box never slip between node and triangle rounding
            let pad = ((n.max - n.min).max_element() * 1e-5).max(1e-6);
            let first = if n.count > 0 { n.first_or_right + tri_base } else { n.first_or_right + node_base };
            nodes.push(GpuBvhNode {
                min_first: [n.min.x - pad, n.min.y - pad, n.min.z - pad, f32::from_bits(first)],
                max_count: [n.max.x + pad, n.max.y + pad, n.max.z + pad, f32::from_bits(n.count)],
            });
        }
        roots.push((node_base, tris.len() as u32));
    }
    if triangles.is_empty() {
        triangles.push(GpuTriangle { v0: [0.0; 4], v1: [0.0; 4], v2: [0.0; 4], normal: [0.0, 1.0, 0.0, 0.0] });
    }
    if nodes.is_empty() {
        nodes.push(GpuBvhNode { min_first: [0.0; 4], max_count: [0.0; 4] }); // count 0 + right 0: never traversed
    }
    PackedMeshes { triangles, nodes, roots }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> f32 {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((self.0 >> 40) as f32) / (1u64 << 24) as f32
        }
        fn v(&mut self) -> Vec3 {
            Vec3::new(self.next() * 2.0 - 1.0, self.next() * 2.0 - 1.0, self.next() * 2.0 - 1.0)
        }
    }

    #[test]
    fn bvh_matches_brute_force() {
        let mut rng = Rng(12345);
        let tris: Vec<[Vec3; 3]> = (0..3000)
            .map(|_| {
                let c = rng.v();
                [c + rng.v() * 0.08, c + rng.v() * 0.08, c + rng.v() * 0.08]
            })
            .collect();
        let bvh = build(&tris);
        // every triangle appears exactly once
        let mut seen = vec![0u8; tris.len()];
        for &o in &bvh.order {
            seen[o as usize] += 1;
        }
        assert!(seen.iter().all(|&s| s == 1));
        assert!(bvh.nodes.iter().all(|n| n.count as usize <= LEAF_SIZE));
        let mut hits = 0;
        for _ in 0..2000 {
            let ro = rng.v() * 3.0;
            let rd = (rng.v() - ro * 0.3).normalize();
            let brute = tris.iter().filter_map(|t| ray_tri(ro, rd, t, 0.05)).fold(None, |a: Option<f32>, t| Some(a.map_or(t, |x| x.min(t))));
            let fast = bvh.intersect(&tris, ro, rd, 0.05);
            match (brute, fast) {
                (None, None) => {}
                (Some(a), Some(b)) => {
                    hits += 1;
                    assert!((a - b).abs() < 1e-4, "{a} vs {b}");
                }
                other => panic!("mismatch {other:?}"),
            }
        }
        assert!(hits > 100, "test rays should hit sometimes ({hits})");
    }

    #[test]
    fn bvh_depth_bounded_on_degenerate_input() {
        // many identical triangles must not blow the depth
        let t = [Vec3::ZERO, Vec3::X, Vec3::Y];
        let tris = vec![t; 100_000];
        let bvh = build(&tris);
        fn depth(b: &MeshBvh, i: usize) -> usize {
            let n = &b.nodes[i];
            if n.count > 0 { 1 } else { 1 + depth(b, i + 1).max(depth(b, n.first_or_right as usize)) }
        }
        assert!(depth(&bvh, 0) <= 40);
    }
}
