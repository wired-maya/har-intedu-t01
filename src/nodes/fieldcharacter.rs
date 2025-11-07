use crate::types::{VecTree, CharType};
use crate::nodes::FieldGripMap;

use godot::{builtin::Vector3i, classes::{CharacterBody3D, Engine, GridMap, ICharacterBody3D}, obj::{Base, WithBaseField}, prelude::{godot_api, GodotClass}};

#[derive(GodotClass)]
#[class(base=CharacterBody3D)]
pub struct FieldCharacter {
    base: Base<CharacterBody3D>,
    pub movement_tree: VecTree<Vector3i>,
    pub attack_tree: VecTree<Vector3i>,
    pub heal_tree: VecTree<Vector3i>,

    // Create getters/setters
    #[export] #[var(get, set=set_field_pos)] pub field_position: Vector3i,
    #[export] pub chartype: CharType,
    #[export] pub movement_range: u32,
    #[export] pub attack_range: u32,
    #[export] pub heal_range: u32,
}

#[godot_api]
impl ICharacterBody3D for FieldCharacter {
    fn init(base: Base<CharacterBody3D>) -> Self {
        Self {
            base,
            movement_tree: VecTree { value: Vector3i::new(0, 0, 0), children: vec![] },
            attack_tree: VecTree { value: Vector3i::new(0, 0, 0), children: vec![] },
            heal_tree: VecTree { value: Vector3i::new(0, 0, 0), children: vec![] },

            field_position: Vector3i::ZERO,
            chartype: CharType::Enemy,
            movement_range: 1,
            attack_range: 1,
            heal_range: 0,
        }
    }
}

#[godot_api]
impl FieldCharacter {
    // Function to have pos changes when remote debugging or in editor
    #[func]
    fn set_field_pos(&mut self, pos: Vector3i) {
        if Engine::singleton().is_editor_hint() || self.base().is_inside_tree() {
            self.base().get_parent().expect("FieldCharacter should be child of FieldGridMap")
                .try_cast::<FieldGripMap>().expect("FieldCharacter should be child of FieldGridMap")
                .bind_mut().reposition_char_from_pos(self.get_field_position(), pos);
        }
    }

    // TODO: Calculate char ranges at point of movement for each field character and store it there
    // TODO: Add vertical range for if attacking with ranged weapons or using thrusters to go up
    // TODO: Add unit height restrictions so mechs are of height 2 for example
    fn _get_range_tree(&self, field: &FieldGripMap, mut node: VecTree<Vector3i>, remaining_range: u32) -> VecTree<Vector3i> {
        if remaining_range == 0 { return node; } // No more range to probe

        // TODO: Handle slopes, cliffs, etc
        // TODO: Make enemies block as well
        // TODO: iterate over existing chars and remove them from being allowed for movement
        // TODO: Handle being able to fall down a cliff 1 high w/o taking damge, can >1 high by taking damage
        // TODO: Handle bounds/dropoff/etc so you can't move off the field
        // Push the 4 tiles around the current one
        for i in 0..4 {
            let mut value: Vector3i = node.value;

            // Adjust offsets to get tiles around current one
            match i {
                0 => value.x += 1,
                1 => value.x -= 1,
                2 => value.z += 1,
                3 => value.z -= 1,
                _ => {} // This case will never occur
            };

            let mut cell_item: i32 = field.base().get_cell_item(value);
            
            // Add ability to move up slopes
            // TODO: Check orientation
            let block_type_len: i32 = field.block_type_len;
            let slope_index: i32 = field.slope_index;
            if cell_item - (cell_item % block_type_len) == slope_index {
                value.y += 1;
            }

            // Handle moving down
            // TODO: handle damage on height > 1 drop
            // TODO: MAKE SURE THIS ISN'T CALCULATED OUT OF FIELD BOUNDS!
            while field.base().get_cell_item(value + Vector3i::new(0, -1, 0)) == GridMap::INVALID_CELL_ITEM {
                value.y -= 1;
            }

            cell_item = field.base().get_cell_item(value); // Get adjusted value

            // Cannot be on non-empty tiles
            if cell_item == GridMap::INVALID_CELL_ITEM {
                let child_node: VecTree<Vector3i> = VecTree::new(value, vec![]);

                node.children.push(self._get_range_tree(field, child_node, remaining_range - 1));
            }
        }

        node
    }

    pub fn get_range_tree(&self, field: &FieldGripMap, range: u32) -> VecTree<Vector3i> {

        self._get_range_tree(&field, VecTree::new(self.field_position, vec![]), range)
    }

    // Make this find and return position in tree
    fn _is_in_tree(&self, tree: &VecTree<Vector3i>, tree_coords: Vec<u32>, pos: Vector3i) -> Vec<u32> {
        if tree.value == pos {
            return tree_coords;
        }

        for i in 0..tree.children.len() {
            let mut tree_coords: Vec<u32> = tree_coords.clone();
            tree_coords.push(i as u32);

            let child_coords: Vec<u32> = self._is_in_tree(tree, tree_coords, pos);

            if !child_coords.is_empty() { return child_coords; }
        }

        return vec![];
    }
}