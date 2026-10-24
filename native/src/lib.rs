//! Standalone legal-stage pool plus optional loser counterpick. Experimental.
#[cfg(all(feature = "adapt-native", not(target_arch = "aarch64")))]
compile_error!("adapt-native requires the reviewed AArch64 Skyline target");
#[cfg(all(feature = "adapt-native", target_arch = "aarch64"))]
mod filter;
#[cfg(all(feature = "adapt-native", target_arch = "aarch64"))]
mod hooks;
#[cfg(all(feature = "adapt-native", target_arch = "aarch64"))]
mod imgui_host;
#[cfg(all(feature = "adapt-native", target_arch = "aarch64"))]
mod menu;
#[cfg(all(feature = "adapt-native", target_arch = "aarch64"))]
mod pool;
#[cfg(all(feature = "adapt-native", target_arch = "aarch64"))]
mod settings;
#[skyline::main(name = "elite_stages_counterpick")]
pub fn main() {
    #[cfg(all(feature = "adapt-native", target_arch = "aarch64"))]
    {
        skyline::println!("[CounterpickAdapter] settings: {:?}", settings::startup());
        filter::start();
        hooks::start();
        menu::start();
    }
}
