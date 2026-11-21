//! Snapshot-based undo/redo.
//!
//! Captured: scene config (`SceneController`), the 16 materials + names, the object list, terrain
//! heightmaps/configs and the selected object. Not captured: imported meshes and the camera rig.
//!
//! An undo point is created when an edit "settles" (see `UndoManager::commit_if_changed`): the caller
//! decides when the UI is idle (no mouse button / text field / lab open) and the manager compares a cheap
//! content key with the last committed snapshot. Heightmaps are shared between snapshots through `Arc`
//! and only cloned when their content hash changed.
use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;
use std::sync::Arc;

use crate::material::GpuMaterial;
use crate::objects::SceneObject;
use crate::scene::SceneController;
use crate::terrain::{HeightmapGrid, TerrainConfig, TerrainObjectData};

pub const UNDO_CAP: usize = 64;

/// Generic undo/redo stack (oldest entries dropped beyond `cap`).
pub struct UndoStack<T> {
    undo: Vec<T>,
    redo: Vec<T>,
    cap: usize,
}

impl<T> UndoStack<T> {
    pub fn new(cap: usize) -> Self {
        Self { undo: Vec::new(), redo: Vec::new(), cap: cap.max(1) }
    }

    /// Record `prev` (the state before a new edit). Clears the redo history.
    pub fn push(&mut self, prev: T) {
        self.redo.clear();
        self.undo.push(prev);
        if self.undo.len() > self.cap {
            self.undo.remove(0);
        }
    }

    /// Step back: `current` goes onto the redo stack and the previous state is returned.
    pub fn undo(&mut self, current: T) -> Option<T> {
        let prev = self.undo.pop()?;
        self.redo.push(current);
        Some(prev)
    }

    /// Step forward: `current` goes back onto the undo stack and the redone state is returned.
    pub fn redo(&mut self, current: T) -> Option<T> {
        let next = self.redo.pop()?;
        self.undo.push(current);
        Some(next)
    }

    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }
    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }
}

#[derive(Clone)]
struct TerrainSnap {
    hash: u64,
    heightmap: Arc<HeightmapGrid>,
    config: TerrainConfig,
}

#[derive(Clone)]
pub struct Snapshot {
    scene: SceneController,
    materials: [GpuMaterial; GpuMaterial::NUM_PRESETS],
    material_names: [String; GpuMaterial::NUM_PRESETS],
    objects: Vec<SceneObject>,
    terrains: Vec<TerrainSnap>,
    selected: usize,
}

/// Mutable borrows of everything undo captures and restores.
pub struct UndoWorld<'a> {
    pub scene_ctrl: &'a mut SceneController,
    pub materials: &'a mut [GpuMaterial; GpuMaterial::NUM_PRESETS],
    pub material_names: &'a mut [String; GpuMaterial::NUM_PRESETS],
    pub objects: &'a mut Vec<SceneObject>,
    pub terrains: &'a mut Vec<TerrainObjectData>,
    pub selected: &'a mut usize,
}

fn hash_heightmap(h: &HeightmapGrid) -> u64 {
    // FNV-1a over the f32 bit patterns; only run when a terrain was reported changed
    let mut x: u64 = 0xcbf29ce484222325;
    let mut feed = |v: u32| {
        x ^= v as u64;
        x = x.wrapping_mul(0x100000001b3);
    };
    feed(h.resolution as u32);
    feed(h.unit.to_bits());
    for v in &h.data {
        feed(v.to_bits());
    }
    x
}

fn json_bytes<T: serde::Serialize>(h: &mut DefaultHasher, v: &T) {
    h.write(&serde_json::to_vec(v).unwrap_or_default());
}

pub struct UndoManager {
    stack: UndoStack<Snapshot>,
    committed: Option<Snapshot>,
    committed_key: u64,
    /// Content hash per terrain heightmap (the "generation" of each heightmap)
    terrain_hashes: Vec<u64>,
    hashes_stale: bool,
}

impl Default for UndoManager {
    fn default() -> Self {
        Self::new()
    }
}

impl UndoManager {
    pub fn new() -> Self {
        Self { stack: UndoStack::new(UNDO_CAP), committed: None, committed_key: 0, terrain_hashes: Vec::new(), hashes_stale: true }
    }

    pub fn undo_steps(&self) -> usize {
        self.stack.undo_len()
    }
    pub fn redo_steps(&self) -> usize {
        self.stack.redo_len()
    }

    /// Call whenever any heightmap may have been modified (alongside `terrain_dirty`).
    pub fn mark_terrains_changed(&mut self) {
        self.hashes_stale = true;
    }

    fn refresh_hashes(&mut self, terrains: &[TerrainObjectData]) {
        if self.hashes_stale || self.terrain_hashes.len() != terrains.len() {
            self.terrain_hashes = terrains.iter().map(|t| hash_heightmap(&t.heightmap)).collect();
            self.hashes_stale = false;
        }
    }

    fn key(&mut self, w: &UndoWorld) -> u64 {
        self.refresh_hashes(w.terrains);
        let mut h = DefaultHasher::new();
        let s = &*w.scene_ctrl;
        json_bytes(&mut h, &s.terrain);
        json_bytes(&mut h, &s.sky);
        json_bytes(&mut h, &s.water);
        json_bytes(&mut h, &s.ground);
        json_bytes(&mut h, &s.lights);
        json_bytes(&mut h, &s.postprocess);
        h.write_u32(s.selected_material);
        h.write_u32(s.render_mode);
        h.write(bytemuck::cast_slice(&w.materials[..]));
        for n in w.material_names.iter() {
            h.write(n.as_bytes());
            h.write_u8(0);
        }
        json_bytes(&mut h, &*w.objects);
        for (t, th) in w.terrains.iter().zip(&self.terrain_hashes) {
            h.write_u64(*th);
            json_bytes(&mut h, &t.config);
        }
        h.write_usize(w.terrains.len());
        h.finish()
    }

    fn capture(&self, w: &UndoWorld) -> Snapshot {
        let prev = self.committed.as_ref();
        let terrains = w
            .terrains
            .iter()
            .zip(&self.terrain_hashes)
            .map(|(t, &hash)| {
                let shared = prev.and_then(|p| p.terrains.iter().find(|s| s.hash == hash && s.heightmap.resolution == t.heightmap.resolution));
                TerrainSnap {
                    hash,
                    heightmap: shared.map(|s| s.heightmap.clone()).unwrap_or_else(|| Arc::new(t.heightmap.clone())),
                    config: t.config.clone(),
                }
            })
            .collect();
        Snapshot {
            scene: w.scene_ctrl.clone(),
            materials: *w.materials,
            material_names: w.material_names.clone(),
            objects: w.objects.clone(),
            terrains,
            selected: *w.selected,
        }
    }

    /// If the editable state differs from the last committed one, push the previous committed state onto
    /// the undo stack and commit the current one. Returns true when an undo point was created.
    /// The caller must only call this while the UI is idle (no drag, no text editing, no lab open).
    pub fn commit_if_changed(&mut self, w: &UndoWorld) -> bool {
        let key = self.key(w);
        if self.committed.is_none() {
            self.committed = Some(self.capture(w));
            self.committed_key = key;
            return false;
        }
        if key == self.committed_key {
            return false;
        }
        let new = self.capture(w);
        if let Some(prev) = self.committed.replace(new) {
            self.stack.push(prev);
        }
        self.committed_key = key;
        true
    }

    /// Restore the previous undo point. The caller should have run `commit_if_changed` first.
    pub fn undo(&mut self, w: &mut UndoWorld) -> bool {
        let Some(cur) = self.committed.clone() else { return false };
        match self.stack.undo(cur) {
            Some(prev) => {
                self.apply(prev, w);
                true
            }
            None => false,
        }
    }

    pub fn redo(&mut self, w: &mut UndoWorld) -> bool {
        let Some(cur) = self.committed.clone() else { return false };
        match self.stack.redo(cur) {
            Some(next) => {
                self.apply(next, w);
                true
            }
            None => false,
        }
    }

    fn apply(&mut self, snap: Snapshot, w: &mut UndoWorld) {
        *w.scene_ctrl = snap.scene.clone();
        *w.materials = snap.materials;
        *w.material_names = snap.material_names.clone();
        *w.objects = snap.objects.clone();
        *w.terrains = snap
            .terrains
            .iter()
            .map(|t| TerrainObjectData { heightmap: (*t.heightmap).clone(), config: t.config.clone() })
            .collect();
        // Drop any heightmap the restored objects do not reference (keeps ids consistent)
        crate::terrain::gc_terrains(w.objects, w.terrains);
        *w.selected = snap.selected.min(w.objects.len().saturating_sub(1));
        self.terrain_hashes = snap.terrains.iter().map(|t| t.hash).collect();
        self.hashes_stale = self.terrain_hashes.len() != w.terrains.len();
        self.committed_key = self.key(w);
        self.committed = Some(snap);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stack_push_undo_redo() {
        let mut s = UndoStack::new(8);
        // states: 0 -> 1 -> 2 (current = 2)
        s.push(0);
        s.push(1);
        assert_eq!(s.undo_len(), 2);
        assert_eq!(s.undo(2), Some(1));
        assert_eq!(s.undo(1), Some(0));
        assert_eq!(s.undo(0), None);
        assert_eq!(s.redo_len(), 2);
        assert_eq!(s.redo(0), Some(1));
        assert_eq!(s.redo(1), Some(2));
        assert_eq!(s.redo(2), None);
        assert_eq!(s.undo_len(), 2);
    }

    #[test]
    fn stack_cap_drops_oldest() {
        let mut s = UndoStack::new(3);
        for i in 0..10 {
            s.push(i);
        }
        assert_eq!(s.undo_len(), 3);
        assert_eq!(s.undo(10), Some(9));
        assert_eq!(s.undo(9), Some(8));
        assert_eq!(s.undo(8), Some(7));
        assert_eq!(s.undo(7), None);
    }

    #[test]
    fn new_edit_clears_redo() {
        let mut s = UndoStack::new(8);
        s.push(0);
        s.push(1);
        assert_eq!(s.undo(2), Some(1));
        assert_eq!(s.redo_len(), 1);
        s.push(1); // new edit from state 1
        assert_eq!(s.redo_len(), 0);
        assert_eq!(s.redo(5), None);
    }

    struct Fixture {
        scene: SceneController,
        materials: [GpuMaterial; GpuMaterial::NUM_PRESETS],
        names: [String; GpuMaterial::NUM_PRESETS],
        objects: Vec<SceneObject>,
        terrains: Vec<TerrainObjectData>,
        selected: usize,
    }

    impl Fixture {
        fn new() -> Self {
            let mut objects = vec![SceneObject::sphere("a", [0.0; 3], 1.0, 0), SceneObject::sphere("t", [0.0; 3], 1.0, 0)];
            objects[1].terrain_id = Some(0);
            Self {
                scene: SceneController::new(),
                materials: GpuMaterial::default_preset_list(),
                names: std::array::from_fn(|i| format!("m{i}")),
                objects,
                terrains: vec![TerrainObjectData::new(8, &TerrainConfig::default())],
                selected: 0,
            }
        }
        fn world(&mut self) -> UndoWorld<'_> {
            UndoWorld {
                scene_ctrl: &mut self.scene,
                materials: &mut self.materials,
                material_names: &mut self.names,
                objects: &mut self.objects,
                terrains: &mut self.terrains,
                selected: &mut self.selected,
            }
        }
    }

    #[test]
    fn manager_commit_undo_redo() {
        let mut f = Fixture::new();
        let mut m = UndoManager::new();
        assert!(!m.commit_if_changed(&f.world())); // initial capture, no undo point
        assert!(!m.commit_if_changed(&f.world())); // unchanged

        f.objects[0].position[0] = 5.0;
        assert!(m.commit_if_changed(&f.world()));
        assert_eq!(m.undo_steps(), 1);
        assert!(!m.commit_if_changed(&f.world()));

        f.materials[1].diffuse[0] += 0.1;
        f.terrains[0].heightmap.data[3] = 0.77;
        m.mark_terrains_changed();
        assert!(m.commit_if_changed(&f.world()));
        assert_eq!(m.undo_steps(), 2);

        assert!(m.undo(&mut f.world()));
        assert_eq!(f.objects[0].position[0], 5.0);
        assert_ne!(f.terrains[0].heightmap.data[3], 0.77);
        assert!(!m.commit_if_changed(&f.world())); // restoring is not a new edit
        assert!(m.undo(&mut f.world()));
        assert_eq!(f.objects[0].position[0], 0.0);
        assert!(!m.undo(&mut f.world()));
        assert_eq!(m.redo_steps(), 2);

        assert!(m.redo(&mut f.world()));
        assert!(m.redo(&mut f.world()));
        assert_eq!(f.terrains[0].heightmap.data[3], 0.77);
        assert!(!m.redo(&mut f.world()));

        // a new edit after undo clears redo
        assert!(m.undo(&mut f.world()));
        f.objects[0].position[2] = 1.0;
        assert!(m.commit_if_changed(&f.world()));
        assert_eq!(m.redo_steps(), 0);
    }

    #[test]
    fn unchanged_terrains_are_shared() {
        let mut f = Fixture::new();
        let mut m = UndoManager::new();
        m.commit_if_changed(&f.world());
        f.objects[0].position[0] = 1.0;
        m.commit_if_changed(&f.world());
        f.objects[0].position[0] = 2.0;
        m.commit_if_changed(&f.world());
        let a = &m.stack.undo[0].terrains[0].heightmap;
        let b = &m.committed.as_ref().unwrap().terrains[0].heightmap;
        assert!(Arc::ptr_eq(a, b));
    }

    #[test]
    fn restore_collects_orphan_terrains() {
        let mut f = Fixture::new();
        let mut m = UndoManager::new();
        m.commit_if_changed(&f.world());
        f.objects.remove(1);
        crate::terrain::gc_terrains(&mut f.objects, &mut f.terrains);
        m.mark_terrains_changed();
        assert!(m.commit_if_changed(&f.world()));
        assert!(m.undo(&mut f.world()));
        assert_eq!(f.terrains.len(), 1);
        assert_eq!(f.objects[1].terrain_id, Some(0));
    }
}
