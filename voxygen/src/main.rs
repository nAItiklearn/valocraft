#![deny(unsafe_code)]
#![recursion_limit = "2048"]

#[cfg(all(
    target_os = "windows",
    not(feature = "tracy-memory"),
    not(feature = "hot-egui"),
    not(feature = "hot-anim"),
))]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

// Allow profiling allocations with Tracy
#[cfg_attr(feature = "tracy-memory", global_allocator)]
#[cfg(feature = "tracy-memory")]
static GLOBAL: common_base::tracy_client::ProfiledAllocator<std::alloc::System> =
    common_base::tracy_client::ProfiledAllocator::new(std::alloc::System, 128);

use clap::Parser;

fn main() {
    let args = veloren_voxygen::cli::Args::parse();
    veloren_voxygen::entry::start(args, None);
}
