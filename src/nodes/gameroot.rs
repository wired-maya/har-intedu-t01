use godot::{classes::{INode, Node}, obj::Base, prelude::{godot_api, GodotClass}};

#[derive(GodotClass)]
#[class(base=Node)]
pub struct GameRoot {
    base: Base<Node>,
}

#[godot_api]
impl INode for GameRoot {
    fn init(base: Base<Node>) -> Self {
        Self {
            base,
        }
    }

    // Set global properties for the game here
    fn ready(&mut self) {

    }
}