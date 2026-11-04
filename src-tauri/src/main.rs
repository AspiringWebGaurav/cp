// Prevents console window on Windows across ALL build profiles (GPN architecture)
#![windows_subsystem = "windows"]

fn main() {
    cp_lib::run();
}
