use glam::{Vec3};
use wgpu::Device;

use crate::{actions::{XRHands}, models::Model};

pub struct World {
    models: Vec<Model>,
    pub hands: XRHands,
    left_hand: Model,
    right_hand: Model,
}

impl World {
    pub fn new(device: &Device) -> World {
        let mut left_hand = Model::load(&device, "gun.glb").expect("Couldn't load left hand model");
        let mut right_hand = Model::load(&device, "gun.glb").expect("Couldn't load right hand model");
        left_hand.scale = Vec3::new(0.05, 0.05, 0.05);
        right_hand.scale = Vec3::new(0.05, 0.05, 0.05);

        let world = World {
            models: Vec::new(),
            hands: XRHands::new(),
            left_hand,
            right_hand,
        };

        world
    }

    pub fn tick(&mut self,) {
        self.left_hand.position = self.hands.left_position;
        self.left_hand.rotation = self.hands.left_rotation;
        self.right_hand.position = self.hands.right_position;
        self.right_hand.rotation = self.hands.right_rotation;
    }

    pub fn get_world(&self) -> Vec<&Model> {
        let mut world = self.models.iter().collect::<Vec<_>>();
        world.push(&self.left_hand);
        world.push(&self.right_hand);
        world
    }
}
