use glam::{Quat, Vec3};

use crate::{actions::XRHands, models::Model, physics_manager::{GRAVITY, PhysicsManager, PhysicsObject}, world::GameObject};

pub struct Player {
    pub position: Vec3,
    pub rotation: Quat,
    pub grounded: bool,

    pub head_rotation: Quat,
    pub head_position: Vec3,

    pub left_hand: GameObject,
    pub right_hand: GameObject,
}

impl Player {
    pub fn new(left_hand: GameObject, right_hand: GameObject) -> Player {
        Player {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            grounded: true,

            head_rotation: Quat::IDENTITY,
            head_position: Vec3::ZERO,

            left_hand,
            right_hand,
        }
    }

    fn to_world(&self, pos: Vec3, rot: Quat) -> (Vec3, Quat) {
        (self.position + self.rotation * pos, self.rotation * rot)
    }

    pub fn turn(&mut self, yaw_delta: f32) {
        // Head position in world space before rotating
        let head_world = self.position + self.rotation * self.head_position;
    
        // Apply yaw around world up
        self.rotation = (Quat::from_rotation_y(-yaw_delta / 50.0) * self.rotation).normalize();
    
        // Move the rig so the head stays in the same world spot
        self.position = head_world - self.rotation * self.head_position;
    }

    // TODO: Actual player body physics
    pub fn update(&mut self, physics_manager: &mut PhysicsManager, hands: &XRHands, delta: f32) {
        // Move first
        self.turn(hands.right_controls.thumbstick.x);

        let lstick = hands.left_controls.thumbstick;
        let look = self.rotation * Quat::from_rotation_y(self.head_rotation.to_euler(glam::EulerRot::YXZ).0);
        let forward = look * Vec3::NEG_Z;
        let right = look * Vec3::X;
        self.position += (forward * lstick.y + right * lstick.x) * 2.0 * delta;

        // World-space hand poses
        let (lp, lr) = self.to_world(hands.left_position, hands.left_rotation);
        let (rp, rr) = self.to_world(hands.right_position, hands.right_rotation);

        physics_manager.set_rigid_kinematic_target(lp, lr, self.left_hand.physics_object.rigidbody);
        physics_manager.set_rigid_kinematic_target(rp, rr, self.right_hand.physics_object.rigidbody);

        // Visual hands: use the tracked world pose directly (no lag)
        self.left_hand.model.position = lp;
        self.left_hand.model.rotation = lr;
        self.right_hand.model.position = rp;
        self.right_hand.model.rotation = rr;
    }
}
