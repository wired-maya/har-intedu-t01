use godot::{classes::{INode, InputEvent, Node, Viewport}, obj::{Base, Gd, WithBaseField}, prelude::{godot_api, GodotClass}};

// Passes input to provided viewport
#[derive(GodotClass)]
#[class(base=Node)]
pub struct InputPassNode {
    base: Base<Node>,

    #[export] child_viewport: Option<Gd<Viewport>>,
}

#[godot_api]
impl INode for InputPassNode {
    fn init(base: Base<Node>) -> Self {
        Self {
            base,

            child_viewport: None,
        }
    }

    fn input(&mut self, event: Gd<InputEvent>) {
        self.handle_input(event);
    }

    fn unhandled_input(&mut self, event: Gd<InputEvent>) {
        self.handle_input(event);
    }
}

#[godot_api]
impl InputPassNode {
    #[func]
    fn handle_input(&mut self, event: Gd<InputEvent>) {
        if let Some(mut viewport) = self.get_child_viewport() {
            // Cannot use && with if/let
            // TODO: Don't have to manually do division here dummy
            if self.base().is_inside_tree() {
                // If attached viewport has different resolution, adjust mouse position to
                // one in new viewport's resolution
                viewport.push_input_ex(event).in_local_coords(true).done();
            }
        }
    }
}