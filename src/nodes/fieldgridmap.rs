use crate::nodes::PanningCamera;
use crate::nodes::FieldCharacter;
use crate::types::VecTree;

use std::collections::HashMap;
use godot::{builtin::{Array, Vector3, Vector3i}, classes::{GridMap, IGridMap, InputEvent, InputEventMouseButton, Node}, global::MouseButton, obj::{Base, Gd, GdMut, WithBaseField}, prelude::{godot_api, GodotClass}};


// TODO: make your own
#[derive(GodotClass)]
#[class(base=GridMap)]
pub struct FieldGripMap {
    base: Base<GridMap>,
    last_mouse_coords: Option<Vector3i>,
    last_highlight_cell_offset: i32,
    char_refs: HashMap<Vector3i, Gd<FieldCharacter>>,
    focused_char: Option<Gd<FieldCharacter>>,
    focus_highlighted_cells: Vec<Vector3i>, // TODO: Make this a hashmap or tree?

    #[export] pub cam: Option<Gd<PanningCamera>>,
    #[export] pub highlight_offset: i32,
    #[export] pub highlight_move_offset: i32,
    #[export] pub highlight_attack_offset: i32,
    #[export] pub highlight_heal_offset: i32,
    #[export] pub block_type_len: i32,
    #[export] pub slope_index: i32,
}

#[godot_api]
impl IGridMap for FieldGripMap {
    fn init(base: Base<GridMap>) -> Self {
        Self {
            base,
            last_mouse_coords: None,
            last_highlight_cell_offset: -1,
            char_refs: HashMap::new(),
            focused_char: None,
            focus_highlighted_cells: Vec::new(),

            cam: None,
            highlight_offset: 0,
            highlight_move_offset: 0,
            highlight_attack_offset: 0,
            highlight_heal_offset: 0,
            block_type_len: 0,
            slope_index: 0,
        }
    }

    fn ready(&mut self) {
        // Set positions of all child characters
        let children: Array<Gd<Node>> = self.base().get_children();
        for char in children.iter_shared() {
            if char.get_class() == "FieldCharacter".into() {
                let mut char: Gd<FieldCharacter> = char.cast::<FieldCharacter>();
                let field_pos: Vector3i = char.bind().get_field_position();

                char.set_position(self.get_world_pos_from_coords(field_pos));

                self.char_refs.insert(field_pos, char);
            }
        }
    }

    fn unhandled_input(&mut self, event: Gd<InputEvent>) {
        if event.get_class() == "InputEventMouseButton".into() {
            let event: Gd<InputEventMouseButton> = event.cast(); // Cast won't fail due to above check

            if event.get_button_index() == MouseButton::LEFT && event.is_pressed() && self.focused_char == None {
                if let Some(mut pos) = self.last_mouse_coords {
                    let mut move_range: u32 = 0;
                    let mut attack_range: u32 = 0;
                    let mut heal_range: u32 = 0;

                    pos.y += 1; // Block above currently moused

                    // Get currently moused over character
                    if let Some(char_ref) = self.char_refs.get(&pos) {
                        // Since movement range doesn't include the current position, add 1
                        move_range = char_ref.bind().get_movement_range() + 1;
                        attack_range = char_ref.bind().get_attack_range();
                        heal_range = char_ref.bind().get_heal_range();
                        self.set_char_focused(Some(char_ref.clone()));
                    }

                    if move_range > 0 || attack_range > 0 || heal_range > 0 {
                        if let Some(char) = &self.focused_char {
                            self.show_char_ranges(char.clone());
                        }
                    }
                }
            } else if event.get_button_index() == MouseButton::RIGHT && event.is_pressed() && self.focused_char != None {
                // Only exit this when hovered over char
                if let Some(mut pos) = self.last_mouse_coords {
                    pos.y += 1; // Block above currently moused

                    if pos == self.focused_char.as_ref().expect("Cannot fail due to above check").bind().get_field_position() {
                        // Exit out of being focused on a character
                        self.set_char_focused(None);
                        self.clear_char_ranges();
                    }
                }
            } else if event.get_button_index() == MouseButton::LEFT && event.is_pressed() && self.focused_char != None {
                // Move char where appropriate
                if let Some(pos) = self.last_mouse_coords {
                    let mut focused_char: GdMut<'_, FieldCharacter> = self.focused_char.as_mut().expect("Always Some").bind_mut();
                    // TODO: focused_char.set_field_pos_dir(self, pos);
                }
            }
        }
    }

    fn process(&mut self, _: f64) {
        // Mouse pos is calculated every frame for smoothness
        if let Some(cam) = self.get_cam() {
            if let Some(world_pos) = cam.bind().get_world_mouse_pos_option() {
                let mouse_coords: Vector3i = self.get_coords_from_world_pos(world_pos);

                if Some(mouse_coords) != self.last_mouse_coords {
                    if self.focused_char == None { self.last_highlight_cell_offset = 0; } // Block offset can only be 0 if char not focused

                    if let Some(last_mouse_coords) = self.last_mouse_coords {
                        self.set_overlay_block(last_mouse_coords, self.last_highlight_cell_offset);
                    }

                    self.last_highlight_cell_offset = self.base().get_cell_item(mouse_coords) % self.block_type_len;

                    self.set_overlay_block(mouse_coords, self.highlight_offset);

                    self.last_mouse_coords = Some(mouse_coords);
                }
            }
        }
    }
}

#[godot_api]
impl FieldGripMap {
    #[func]
    pub fn get_coords_from_world_pos(&self, world_pos: Vector3) -> Vector3i {
        let local_pos: Vector3 = self.base().to_local(world_pos);
        self.base().local_to_map(local_pos)
    }

    #[func]
    pub fn get_world_pos_from_coords(&self, coords: Vector3i) -> Vector3 {
        let local_pos: Vector3 = self.base().map_to_local(coords);
        self.base().to_global(local_pos)
    }

    #[func]
    pub fn set_overlay_block(&mut self, overlay_coords: Vector3i, highlight_offset: i32) {
        let mut cell_type: i32 = self.base().get_cell_item(overlay_coords);
        cell_type -= cell_type % self.block_type_len;

        // Always preserve orientation
        let orientation: i32 = self.base().get_cell_item_orientation(overlay_coords);

        self.base_mut().set_cell_item_ex(overlay_coords, cell_type + highlight_offset)
            .orientation(orientation).done();
    }

    #[func]
    pub fn map_to_local(&self, coords: Vector3i) -> Vector3 {
        self.base().map_to_local(coords)
    }

    // Change position of character currently on board
    // Does not check whether char_ref at cur_pos is the same
    // Overwrites whatever is at the position
    // Current position needed to avoid binding issues
    #[func]
    pub fn reposition_char_from_pos(&mut self, cur_pos: Vector3i, new_pos: Vector3i) {
        let char_ref = self.char_refs.get(&cur_pos);

        if let Some(char_ref) = char_ref {
            let mut char_ref: Gd<FieldCharacter> = char_ref.clone();

            char_ref.set_position(self.get_world_pos_from_coords(new_pos));

            self.char_refs.remove_entry(&cur_pos);
            self.char_refs.insert(new_pos, char_ref);

            // TODO: rebuild trees?
        }
    }

    pub fn show_range_tree(&mut self, node: &VecTree<Vector3i>, highlight_offset: i32) {
        for child_node in node.children.iter() {
            self.show_range_tree(child_node, highlight_offset);
        }

        // Highlight block under current pos
        let pos: Vector3i = node.value + Vector3i::new(0, -1, 0);
        self.set_overlay_block(pos, highlight_offset);
        self.focus_highlighted_cells.push(pos);
    }

    #[func]
    pub fn show_char_ranges(&mut self, char: Gd<FieldCharacter>) {
        // TODO: Disable healable and attackable if range is 0
        // TODO: Store these trees in field for movement data
        let healable: VecTree<Vector3i> = char.bind().get_range_tree(&self, char.bind().movement_range + char.bind().heal_range);
        self.show_range_tree(&healable, self.highlight_heal_offset);

        let attackable: VecTree<Vector3i> = char.bind().get_range_tree(&self, char.bind().movement_range + char.bind().attack_range);
        self.show_range_tree(&attackable, self.highlight_attack_offset);

        let reachable: VecTree<Vector3i> = char.bind().get_range_tree(&self, char.bind().movement_range);
        self.show_range_tree(&reachable, self.highlight_move_offset);

        // Keep mouse highlight on field
        if let Some(mouse_coords) = self.last_mouse_coords {
            self.last_highlight_cell_offset = self.highlight_move_offset; // Draw movement range under mouse when clicking
            self.set_overlay_block(mouse_coords, self.highlight_offset);
        }
    }

    #[func]
    pub fn clear_char_ranges(&mut self) {
        let last_range_cells: Vec<Vector3i> = self.focus_highlighted_cells.clone();

        for pos in last_range_cells.into_iter() {
            let mut highlight_offset: i32 = 0;

            // Don't clear if block is highlighted currently
            if let Some(last_mouse_coords) = self.last_mouse_coords {
                if pos == last_mouse_coords {
                    self.last_highlight_cell_offset = highlight_offset;
                    highlight_offset += self.highlight_offset;
                }
            }

            self.set_overlay_block(pos, highlight_offset);
        }

        self.focus_highlighted_cells.clear();
    }

    // TODO: Make this show/hide char information
    pub fn set_char_focused(&mut self, val: Option<Gd<FieldCharacter>>) {
        self.focused_char = val;
    }
}