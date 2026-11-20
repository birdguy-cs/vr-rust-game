use rapier3d::prelude::*;

use crate::models::Model;

pub struct PhysicsObject {
    pub collider: ColliderHandle,
    pub rigidbody: RigidBodyHandle,
}

pub struct PhysicsManager {
    pub rigid_body_set: RigidBodySet,
    pub collider_set: ColliderSet,
    pub gravity: Vector3,
    integration_parameters: IntegrationParameters,
    physics_pipeline: PhysicsPipeline,
    island_manager: IslandManager,
    broad_phase: BroadPhaseBvh,
    narrow_phase: NarrowPhase,
    impulse_joint_set: ImpulseJointSet,
    multibody_joint_set: MultibodyJointSet,
    ccd_solver: CCDSolver,
    physics_hooks: (),
    event_handler: (),
}

impl PhysicsManager {
    pub fn new() -> PhysicsManager {
        // Create all structures necessary for physics simulation
        let rigid_body_set = RigidBodySet::new();
        let collider_set = ColliderSet::new();
        let gravity = Vector::new(0.0, -9.81, 0.0);
        let integration_parameters = IntegrationParameters::default();
        let physics_pipeline = PhysicsPipeline::new();
        let island_manager = IslandManager::new();
        let broad_phase = DefaultBroadPhase::new();
        let narrow_phase = NarrowPhase::new();
        let impulse_joint_set = ImpulseJointSet::new();
        let multibody_joint_set = MultibodyJointSet::new();
        let ccd_solver = CCDSolver::new();
        let physics_hooks = ();
        let event_handler = ();

        let physics_manager = PhysicsManager {
            rigid_body_set,
            collider_set,
            gravity,
            integration_parameters,
            physics_pipeline,
            island_manager,
            broad_phase,
            narrow_phase,
            impulse_joint_set,
            multibody_joint_set,
            ccd_solver,
            physics_hooks,
            event_handler,
        };

        physics_manager
    }

    pub fn physics_tick(&mut self) {
        self.physics_pipeline.step(
            self.gravity,
            &self.integration_parameters,
            &mut self.island_manager,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.rigid_body_set,
            &mut self.collider_set,
            &mut self.impulse_joint_set,
            &mut self.multibody_joint_set,
            &mut self.ccd_solver,
            &self.physics_hooks,
            &self.event_handler,
        );
    }

    pub fn create_new_object(&mut self, position: Vec3, rotation: Vec3, kinematic: bool, model: Model) -> PhysicsObject {
        let rigid_body = if kinematic {
            RigidBodyBuilder::kinematic_position_based()
                .translation(position)
                .rotation(rotation)
                .build()
        } else {
            RigidBodyBuilder::dynamic()
                .translation(position)
                .rotation(rotation)
                .build()
        };

        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        for mesh in &model.meshes {
            let vertex_offset = vertices.len() as u32;

            vertices.extend(
                mesh.vertices
                    .iter()
                    .map(|v| [
                        v[0] * model.scale.x,
                        v[1] * model.scale.y,
                        v[2] * model.scale.z
                    ])
            );

            indices.extend(
                mesh.indices
                    .iter()
                    .map(|index| index + vertex_offset)
            );
        }
        let points: Vec<Vector> = vertices
            .iter()
            .map(|p| Vector::new(p[0], p[1], p[2]))
            .collect();

        let triangles: Vec<[u32; 3]> = indices
            .chunks_exact(3)
            .map(|i| [i[0], i[1], i[2]])
            .collect();

        let collider = ColliderBuilder::trimesh(points, triangles).expect("Failed to create mesh");

        let rigid_body_handle = self.rigid_body_set.insert(rigid_body);

        let collider_handle = self.collider_set.insert_with_parent(collider, rigid_body_handle, &mut self.rigid_body_set);

        PhysicsObject { collider: collider_handle, rigidbody: rigid_body_handle }
    }
}
