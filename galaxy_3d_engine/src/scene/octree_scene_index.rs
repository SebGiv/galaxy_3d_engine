/// OctreeSceneIndex — static octree spatial index for frustum culling.
///
/// Uses **Approach 1** (single-node placement): each object is stored in
/// exactly one node — the deepest node whose AABB fully contains the object.
/// If the object straddles a child boundary, it stays in the parent.
///
/// Benefits:
/// - No duplication → no HashSet needed for query results
/// - Simple insert/remove/query logic
/// - O(depth) insert and remove per object
///
/// The tree structure is pre-allocated at construction time (static octree)
/// with a fixed `max_depth`. All 8^d nodes exist regardless of occupancy.
///
/// Each object stores its world-space position alongside its key inside the
/// node it belongs to. This allows `query_frustum` to compute view-space depth
/// for each visible instance without a separate scene lookup.

use rustc_hash::FxHashMap;
use glam::Vec3;
use crate::camera::{Frustum, FrustumTest, VisibleInstance};
use super::render_instance::{RenderInstanceKey, AABB};
use super::scene_index::SceneIndex;

/// Index of the root node in the flat node array.
const ROOT: usize = 0;

/// A single node in the octree.
struct OctreeNode {
    /// World-space AABB of this node
    aabb: AABB,
    /// Index of the first child in the flat array (0 = no children / leaf)
    first_child: usize,
    /// Objects stored in this node, paired with their world-space positions.
    /// (Approach 1: objects that don't fit in any child.)
    objects: Vec<(RenderInstanceKey, Vec3)>,
}

/// Static octree spatial index.
///
/// Constructed with a world-space AABB and a maximum depth.
/// All nodes are pre-allocated. Objects are inserted into the deepest
/// node that fully contains their world AABB.
pub struct OctreeSceneIndex {
    /// Flat array of all octree nodes (pre-allocated)
    nodes: Vec<OctreeNode>,
    /// Maximum depth of the tree (root = depth 0)
    max_depth: u32,
    /// Reverse lookup: object key → (node index, world position, world AABB).
    /// Needed for O(1) remove without tree traversal, and to recover the AABB
    /// during partial-frustum queries.
    object_locations: FxHashMap<RenderInstanceKey, (usize, Vec3, AABB)>,
    /// Pre-computed subtree sizes indexed by remaining depth.
    /// subtree_sizes[d] = total node count for a subtree of depth d.
    subtree_sizes: Vec<usize>,
}

impl OctreeSceneIndex {
    /// Create a new static octree with the given world bounds and depth.
    ///
    /// # Arguments
    ///
    /// * `world_aabb` - The world-space AABB encompassing the entire scene
    /// * `max_depth` - Maximum tree depth (root = 0). Total nodes = (8^(d+1) - 1) / 7.
    ///   Typical values: 4–6 for most scenes.
    pub fn new(world_aabb: AABB, max_depth: u32) -> Self {
        // Pre-compute total node count: sum of 8^i for i=0..=max_depth
        let total_nodes = Self::total_node_count(max_depth);
        let mut nodes = Vec::with_capacity(total_nodes);

        // Build the tree level by level
        Self::build_recursive(&mut nodes, &world_aabb, 0, max_depth);

        debug_assert_eq!(nodes.len(), total_nodes);

        let subtree_sizes: Vec<usize> = (0..=max_depth).map(Self::total_node_count).collect();

        Self {
            nodes,
            max_depth,
            object_locations: FxHashMap::default(),
            subtree_sizes,
        }
    }

    /// Total number of nodes for a given depth: (8^(d+1) - 1) / 7
    fn total_node_count(max_depth: u32) -> usize {
        let mut count = 0usize;
        let mut level_count = 1usize;
        for _ in 0..=max_depth {
            count += level_count;
            level_count *= 8;
        }
        count
    }

    /// Recursively build the static octree node array (depth-first).
    fn build_recursive(
        nodes: &mut Vec<OctreeNode>,
        aabb: &AABB,
        depth: u32,
        max_depth: u32,
    ) {
        let node_index = nodes.len();

        if depth >= max_depth {
            // Leaf node: no children
            nodes.push(OctreeNode {
                aabb: *aabb,
                first_child: 0,
                objects: Vec::new(),
            });
            return;
        }

        // Internal node: reserve slot, then build 8 children
        nodes.push(OctreeNode {
            aabb: *aabb,
            first_child: 0, // will be filled below
            objects: Vec::new(),
        });

        let center = (*aabb).center();
        let first_child = nodes.len();
        nodes[node_index].first_child = first_child;

        // 8 children: enumerate all octants
        for octant in 0..8u8 {
            let child_aabb = Self::octant_aabb(aabb, &center, octant);
            Self::build_recursive(nodes, &child_aabb, depth + 1, max_depth);
        }
    }

    /// Compute the AABB of a specific octant (0–7).
    ///
    /// Octant bit layout: bit0 = X, bit1 = Y, bit2 = Z.
    /// - 0 = low, 1 = high for each axis.
    fn octant_aabb(parent: &AABB, center: &Vec3, octant: u8) -> AABB {
        AABB {
            min: Vec3::new(
                if octant & 1 == 0 { parent.min.x } else { center.x },
                if octant & 2 == 0 { parent.min.y } else { center.y },
                if octant & 4 == 0 { parent.min.z } else { center.z },
            ),
            max: Vec3::new(
                if octant & 1 == 0 { center.x } else { parent.max.x },
                if octant & 2 == 0 { center.y } else { parent.max.y },
                if octant & 4 == 0 { center.z } else { parent.max.z },
            ),
        }
    }

    /// Determine which octant a point falls into relative to a center.
    ///
    /// Bit layout: bit0 = X, bit1 = Y, bit2 = Z (0 = low, 1 = high).
    fn point_octant(center: &Vec3, point: &Vec3) -> u8 {
        ((point.x >= center.x) as u8)
            | (((point.y >= center.y) as u8) << 1)
            | (((point.z >= center.z) as u8) << 2)
    }

    /// Insert an object into the deepest node that fully contains it.
    ///
    /// Uses direct octant calculation instead of testing all 8 children:
    /// if both AABB corners (min, max) fall into the same octant, the object
    /// fits entirely in that child. Otherwise it straddles a boundary
    /// and stays in the current node.
    fn insert_iterative(
        &mut self,
        key: RenderInstanceKey,
        world_position: Vec3,
        world_aabb: &AABB,
    ) -> usize {
        let mut node_idx = ROOT;
        let mut depth = 0;

        loop {
            if depth >= self.max_depth {
                self.nodes[node_idx].objects.push((key, world_position));
                return node_idx;
            }

            let first_child = self.nodes[node_idx].first_child;
            if first_child == 0 {
                self.nodes[node_idx].objects.push((key, world_position));
                return node_idx;
            }

            let center = self.nodes[node_idx].aabb.center();
            let min_oct = Self::point_octant(&center, &world_aabb.min);
            let max_oct = Self::point_octant(&center, &world_aabb.max);

            if min_oct != max_oct {
                // Straddles boundary — stays in current node
                self.nodes[node_idx].objects.push((key, world_position));
                return node_idx;
            }

            // Both corners in same octant — descend
            node_idx = first_child + self.subtree_offset(min_oct, self.max_depth - depth - 1);
            depth += 1;
        }
    }

    /// Find the deepest node that fully contains an AABB (read-only, no tree modification).
    ///
    /// Same traversal logic as `insert_iterative` but without pushing into any node.
    /// Used by `update` to check if an object needs to move.
    fn find_target_node(&self, world_aabb: &AABB) -> usize {
        let mut node_idx = ROOT;
        let mut depth = 0;

        loop {
            if depth >= self.max_depth {
                return node_idx;
            }

            let first_child = self.nodes[node_idx].first_child;
            if first_child == 0 {
                return node_idx;
            }

            let center = self.nodes[node_idx].aabb.center();
            let min_oct = Self::point_octant(&center, &world_aabb.min);
            let max_oct = Self::point_octant(&center, &world_aabb.max);

            if min_oct != max_oct {
                return node_idx;
            }

            node_idx = first_child + self.subtree_offset(min_oct, self.max_depth - depth - 1);
            depth += 1;
        }
    }

    /// Compute the offset of octant `i` in the depth-first node layout.
    ///
    /// Uses pre-computed subtree sizes for O(1) lookup.
    fn subtree_offset(&self, octant: u8, remaining_depth: u32) -> usize {
        octant as usize * self.subtree_sizes[remaining_depth as usize]
    }

    /// Recursively query the octree with a frustum.
    ///
    /// 3-way classification at each node:
    /// - `Outside` → skip entire subtree
    /// - `Inside` → collect all objects from subtree without further testing
    /// - `Partial` → test objects individually, recurse into children
    ///
    /// For each visible instance, computes the view-space depth
    /// `depth = dot(inst_pos - camera_pos, camera_forward)` and pushes it
    /// (encoded to u16) into `results`.
    fn query_recursive(
        &self,
        node_idx: usize,
        frustum: &Frustum,
        classification: FrustumTest,
        camera_pos: Vec3,
        camera_forward: Vec3,
        results: &mut Vec<VisibleInstance>,
        depth: u32,
    ) {
        let node = &self.nodes[node_idx];

        match classification {
            FrustumTest::Outside => return,

            FrustumTest::Inside => {
                // Everything in this subtree is visible
                self.collect_all(node_idx, camera_pos, camera_forward, results, depth);
                return;
            }

            FrustumTest::Partial => {
                // Test objects at this node individually
                for &(key, inst_pos) in &node.objects {
                    // Recover the AABB from object_locations to perform a precise frustum test.
                    if let Some((_, _, world_aabb)) = self.object_locations.get(&key) {
                        if frustum.intersects_aabb(world_aabb) {
                            let view_depth = (inst_pos - camera_pos).dot(camera_forward);
                            results.push(VisibleInstance { key, distance: view_depth });
                        }
                    }
                }

                // Recurse into children
                if depth < self.max_depth {
                    let first_child = node.first_child;
                    if first_child != 0 {
                        for octant in 0..8u8 {
                            let child_idx = first_child
                                + self.subtree_offset(octant, self.max_depth - depth - 1);
                            let child_aabb = &self.nodes[child_idx].aabb;
                            let child_class = frustum.classify_aabb(child_aabb);
                            self.query_recursive(
                                child_idx, frustum, child_class,
                                camera_pos, camera_forward, results, depth + 1,
                            );
                        }
                    }
                }
            }
        }
    }

    /// Collect all objects from a node and its entire subtree (no frustum test).
    ///
    /// Used when the parent node's AABB is fully inside the frustum. Each object
    /// is pushed with its computed view-space depth.
    fn collect_all(
        &self,
        node_idx: usize,
        camera_pos: Vec3,
        camera_forward: Vec3,
        results: &mut Vec<VisibleInstance>,
        depth: u32,
    ) {
        let node = &self.nodes[node_idx];
        for &(key, inst_pos) in &node.objects {
            let view_depth = (inst_pos - camera_pos).dot(camera_forward);
            results.push(VisibleInstance { key, distance: view_depth });
        }

        if depth < self.max_depth {
            let first_child = node.first_child;
            if first_child != 0 {
                for octant in 0..8u8 {
                    let child_idx = first_child
                        + self.subtree_offset(octant, self.max_depth - depth - 1);
                    self.collect_all(child_idx, camera_pos, camera_forward, results, depth + 1);
                }
            }
        }
    }
}

// ===== SCENE INDEX TRAIT =====

impl SceneIndex for OctreeSceneIndex {
    fn insert(&mut self, key: RenderInstanceKey, world_position: Vec3, world_aabb: &AABB) {
        // If object is outside the octree bounds, store at root
        if !self.nodes[ROOT].aabb.contains(world_aabb) {
            self.nodes[ROOT].objects.push((key, world_position));
            self.object_locations.insert(key, (ROOT, world_position, *world_aabb));
            return;
        }

        let node_idx = self.insert_iterative(key, world_position, world_aabb);
        self.object_locations.insert(key, (node_idx, world_position, *world_aabb));
    }

    fn remove(&mut self, key: RenderInstanceKey) {
        if let Some((node_idx, _, _)) = self.object_locations.remove(&key) {
            let objects = &mut self.nodes[node_idx].objects;
            if let Some(pos) = objects.iter().position(|(k, _)| *k == key) {
                objects.swap_remove(pos);
            }
        }
    }

    fn update(&mut self, key: RenderInstanceKey, world_position: Vec3, world_aabb: &AABB) {
        let target = if self.nodes[ROOT].aabb.contains(world_aabb) {
            self.find_target_node(world_aabb)
        } else {
            ROOT
        };

        if let Some(entry) = self.object_locations.get_mut(&key) {
            if entry.0 == target {
                // Same node — update stored position + AABB in place,
                // and update the position stored on the node itself.
                entry.1 = world_position;
                entry.2 = *world_aabb;
                let objects = &mut self.nodes[target].objects;
                if let Some(pos) = objects.iter().position(|(k, _)| *k == key) {
                    objects[pos].1 = world_position;
                }
                return;
            }
        }

        // Different node — remove from old, place directly in target
        self.remove(key);
        self.nodes[target].objects.push((key, world_position));
        self.object_locations.insert(key, (target, world_position, *world_aabb));
    }

    fn query_frustum(
        &self,
        frustum: &Frustum,
        camera_pos: Vec3,
        camera_forward: Vec3,
        results: &mut Vec<VisibleInstance>,
    ) {
        if self.nodes.is_empty() {
            return;
        }

        let root_class = frustum.classify_aabb(&self.nodes[ROOT].aabb);
        self.query_recursive(ROOT, frustum, root_class, camera_pos, camera_forward, results, 0);
    }

    fn clear(&mut self) {
        for node in &mut self.nodes {
            node.objects.clear();
        }
        self.object_locations.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{Mat4, Vec3};

    fn make_aabb(min: Vec3, max: Vec3) -> AABB {
        AABB { min, max }
    }

    fn world_aabb() -> AABB {
        make_aabb(Vec3::splat(-100.0), Vec3::splat(100.0))
    }

    /// Compute the center of an AABB as a stand-in for the instance position
    /// (used by tests that previously did not care about position).
    fn aabb_center(aabb: &AABB) -> Vec3 {
        (aabb.min + aabb.max) * 0.5
    }

    /// Camera looking down -Z from origin, narrow FOV.
    fn forward_camera_pos_forward() -> (Vec3, Vec3) {
        (Vec3::ZERO, Vec3::new(0.0, 0.0, -1.0))
    }

    /// Create a frustum that sees everything (all planes far away).
    fn all_visible_frustum() -> Frustum {
        let proj = Mat4::perspective_rh(
            std::f32::consts::FRAC_PI_2, 1.0, 0.1, 1000.0,
        );
        let view = Mat4::look_at_rh(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, -1.0),
            Vec3::Y,
        );
        Frustum::from_view_projection(&(proj * view))
    }

    /// Create a frustum looking down -Z from origin, narrow FOV.
    fn forward_frustum() -> Frustum {
        let proj = Mat4::perspective_rh(
            std::f32::consts::FRAC_PI_4, 1.0, 0.1, 50.0,
        );
        let view = Mat4::look_at_rh(
            Vec3::ZERO,
            Vec3::new(0.0, 0.0, -1.0),
            Vec3::Y,
        );
        Frustum::from_view_projection(&(proj * view))
    }

    // Use a simple key factory
    fn make_key(idx: u32) -> RenderInstanceKey {
        use slotmap::SlotMap;
        let mut sm = SlotMap::<RenderInstanceKey, ()>::with_key();
        let mut key = sm.insert(());
        for _ in 1..idx {
            key = sm.insert(());
        }
        key
    }

    /// Helper: collect visible keys from a results vec.
    fn visible_keys(list: &[VisibleInstance]) -> Vec<RenderInstanceKey> {
        list.iter().map(|vi| vi.key).collect()
    }

    #[test]
    fn test_new_creates_correct_node_count() {
        let octree = OctreeSceneIndex::new(world_aabb(), 0);
        assert_eq!(octree.nodes.len(), 1); // depth 0 = root only

        let octree = OctreeSceneIndex::new(world_aabb(), 1);
        assert_eq!(octree.nodes.len(), 9); // 1 + 8

        let octree = OctreeSceneIndex::new(world_aabb(), 2);
        assert_eq!(octree.nodes.len(), 73); // 1 + 8 + 64
    }

    #[test]
    fn test_insert_and_query_single_object() {
        let mut octree = OctreeSceneIndex::new(world_aabb(), 3);
        let key = make_key(1);
        let obj_aabb = make_aabb(Vec3::new(-1.0, -1.0, -10.0), Vec3::new(1.0, 1.0, -8.0));

        octree.insert(key, aabb_center(&obj_aabb), &obj_aabb);

        let frustum = forward_frustum();
        let (cam_pos, cam_fwd) = forward_camera_pos_forward();
        let mut results = Vec::new();
        octree.query_frustum(&frustum, cam_pos, cam_fwd, &mut results);

        assert!(visible_keys(&results).contains(&key));
    }

    #[test]
    fn test_insert_outside_bounds_goes_to_root() {
        let mut octree = OctreeSceneIndex::new(world_aabb(), 3);
        let key = make_key(1);
        let obj_aabb = make_aabb(Vec3::splat(-200.0), Vec3::splat(-150.0));

        octree.insert(key, aabb_center(&obj_aabb), &obj_aabb);

        // Should be stored at root
        let root_keys: Vec<_> = octree.nodes[ROOT].objects.iter().map(|(k, _)| *k).collect();
        assert!(root_keys.contains(&key));
    }

    #[test]
    fn test_remove_object() {
        let mut octree = OctreeSceneIndex::new(world_aabb(), 3);
        let key = make_key(1);
        let obj_aabb = make_aabb(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0));

        octree.insert(key, aabb_center(&obj_aabb), &obj_aabb);
        assert!(octree.object_locations.contains_key(&key));

        octree.remove(key);
        assert!(!octree.object_locations.contains_key(&key));

        let frustum = all_visible_frustum();
        let (cam_pos, cam_fwd) = forward_camera_pos_forward();
        let mut results = Vec::new();
        octree.query_frustum(&frustum, cam_pos, cam_fwd, &mut results);
        assert!(!visible_keys(&results).contains(&key));
    }

    #[test]
    fn test_update_moves_object() {
        let mut octree = OctreeSceneIndex::new(world_aabb(), 3);
        let key = make_key(1);

        // Start at one position
        let aabb1 = make_aabb(Vec3::new(50.0, 50.0, 50.0), Vec3::new(60.0, 60.0, 60.0));
        octree.insert(key, aabb_center(&aabb1), &aabb1);
        let (node1, _, _) = octree.object_locations[&key];

        // Move to a distant position
        let aabb2 = make_aabb(Vec3::new(-60.0, -60.0, -60.0), Vec3::new(-50.0, -50.0, -50.0));
        octree.update(key, aabb_center(&aabb2), &aabb2);
        let (node2, _, _) = octree.object_locations[&key];

        // Should be in a different node
        assert_ne!(node1, node2);
    }

    #[test]
    fn test_query_culls_outside_objects() {
        let mut octree = OctreeSceneIndex::new(world_aabb(), 3);

        let key_visible = make_key(1);
        let key_behind = make_key(2);

        // In front of camera (visible with forward_frustum)
        let aabb_front = make_aabb(Vec3::new(-1.0, -1.0, -10.0), Vec3::new(1.0, 1.0, -8.0));
        octree.insert(key_visible, aabb_center(&aabb_front), &aabb_front);

        // Behind camera (should be culled)
        let aabb_behind = make_aabb(Vec3::new(-1.0, -1.0, 10.0), Vec3::new(1.0, 1.0, 12.0));
        octree.insert(key_behind, aabb_center(&aabb_behind), &aabb_behind);

        let frustum = forward_frustum();
        let (cam_pos, cam_fwd) = forward_camera_pos_forward();
        let mut results = Vec::new();
        octree.query_frustum(&frustum, cam_pos, cam_fwd, &mut results);

        let keys = visible_keys(&results);
        assert!(keys.contains(&key_visible));
        assert!(!keys.contains(&key_behind));
    }

    #[test]
    fn test_clear_removes_all() {
        let mut octree = OctreeSceneIndex::new(world_aabb(), 3);

        for i in 1..=10 {
            let key = make_key(i);
            let pos = i as f32 * 5.0 - 25.0;
            let aabb = make_aabb(
                Vec3::new(pos, pos, pos),
                Vec3::new(pos + 2.0, pos + 2.0, pos + 2.0),
            );
            octree.insert(key, aabb_center(&aabb), &aabb);
        }

        assert_eq!(octree.object_locations.len(), 10);

        octree.clear();

        assert_eq!(octree.object_locations.len(), 0);

        let frustum = all_visible_frustum();
        let (cam_pos, cam_fwd) = forward_camera_pos_forward();
        let mut results = Vec::new();
        octree.query_frustum(&frustum, cam_pos, cam_fwd, &mut results);
        assert!(results.is_empty());
    }

    #[test]
    fn test_no_duplicates_in_results() {
        let mut octree = OctreeSceneIndex::new(world_aabb(), 3);
        let key = make_key(1);
        let obj_aabb = make_aabb(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0));

        octree.insert(key, aabb_center(&obj_aabb), &obj_aabb);

        let frustum = all_visible_frustum();
        let (cam_pos, cam_fwd) = forward_camera_pos_forward();
        let mut results = Vec::new();
        octree.query_frustum(&frustum, cam_pos, cam_fwd, &mut results);

        // With Approach 1, each object is in exactly one node → no duplicates
        let count = visible_keys(&results).iter().filter(|&&k| k == key).count();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_query_computes_distance() {
        let mut octree = OctreeSceneIndex::new(world_aabb(), 3);

        let key_near = make_key(1);
        let key_far = make_key(2);

        // Near object: 5 units ahead of camera
        let near_pos = Vec3::new(0.0, 0.0, -5.0);
        let near_aabb = make_aabb(near_pos - Vec3::splat(0.5), near_pos + Vec3::splat(0.5));
        octree.insert(key_near, near_pos, &near_aabb);

        // Far object: 30 units ahead of camera
        let far_pos = Vec3::new(0.0, 0.0, -30.0);
        let far_aabb = make_aabb(far_pos - Vec3::splat(0.5), far_pos + Vec3::splat(0.5));
        octree.insert(key_far, far_pos, &far_aabb);

        let frustum = forward_frustum();
        let (cam_pos, cam_fwd) = forward_camera_pos_forward();
        let mut results = Vec::new();
        octree.query_frustum(&frustum, cam_pos, cam_fwd, &mut results);

        // Both should be visible
        assert_eq!(results.len(), 2);

        // The near object's encoded distance should be smaller than the far one's
        // (smaller positive depth → smaller u16 sort value).
        let near_entry = results.iter().find(|vi| vi.key == key_near).unwrap();
        let far_entry = results.iter().find(|vi| vi.key == key_far).unwrap();
        assert!(near_entry.distance < far_entry.distance);
    }

    #[test]
    fn test_aabb_contains() {
        let big = make_aabb(Vec3::splat(-10.0), Vec3::splat(10.0));
        let small = make_aabb(Vec3::splat(-1.0), Vec3::splat(1.0));
        let straddling = make_aabb(Vec3::new(5.0, 5.0, 5.0), Vec3::new(15.0, 15.0, 15.0));

        assert!(big.contains(&small));
        assert!(!small.contains(&big));
        assert!(!big.contains(&straddling));
    }

    #[test]
    fn test_aabb_intersects() {
        let a = make_aabb(Vec3::splat(-2.0), Vec3::splat(2.0));
        let b = make_aabb(Vec3::splat(1.0), Vec3::splat(3.0));
        let c = make_aabb(Vec3::splat(5.0), Vec3::splat(7.0));

        assert!(a.intersects(&b)); // overlapping
        assert!(!a.intersects(&c)); // disjoint
    }
}
