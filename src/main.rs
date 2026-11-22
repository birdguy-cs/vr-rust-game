#![windows_subsystem = "windows"]
// TODO: Figure out how to compile and get this game running on Android OpenXR
// TODO: Clean up android specific changes

mod actions;
mod math;
mod rendering;
mod session;
mod world;
mod models;
#[cfg(not(target_os = "android"))]
mod window;
mod logging;
mod physics_manager;
mod player;

#[cfg(target_os = "android")]
type AppHandle = android_activity::AndroidApp;
#[cfg(not(target_os = "android"))]
type AppHandle = ();

use world::World;

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use anyhow::{Error};

use crate::rendering::{RenderContext, create_depth_buffer};

const VIEW_TYPE: openxr::ViewConfigurationType = openxr::ViewConfigurationType::PRIMARY_STEREO;

const VIEW_COUNT: u32 = 2;

#[cfg(not(target_os = "android"))]
pub fn main() -> Result<(), Error> { run(()) }

#[allow(unused_variables)]
pub fn run(app: AppHandle) -> Result<(), Error> {
    #[cfg(target_os = "android")]
    let mut android_state = AndroidState::default();
    #[cfg(target_os = "android")]
    while !(android_state.resumed && android_state.has_window) {
        pump_android(&app, Duration::from_millis(100), &mut android_state);
        if android_state.destroy { return Ok(()); }
    }

    #[cfg(not(target_os = "android"))]
    let ctrlc_request_exit = create_ctrlc_handler();
    #[cfg(target_os = "android")]
    let ctrlc_request_exit = Arc::new(AtomicBool::new(false));

    #[cfg(not(target_os = "android"))]
    let xr_entry = openxr::Entry::linked();
    #[cfg(target_os = "android")]
    let xr_entry = unsafe { openxr::Entry::load() }
        .expect("Couldn't load libopenxr_loader.so");

    #[cfg(target_os = "android")]
    if let Err(e) = xr_entry.initialize_android_loader() {
        popup_error!("Failed to initialize OpenXR Android loader: {:?}", e);
        // Return an Err variant to match your function signature!
        return Err(anyhow::anyhow!("Android loader failed: {:?}", e).into());
    }

    let xr_instance = create_openxr_instance(&xr_entry)?;
    let _debug_utils = indite::DebugUtils::new(&xr_entry, &xr_instance);

    // Request a form factor from the device (HMD, Handheld, etc.)
    let xr_system = xr_instance
        .system(openxr::FormFactor::HEAD_MOUNTED_DISPLAY)
        .unwrap();

    // Check what blend mode is valid for this device (opaque vs transparent displays). We'll just
    // take the first one available!
    let environment_blend_mode = xr_instance
        .enumerate_environment_blend_modes(xr_system, VIEW_TYPE)
        .unwrap()[0];

    let mut render_context = RenderContext::new(&xr_instance, xr_system);
    let mut session_bundle = session::create_session(&xr_instance, xr_system, &render_context);
    let action_set_bundle = actions::create_action_set(&xr_instance, &session_bundle.session);

    let views = xr_instance
        .enumerate_view_configuration_views(
            xr_system,
            openxr::ViewConfigurationType::PRIMARY_STEREO,
        )
        .unwrap();

    let width = views[0].recommended_image_rect_width;
    let height = views[0].recommended_image_rect_height;

    render_context.depth_buffer = Some(create_depth_buffer(&render_context.device, width, height));

    // Main loop
    let mut event_storage = openxr::EventDataBuffer::new();
    let mut session_running = false;

    // Create the game world and the window
    let ground_mat = render_context.load_material("grass-3937-in-architextures.jpg");

    let mut game_world = World::new(&render_context.device, ground_mat);
    #[cfg(not(target_os = "android"))]
    let window: Option<window::Window> = Some(window::create_window());
    #[cfg(target_os = "android")]
    let window: Option<String> = None;

    loop {
        if !handle_ctrlc(&ctrlc_request_exit, &session_bundle.session) {
            break;
        }
    
        #[cfg(target_os = "android")]
        {
            let timeout = if session_running { Duration::ZERO } else { Duration::from_millis(100) };
            pump_android(&app, timeout, &mut android_state);
            if android_state.destroy { break; }
        }
    
        if !handle_instance_events(
            &xr_instance,
            &session_bundle.session,
            &mut event_storage,
            &mut session_running,
        ) {
            break;
        }
    
        #[cfg(not(target_os = "android"))]
        if let Some(window) = &window {
            if window.exit.load(Ordering::Relaxed) {
                break;
            }
        }
    
        if !session_running {
            #[cfg(not(target_os = "android"))]
            std::thread::sleep(Duration::from_millis(100));
            continue;
        }
    
        game_world.tick();

        rendering::render_frame(
            environment_blend_mode,
            &render_context,
            &mut session_bundle,
            &action_set_bundle,
            &mut game_world,
            #[cfg(not(target_os = "android"))]
            &window,
        );
    }

    #[cfg(not(target_os = "android"))]
    if let Some(window) = window {
        window.close();
    }
    println!("exiting cleanly");
    Ok(())
}

fn create_openxr_instance(xr_entry: &openxr::Entry) -> Result<openxr::Instance, Error> {
    // OpenXR will fail to initialize if we ask for an extension that OpenXR can't provide! So we
    // need to check all our extensions before initializing OpenXR with them. Note that even if the
    // extension is present, it's still possible you may not be able to use it. For example: the
    // hand tracking extension may be present, but the hand sensor might not be plugged in or turned
    // on. There are often additional checks that should be made before using certain features!
    let available_extensions = match xr_entry.enumerate_extensions() {
        Ok(exts) => exts,
        Err(openxr::sys::Result::ERROR_RUNTIME_UNAVAILABLE) => {
            popup_bail!("No active OpenXR runtime found.");
        }
        Err(e) => {
            popup_bail!("Failed to enumerate extensions due to an unexpected error: {:?}", e);
        }
    };

    // If a required extension isn't present, you want to ditch out here! It's possible something
    // like your rendering API might not be provided by the active runtime. APIs like OpenGL don't
    // have universal support.
    if !available_extensions.khr_vulkan_enable2 {
        popup_bail!("Vulkan OpenXR extension not available");
    }

    let mut enabled_extensions = openxr::ExtensionSet::default();
    enabled_extensions.khr_vulkan_enable2 = true;

    #[cfg(target_os = "android")]
    {
        if !available_extensions.khr_android_create_instance {
            popup_bail!("XR_KHR_android_create_instance not available");
        }
        enabled_extensions.khr_android_create_instance = true;
    }

    enabled_extensions.ext_debug_utils = true;

    // TODO: Only enable debugging when given debugging flags
    // let core_validation_layer_name = "XR_APILAYER_LUNARG_core_validation";

    let xr_instance = xr_entry
        .create_instance(
            &openxr::ApplicationInfo {
                application_name: "Epic cool vr game",
                application_version: 0,
                engine_name: "FVR engine",
                engine_version: 0,
                api_version: openxr::Version::new(1, 0, 0),
            },
            &enabled_extensions,
            &[],
        )
        .unwrap();
    let instance_props = xr_instance.properties().unwrap();
    println!(
        "loaded openxr runtime: {} {}",
        instance_props.runtime_name, instance_props.runtime_version
    );

    Ok(xr_instance)
}

fn create_ctrlc_handler() -> Arc<AtomicBool> {
    let ctrlc_request_exit = Arc::new(AtomicBool::new(false));
    let r = ctrlc_request_exit.clone();

    let handler = move || r.store(true, Ordering::Relaxed);
    ctrlc::set_handler(handler).unwrap();

    ctrlc_request_exit
}

fn handle_ctrlc(
    ctrlc_request_exit: &Arc<AtomicBool>,
    xr_session: &openxr::Session<openxr::Vulkan>,
) -> bool {
    // Check for ctrl-c
    if ctrlc_request_exit.load(Ordering::Relaxed) {
        println!("ctrl-c requesting exit");

        // The OpenXR runtime may want to perform a smooth transition between scenes, so we
        // can't necessarily exit instantly. Instead, we must notify the runtime of our
        // intent and wait for it to tell us when we're actually done.
        match xr_session.request_exit() {
            Ok(()) => {}
            Err(openxr::sys::Result::ERROR_SESSION_NOT_RUNNING) => return false,
            Err(e) => popup_panic!("{}", e),
        }
    }

    true
}

fn handle_instance_events(
    xr_instance: &openxr::Instance,
    xr_session: &openxr::Session<openxr::Vulkan>,
    event_storage: &mut openxr::EventDataBuffer,
    session_running: &mut bool,
) -> bool {
    while let Some(event) = xr_instance.poll_event(event_storage).unwrap() {
        match event {
            openxr::Event::SessionStateChanged(e) => {
                // Session state change is where we can begin and end sessions, as well as
                // find quit messages!
                // println!("entered state {:?}", e.state());
                match e.state() {
                    openxr::SessionState::READY => {
                        xr_session.begin(VIEW_TYPE).unwrap();
                        *session_running = true;
                    }
                    openxr::SessionState::STOPPING => {
                        xr_session.end().unwrap();
                        *session_running = false;
                    }
                    openxr::SessionState::EXITING | openxr::SessionState::LOSS_PENDING => {
                        return false;
                    }
                    _ => {}
                }
            }
            openxr::Event::InstanceLossPending(_) => {
                return false;
            }
            openxr::Event::EventsLost(e) => {
                println!("lost {} events", e.lost_event_count());
            }
            _ => {}
        }
    }

    true
}

// Oculus / OpenXR support
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: android_activity::AndroidApp) {
    if let Err(e) = run(app) {
        eprintln!("Game crashed: {e:?}");
    }
}

#[cfg(target_os = "android")]
fn pump_android(app: &android_activity::AndroidApp, timeout: Duration, state: &mut AndroidState) {
    use android_activity::{InputStatus, MainEvent, PollEvent};

    app.poll_events(Some(timeout), |ev| {
        if let PollEvent::Main(main_ev) = ev {
            match main_ev {
                MainEvent::Resume { .. } => state.resumed = true,
                MainEvent::Pause => state.resumed = false,
                MainEvent::InitWindow { .. } => state.has_window = true,
                MainEvent::TerminateWindow { .. } => state.has_window = false,
                MainEvent::Destroy => state.destroy = true,
                _ => {}
            }
        }
    });

    // Drain input so the system doesn't think you're hung.
    // android-activity 0.6+ (iterator API):
    if let Ok(mut iter) = app.input_events_iter() {
        while iter.next(|_event| InputStatus::Unhandled) {}
    }
    // android-activity <= 0.5 instead:
    // app.input_events(|_e| InputStatus::Unhandled);
}

#[cfg(target_os = "android")]
#[derive(Default)]
struct AndroidState { resumed: bool, has_window: bool, destroy: bool }

#[cfg(target_os = "android")]
fn main() {}

// TODO: More work on openxr for android
/*
 adb logcat *:E | grep -i "openxr\|wgpu\|rust\|VR_GAME"
*/
