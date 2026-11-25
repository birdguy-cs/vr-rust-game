use glam::{Quat, Vec2, Vec3};

pub struct ActionSetBundle {
    pub action_set: openxr::ActionSet,
    pub left_action: openxr::Action<openxr::Posef>,
    pub right_action: openxr::Action<openxr::Posef>,
    pub left_space: openxr::Space,
    pub right_space: openxr::Space,
    pub trigger: openxr::Action<f32>,
    pub grip: openxr::Action<f32>,
    pub primary: openxr::Action<bool>,
    pub secondary: openxr::Action<bool>,
    pub touchpad: openxr::Action<bool>,
    pub stick: openxr::Action<openxr::Vector2f>,
    pub haptic: openxr::Action<openxr::Haptic>,
    pub left_path: openxr::Path,
    pub right_path: openxr::Path,
}

impl ActionSetBundle {
    pub fn rumble(&self, session: &openxr::Session<openxr::Vulkan>, left: bool, amplitude: f32, millis: i64) {
        let path = if left { self.left_path } else { self.right_path };
        let _ = self.haptic.apply_feedback(
            session,
            path,
            &openxr::HapticVibration::new()
                .amplitude(amplitude)
                .frequency(openxr::FREQUENCY_UNSPECIFIED)
                .duration(openxr::Duration::from_nanos(millis * 1_000_000)),
        );
    }
}

// Add more as wanted / needed
pub fn create_action_set(
    xr_instance: &openxr::Instance,
    xr_session: &openxr::Session<openxr::Vulkan>,
) -> ActionSetBundle {
    let action_set = xr_instance
        .create_action_set("input", "input pose information", 0)
        .unwrap();

    let left_path = xr_instance.string_to_path("/user/hand/left").unwrap();
    let right_path = xr_instance.string_to_path("/user/hand/right").unwrap();
    let hands = [left_path, right_path];

    // Poses
    let left_action = action_set
        .create_action::<openxr::Posef>("left_hand", "Left Hand Controller", &[])
        .unwrap();
    let right_action = action_set
        .create_action::<openxr::Posef>("right_hand", "Right Hand Controller", &[])
        .unwrap();

    // Haptic feedback
    let haptic = action_set.create_action::<openxr::Haptic>("haptic", "Haptic feedback", &hands).unwrap();

    // Buttons / analog, one action per input, usable on both hands
    let trigger = action_set.create_action::<f32>("trigger", "Trigger", &hands).unwrap();
    let grip = action_set.create_action::<f32>("grip", "Grip", &hands).unwrap();
    let primary = action_set.create_action::<bool>("primary", "Primary button", &hands).unwrap();
    let secondary = action_set.create_action::<bool>("secondary", "Secondary button", &hands).unwrap();
    let touchpad = action_set.create_action::<bool>("touchpad", "Touchpad", &hands).unwrap();
    let stick = action_set.create_action::<openxr::Vector2f>("stick", "Thumbstick", &hands).unwrap();

    let p = |s: &str| xr_instance.string_to_path(s).unwrap();

    // Pose bindings reused for every profile
    let pose_bindings = |profile: &str, extra: &[openxr::Binding]| {
        let mut b = vec![
            openxr::Binding::new(&left_action, p("/user/hand/left/input/grip/pose")),
            openxr::Binding::new(&right_action, p("/user/hand/right/input/grip/pose")),
        ];
        b.extend_from_slice(extra);
        xr_instance
            .suggest_interaction_profile_bindings(p(profile), &b)
            .unwrap();
    };

    // Simple controller fallback (poses only)
    pose_bindings("/interaction_profiles/khr/simple_controller", &[]);

    // Valve Index
    pose_bindings(
        "/interaction_profiles/valve/index_controller",
        &[
            openxr::Binding::new(&trigger, p("/user/hand/left/input/trigger/value")),
            openxr::Binding::new(&trigger, p("/user/hand/right/input/trigger/value")),
            openxr::Binding::new(&grip, p("/user/hand/left/input/squeeze/value")),
            openxr::Binding::new(&grip, p("/user/hand/right/input/squeeze/value")),
            openxr::Binding::new(&primary, p("/user/hand/left/input/a/click")),
            openxr::Binding::new(&primary, p("/user/hand/right/input/a/click")),
            openxr::Binding::new(&secondary, p("/user/hand/left/input/b/click")),
            openxr::Binding::new(&secondary, p("/user/hand/right/input/b/click")),
            openxr::Binding::new(&touchpad, p("/user/hand/left/input/trackpad/touch")),
            openxr::Binding::new(&touchpad, p("/user/hand/right/input/trackpad/touch")),
            openxr::Binding::new(&stick, p("/user/hand/left/input/thumbstick")),
            openxr::Binding::new(&stick, p("/user/hand/right/input/thumbstick")),
            openxr::Binding::new(&haptic, p("/user/hand/left/output/haptic")),
            openxr::Binding::new(&haptic, p("/user/hand/right/output/haptic")),
        ],
    );

    // Oculus / Meta Touch
    pose_bindings(
        "/interaction_profiles/oculus/touch_controller",
        &[
            openxr::Binding::new(&trigger, p("/user/hand/left/input/trigger/value")),
            openxr::Binding::new(&trigger, p("/user/hand/right/input/trigger/value")),
            openxr::Binding::new(&grip, p("/user/hand/left/input/squeeze/value")),
            openxr::Binding::new(&grip, p("/user/hand/right/input/squeeze/value")),
            openxr::Binding::new(&primary, p("/user/hand/left/input/x/click")),
            openxr::Binding::new(&primary, p("/user/hand/right/input/a/click")),
            openxr::Binding::new(&secondary, p("/user/hand/left/input/y/click")),
            openxr::Binding::new(&secondary, p("/user/hand/right/input/b/click")),
            openxr::Binding::new(&touchpad, p("/user/hand/left/input/thumbrest/touch")),
            openxr::Binding::new(&touchpad, p("/user/hand/right/input/thumbrest/touch")),
            openxr::Binding::new(&stick, p("/user/hand/left/input/thumbstick")),
            openxr::Binding::new(&stick, p("/user/hand/right/input/thumbstick")),
            openxr::Binding::new(&haptic, p("/user/hand/left/output/haptic")),
            openxr::Binding::new(&haptic, p("/user/hand/right/output/haptic")),
        ],
    );

    xr_session.attach_action_sets(&[&action_set]).unwrap();

    let left_space = left_action
        .create_space(xr_session, openxr::Path::NULL, openxr::Posef::IDENTITY)
        .unwrap();
    let right_space = right_action
        .create_space(xr_session, openxr::Path::NULL, openxr::Posef::IDENTITY)
        .unwrap();

    ActionSetBundle {
        action_set,
        left_action,
        right_action,
        left_space,
        right_space,
        trigger,
        grip,
        primary,
        secondary,
        touchpad,
        stick,
        haptic,
        left_path,
        right_path,
    }
}

pub struct HandControls {
    pub primary_button: bool,
    pub secondary_button: bool,
    pub touchpad: bool,
    pub grip: f32,
    pub trigger: f32,
    pub thumbstick: Vec2,
}

impl HandControls {
    pub fn new() -> HandControls {
        HandControls {
            primary_button: false,
            secondary_button: false,
            touchpad: false,
            grip: 0.0,
            trigger: 0.0,
            thumbstick: Vec2::ZERO,
        }
    }
}

pub struct XRHands {
    pub left_position: Vec3,
    pub left_rotation: Quat,
    pub left_controls: HandControls,

    pub right_position: Vec3,
    pub right_rotation: Quat,
    pub right_controls: HandControls,
}

impl XRHands {
    pub fn new() -> XRHands {
        XRHands {
            left_position: Vec3::ZERO,
            left_rotation: Quat::IDENTITY,
            left_controls: HandControls::new(),

            right_position: Vec3::ZERO,
            right_rotation: Quat::IDENTITY,
            right_controls: HandControls::new(),
        }
    }
}

pub fn get_hands(
    xr_session: &openxr::Session<openxr::Vulkan>,
    action_set_bundle: &ActionSetBundle,
    xr_stage: &openxr::Space,
    xr_frame_state: &openxr::FrameState,
) -> XRHands {
    let active_action_set = (&action_set_bundle.action_set).into();
    xr_session.sync_actions(&[active_action_set]).unwrap();

    // Find where our controllers are located in the Stage space
    let left_location = action_set_bundle
        .left_space
        .locate(xr_stage, xr_frame_state.predicted_display_time)
        .unwrap();

    let right_location = action_set_bundle
        .right_space
        .locate(xr_stage, xr_frame_state.predicted_display_time)
        .unwrap();

    // Get the controls of the controllers
    let left_trigger = action_set_bundle.trigger.state(&xr_session, action_set_bundle.left_path).unwrap();
    let left_grip = action_set_bundle.grip.state(&xr_session, action_set_bundle.left_path).unwrap();

    let right_trigger = action_set_bundle.trigger.state(&xr_session, action_set_bundle.right_path).unwrap();
    let right_grip = action_set_bundle.grip.state(&xr_session, action_set_bundle.right_path).unwrap();

    let right_primary = action_set_bundle.primary.state(&xr_session, action_set_bundle.right_path).unwrap();
    let right_secondary = action_set_bundle.secondary.state(&xr_session, action_set_bundle.right_path).unwrap();

    let left_primary = action_set_bundle.primary.state(&xr_session, action_set_bundle.left_path).unwrap();
    let left_secondary = action_set_bundle.secondary.state(&xr_session, action_set_bundle.left_path).unwrap();

    let left_touchpad = action_set_bundle.touchpad.state(&xr_session, action_set_bundle.left_path).unwrap();
    let right_touchpad = action_set_bundle.touchpad.state(&xr_session, action_set_bundle.right_path).unwrap();

    let ls = action_set_bundle.stick.state(&xr_session, action_set_bundle.left_path).unwrap().current_state;
    let rs = action_set_bundle.stick.state(&xr_session, action_set_bundle.right_path).unwrap().current_state;

    let mut hands = XRHands::new();
    if action_set_bundle
        .left_action
        .is_active(xr_session, openxr::Path::NULL)
        .unwrap()
    {
        hands.left_position = Vec3::new(left_location.pose.position.x, left_location.pose.position.y, left_location.pose.position.z);
        hands.left_rotation = Quat::from_xyzw(
            left_location.pose.orientation.x,
            left_location.pose.orientation.y,
            left_location.pose.orientation.z,
            left_location.pose.orientation.w,
        );

        let left_primary = left_primary.is_active && left_primary.current_state;
        let left_secondary = left_secondary.is_active && left_secondary.current_state;
        let left_touchpad = left_touchpad.is_active && left_touchpad.current_state;
        let left_grip = left_grip.current_state;
        let left_trigger = left_trigger.current_state;

        hands.left_controls = HandControls {
            primary_button: left_primary,
            secondary_button: left_secondary,
            touchpad: left_touchpad,
            grip: left_grip,
            trigger: left_trigger,
            thumbstick: Vec2::new(ls.x, ls.y),
        }
    }

    if action_set_bundle
        .right_action
        .is_active(xr_session, openxr::Path::NULL)
        .unwrap()
    {
        hands.right_position = Vec3::new(right_location.pose.position.x, right_location.pose.position.y, right_location.pose.position.z);
        hands.right_rotation = Quat::from_xyzw(
            right_location.pose.orientation.x,
            right_location.pose.orientation.y,
            right_location.pose.orientation.z,
            right_location.pose.orientation.w,
        );

        let right_primary = right_primary.is_active && right_primary.current_state;
        let right_secondary = right_secondary.is_active && right_secondary.current_state;
        let right_touchpad = right_touchpad.is_active && right_touchpad.current_state;
        let right_grip = right_grip.current_state;
        let right_trigger = right_trigger.current_state;

        hands.right_controls = HandControls {
            primary_button: right_primary,
            secondary_button: right_secondary,
            touchpad: right_touchpad,
            grip: right_grip,
            trigger: right_trigger,
            thumbstick: Vec2::new(rs.x, rs.y),
        }
    }

    hands
}
