use std::sync::Arc;
use glam::{Vec2, Vec3};
use openxr::{Session, Vulkan};
use wgpu::{BindGroup, Device};
use std::time::Instant;

use crate::{actions::{ActionSetBundle, XRHands}, models::Model, physics_manager::{PhysicsManager, PhysicsObject}, player::Player};

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
    pub player: Player,
    pub actions: ActionSetBundle,
    world_objects: Vec<GameObject>,
    last_tick: Instant,
}

impl World {
    pub fn new(device: &Device, ground_texture: Arc<BindGroup>, actions: ActionSetBundle) -> World {
        let mut sphere = Model::load(&device, "sphere.glb").expect("Couldn't load sphere model");
        let mut cube = Model::load(&device, "cube.glb").expect("Couldn't load cube model");
        let mut left_hand_model = Model::load(&device, "sphere.glb").expect("Couldn't load left hand model");
        let mut right_hand_model = Model::load(&device, "sphere.glb").expect("Couldn't load right hand model");
        let mut test_model = Model::load(&device, "gun.glb").expect("Couldn't load test model");

        left_hand_model.scale = Vec3::new(0.05, 0.05, 0.05);
        right_hand_model.scale = Vec3::new(0.05, 0.05, 0.05);

        let mut physics_manager = PhysicsManager::new();
        let mut world_objects = Vec::new();

        sphere.scale = Vec3::new(0.5, 0.5, 0.5);
        sphere.position = Vec3::new(0.0, 20.0, 0.0);
        cube.scale = Vec3::new(100.0, 0.1, 100.0);

        test_model.scale = Vec3::new(0.05, 0.05, 0.05);
        test_model.position = Vec3::new(0.0, 2.0, 5.0);

        let test_sphere = GameObject::new(sphere.clone(), &mut physics_manager, false);
        let test_object = GameObject::new(test_model.clone(), &mut physics_manager, true);

        // Hands
        let left_hand = GameObject::new(left_hand_model, &mut physics_manager, true);
        let right_hand = GameObject::new(right_hand_model, &mut physics_manager, true);

        // Create ground
        let mut ground_cube = cube.clone();

        ground_cube.set_mat(ground_texture);
        ground_cube.uv_scale = Vec2::new(100.0, 100.0);

        let ground = GameObject::new(ground_cube, &mut physics_manager, true);

        world_objects.push(test_sphere);
        world_objects.push(test_object);
        world_objects.push(ground);

        let world = World {
            physics_manager,
            models: Vec::new(),
            hands: XRHands::new(),
            headset_on: true,
            headset_on_last: true,
            player: Player::new(left_hand, right_hand),
            actions,
            world_objects,
            last_tick: Instant::now(),
        };

        world
    }

    pub fn tick(&mut self, session: &Session<Vulkan>) {
        // Deltatime
        let now = Instant::now();
        let delta = (now - self.last_tick).as_secs_f32();
        self.last_tick = now;

        if self.hands.left_controls.touchpad { self.actions.rumble(session, true,  40.0,  40); }
        if self.hands.right_controls.touchpad { self.actions.rumble(session, false, 40.0, 40); }

        self.player.update(&mut self.physics_manager, &self.hands, delta);

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
        world.push(&self.player.left_hand.model);
        world.push(&self.player.right_hand.model);

        for model in self.world_objects.iter().collect::<Vec<_>>() {
            world.push(&model.model);
        }

        world
    }
}
