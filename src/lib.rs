use godot::init::{gdextension, ExtensionLibrary};

mod constants;
mod nodes;
mod types;

struct Game;

// Expose the extension to Godot
#[gdextension]
unsafe impl ExtensionLibrary for Game {}