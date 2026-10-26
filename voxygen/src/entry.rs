#[cfg(feature = "singleplayer")]
use crate::singleplayer::SingleplayerState;
use crate::{
    GlobalState,
    audio::AudioFrontend,
    cli, panic_handler,
    profile::Profile,
    run,
    scene::terrain::SpriteRenderContext,
    settings::{AudioOutput, Settings, get_fps},
    window::{EventLoop, Window},
};
use i18n::{self, LocalizationHandle};

#[cfg(feature = "egui-ui")]
use crate::ui::egui::EguiState;
use chrono::Utc;
use common::clock::Clock;
use std::{panic, path::PathBuf};
use tracing::{info, warn};
use wgpu::{Backends, Instance};

/// Shared startup path for the desktop binary and the Android native activity.
///
/// `android_event_loop` is supplied by `android_main`; desktop callers pass
/// `None` and let `Window::new` create the event loop.
pub fn start(args: cli::Args, android_event_loop: Option<EventLoop>) {
    if let Some(command) = args.command {
        match command {
            cli::Commands::ListWgpuBackends => {
                #[cfg(target_os = "windows")]
                let backends = &["opengl", "dx12", "vulkan"];
                #[cfg(not(any(target_os = "windows", target_os = "macos")))]
                let backends = &["opengl", "vulkan"];
                #[cfg(target_os = "macos")]
                let backends = &["metal"];

                for backend in backends {
                    println!("{backend}");
                }
                return;
            },
            cli::Commands::ListWgpuDevices => {
                let adapters = Instance::new(&wgpu::InstanceDescriptor::from_env_or_default())
                    .enumerate_adapters(Backends::default());
                for adapter in adapters {
                    println!("{}", adapter.get_info().name);
                }
                return;
            },
        }
    }

    #[cfg(feature = "tracy")]
    common_base::tracy_client::Client::start();

    let userdata_dir = common_base::userdata_dir();

    // Determine where Voxygen's logs should go
    // Choose a path to store the logs by the following order:
    //  - The VOXYGEN_LOGS environment variable
    //  - The <userdata>/voxygen/logs
    let logs_dir = std::env::var_os("VOXYGEN_LOGS")
        .map(PathBuf::from)
        .unwrap_or_else(|| userdata_dir.join("voxygen").join("logs"));

    // Init logging and hold the guards.
    let now = Utc::now();
    let log_filename = format!("{}_voxygen.log", now.format("%Y-%m-%d"));
    let _guards = common_frontend::init_stdout(Some((&logs_dir, &log_filename)));

    // Re-run userdata selection so any warnings will be logged
    common_base::userdata_dir();

    info!("Using userdata dir at: {}", userdata_dir.display());

    // Determine Voxygen's config directory either by env var or placed in veloren's
    // userdata folder
    let config_dir = std::env::var_os("VOXYGEN_CONFIG")
        .map(PathBuf::from)
        .and_then(|path| {
            if path.exists() {
                Some(path)
            } else {
                warn!(?path, "VOXYGEN_CONFIG points to invalid path.");
                None
            }
        })
        .unwrap_or_else(|| userdata_dir.join("voxygen"));
    info!("Using config dir at: {}", config_dir.display());

    // Load the settings
    let mut settings = Settings::load(&config_dir);
    settings.display_warnings();

    panic_handler::set_panic_hook(log_filename, logs_dir);

    // Setup tokio runtime
    use common::consts::MIN_RECOMMENDED_TOKIO_THREADS;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use tokio::runtime::Builder;

    // TODO: evaluate std::thread::available_concurrency as a num_cpus replacement
    let cores = num_cpus::get();
    let tokio_runtime = Arc::new(
        Builder::new_multi_thread()
            .enable_all()
            .worker_threads((cores / 4).max(MIN_RECOMMENDED_TOKIO_THREADS))
            .thread_name_fn(|| {
                static ATOMIC_ID: AtomicUsize = AtomicUsize::new(0);
                let id = ATOMIC_ID.fetch_add(1, Ordering::SeqCst);
                format!("tokio-voxygen-{}", id)
            })
            .build()
            .unwrap(),
    );

    // Initialise watcher for animation hot-reloading
    #[cfg(feature = "hot-anim")]
    {
        anim::init();
    }

    // Initialise watcher for egui hot-reloading
    #[cfg(feature = "hot-egui")]
    {
        voxygen_egui::init();
    }

    // Setup audio
    let mut audio = match settings.audio.output {
        AudioOutput::Off => AudioFrontend::no_audio(),
        AudioOutput::Automatic => AudioFrontend::new(
            settings.audio.num_sfx_channels,
            settings.audio.num_ui_channels,
            settings.audio.subtitles,
            // settings.audio.combat_music_enabled,
            false, // We're disabling combat music for now
            settings.audio.buffer_size.samples,
            settings.audio.sample_rate,
        ),
    };

    audio.set_master_volume(settings.audio.master_volume.get_checked());
    audio.set_music_volume(settings.audio.music_volume.get_checked());
    audio.set_sfx_volume(settings.audio.sfx_volume.get_checked());
    audio.set_instrument_volume(settings.audio.instrument_volume.get_checked());
    audio.set_ambience_volume(settings.audio.ambience_volume.get_checked());
    audio.set_music_spacing(settings.audio.music_spacing);

    // Load the profile.
    let profile = Profile::load(&config_dir);

    let mut i18n =
        LocalizationHandle::load(&settings.language.selected_language).unwrap_or_else(|error| {
            let selected_language = &settings.language.selected_language;
            warn!(
                ?error,
                ?selected_language,
                "Impossible to load language: change to the default language (English) instead.",
            );
            i18n::REFERENCE_LANG.clone_into(&mut settings.language.selected_language);
            LocalizationHandle::load_expect(&settings.language.selected_language)
        });
    i18n.set_english_fallback(settings.language.use_english_fallback);

    // Create window
    use crate::{error::Error, render::RenderError};
    let window_result = if let Some(event_loop) = android_event_loop {
        Window::new_with_event_loop(&settings, &tokio_runtime, event_loop)
    } else {
        Window::new(&settings, &tokio_runtime)
    };
    let (mut window, event_loop) = match window_result {
        Ok(ok) => ok,
        // Custom panic message when a graphics backend could not be found
        Err(Error::RenderError(RenderError::CouldNotFindAdapter)) => {
            #[cfg(target_os = "windows")]
            const POTENTIAL_FIX: &str =
                " Updating the graphics drivers on this system may resolve this issue.";
            #[cfg(target_os = "macos")]
            const POTENTIAL_FIX: &str = "";
            #[cfg(not(any(target_os = "windows", target_os = "macos")))]
            const POTENTIAL_FIX: &str =
                " Installing or updating vulkan drivers may resolve this issue.";

            panic!(
                "Failed to select a rendering backend! No compatible backends were found. We \
                 currently support vulkan, metal, dx12, and opengl.{} If the issue persists, \
                 please include the operating system and GPU details in your bug report to help \
                 us identify the cause.",
                POTENTIAL_FIX
            );
        },
        Err(error) => panic!("Failed to create window!: {:?}", error),
    };

    let clipboard = crate::ui::ice::Clipboard::connect(window.window());

    let lazy_init = SpriteRenderContext::new(window.renderer_mut());

    #[cfg(feature = "egui-ui")]
    let egui_state = EguiState::new(&window);

    #[cfg(feature = "discord")]
    let discord = if settings.networking.enable_discord_integration {
        crate::discord::Discord::start(&tokio_runtime)
    } else {
        crate::discord::Discord::Inactive
    };

    let global_state = GlobalState {
        userdata_dir,
        config_dir,
        audio,
        profile,
        window,
        tokio_runtime,
        #[cfg(feature = "egui-ui")]
        egui_state,
        lazy_init,
        clock: Clock::new(std::time::Duration::from_secs_f64(
            1.0 / get_fps(settings.graphics.max_fps) as f64,
        )),
        settings,
        info_message: None,
        #[cfg(feature = "singleplayer")]
        singleplayer: SingleplayerState::None,
        auto_boot_character: false,
        i18n,
        clipboard,
        clear_shadows_next_frame: false,
        #[cfg(feature = "discord")]
        discord,
        args: args.clone(),
    };

    run::run(global_state, event_loop).unwrap();
}
