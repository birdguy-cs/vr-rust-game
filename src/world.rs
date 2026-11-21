use std::sync::Arc;

use glam::{Vec2, Vec3};
use wgpu::{BindGroup, Device};

use crate::{actions::XRHands, models::Model, physics_manager::{PhysicsManager, PhysicsObject}};

pub struct GameObject {
    pub model: Model,
    pub physics_object: PhysicsObject,
}

impl GameObject {
    pub fn new(model: Model, physics_manager: &mut PhysicsManager, kinematic: bool) -> GameObject {
        let physics_object = physics_manager.create_new_object(model.position, model.rotation.xyz(), kinematic, model.clone());

        let go = GameObject {
            model,
            physics_object,
        };

        go
    }
}

// The current scene basically
pub struct World {
    physics_manager: PhysicsManager,
    models: Vec<Model>, // All this vec is used for is giving the renderer models when rendering
    pub hands: XRHands,
    pub headset_on: bool,
    headset_on_last: bool,
    left_hand: Model,
    right_hand: Model,
    world_objects: Vec<GameObject>,
}

impl World {
    pub fn new(device: &Device, ground_texture: Arc<BindGroup>) -> World {
        let mut sphere = Model::load(&device, "sphere.glb").expect("Couldn't load sphere model");
        let mut cube = Model::load(&device, "cube.glb").expect("Couldn't load cube model");
        let mut left_hand = Model::load(&device, "gun.glb").expect("Couldn't load left hand model");
        let mut right_hand = Model::load(&device, "gun.glb").expect("Couldn't load right hand model");
        let mut test_model = Model::load(&device, "gun.glb").expect("Couldn't load test model");

        left_hand.scale = Vec3::new(0.05, 0.05, 0.05);
        right_hand.scale = Vec3::new(0.05, 0.05, 0.05);

        let mut physics_manager = PhysicsManager::new();
        let mut world_objects = Vec::new();

        sphere.scale = Vec3::new(0.5, 0.5, 0.5);
        sphere.position = Vec3::new(0.0, 20.0, 0.0);
        cube.scale = Vec3::new(100.0, 0.1, 100.0);

        test_model.scale = Vec3::new(0.05, 0.05, 0.05);
        test_model.position = Vec3::new(0.0, 2.0, 5.0);

        let test_sphere = GameObject::new(sphere.clone(), &mut physics_manager, false);
        let test_object = GameObject::new(test_model.clone(), &mut physics_manager, true);
        // let test_sphere2 = GameObject::new(sphere.clone(), &mut physics_manager, false);
        // let test_sphere3 = GameObject::new(sphere.clone(), &mut physics_manager, false);

        // Create ground
        let mut ground_cube = cube.clone();

        ground_cube.set_mat(ground_texture);
        ground_cube.uv_scale = Vec2::new(100.0, 100.0);

        let ground = GameObject::new(ground_cube, &mut physics_manager, true);

        world_objects.push(test_sphere);
        world_objects.push(test_object);
        // world_objects.push(test_sphere2);
        // world_objects.push(test_sphere3);
        world_objects.push(ground);

        let world = World {
            physics_manager,
            models: Vec::new(),
            hands: XRHands::new(),
            headset_on: true,
            headset_on_last: true,
            left_hand,
            right_hand,
            world_objects,
        };

        world
    }

    pub fn tick(&mut self,) {
        self.left_hand.position = self.hands.left_position;
        self.left_hand.rotation = self.hands.left_rotation;
        self.right_hand.position = self.hands.right_position;
        self.right_hand.rotation = self.hands.right_rotation;

        self.physics_manager.physics_tick();

        for object in self.world_objects.iter_mut() {
            let rb = &self.physics_manager.rigid_body_set[object.physics_object.rigidbody];

            let rb_pos3 = *rb.position();

            object.model.rotation = rb_pos3.rotation;
            object.model.position = rb_pos3.translation;
        }

        if self.headset_on != self.headset_on_last {
            println!("Headset is {}", if self.headset_on {"on"} else {"off"});
        }
        self.headset_on_last = self.headset_on;
    }

    pub fn get_world(&self) -> Vec<&Model> {
        let mut world = self.models.iter().collect::<Vec<_>>();
        world.push(&self.left_hand);
        world.push(&self.right_hand);

        for model in self.world_objects.iter().collect::<Vec<_>>() {
            world.push(&model.model);
        }

        world
    }
}
