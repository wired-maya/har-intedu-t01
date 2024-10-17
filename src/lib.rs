use std::collections::HashMap;

use godot::{builtin::{math::ApproxEq, Array, Basis, Dictionary, EulerOrder, GString, Plane, Quaternion, Rect2, StringName, Variant, Vector2, Vector2i, Vector3, Vector3i}, classes::{Camera3D, CanvasItem, CharacterBody3D, ColorRect, Engine, GridMap, ICamera3D, ICharacterBody3D, IColorRect, IGridMap, INode, ISubViewport, InputEvent, InputEventMouse, InputEventMouseButton, InputEventMouseMotion, Material, Node, PhysicsDirectSpaceState3D, PhysicsRayQueryParameters3D, PhysicsServer3D, ShaderMaterial, SubViewport, Time, Viewport}, global::{deg_to_rad, godot_print, MouseButton}, init::{gdextension, ExtensionLibrary}, meta::FromGodot, obj::{Base, Gd, WithBaseField}, prelude::{godot_api, Export, GodotClass, GodotConvert, Var}};

mod constants;
use constants::*;

struct Game;

#[gdextension]
unsafe impl ExtensionLibrary for Game {}

#[derive(GodotClass)]
#[class(base=Node)]
struct GameRoot {
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

// TODO: Rotation around current pos on plane, scrolling is distance from that point
// TODO: Basically use pivot where pivot is clamped to bounds but not the actual cam
#[derive(GodotClass)]
#[class(base=Camera3D)]
struct PanningCamera {
    // TODO: reorder to make more sense
    base: Base<Camera3D>,
    screen_last_pos: Vector2,
    mouse_last_pos: Vector3,
    panning: bool,
    mouse_world_intersection: Dictionary,
    last_space_state: Option<Gd<PhysicsDirectSpaceState3D>>,
    last_cam_pos: Vector3,
    cam_pos_diff: Vector3,
    centre_pos: Vector3,
    orbit_pos: Quaternion,
    orbiting: bool,
    orbit_mouse_last_pos: Quaternion,

    #[export] plane: Plane,
    #[export] bounds: Rect2, // 0,0 rect means no bounds
    #[export] zoom_step: f32,
    #[export] zoom_max: f32,
    #[export] zoom_min: f32,
    #[export] #[var(get, set = set_zoom)] zoom: f32,
    #[export] uniform_shader_canvas_item: Option<Gd<CanvasItem>>, // Information is set this shader for processing
}

#[godot_api]
impl ICamera3D for PanningCamera {
    fn init(base: Base<Camera3D>) -> Self {
        Self {
            base,
            screen_last_pos: Vector2::ZERO,
            mouse_last_pos: Vector3::ZERO,
            panning: false,
            mouse_world_intersection: Dictionary::new(),
            last_space_state: None,
            last_cam_pos: Vector3::ZERO,
            cam_pos_diff: Vector3::ZERO,
            centre_pos: Vector3::ZERO,
            orbit_pos: Quaternion::from_axis_angle(Vector3::RIGHT, deg_to_rad(45.0) as f32), // Start with 45 deg tilt down
            orbiting: false,
            orbit_mouse_last_pos: Quaternion::default(), // Identity quaternion representing no rotation

            // These should be set in editor
            plane: Plane::from_normal_at_origin(Vector3::UP),
            bounds: Rect2::from_corners(Vector2::new(0.0, 0.0), Vector2::new(0.0, 0.0)),
            zoom_max: CAM_ZOOM_MAX_DEFAULT,
            zoom_min: CAM_ZOOM_MIN_DEFAULT,
            zoom_step: CAM_ZOOM_STEP_DEFAULT,
            zoom: 1.0,
            uniform_shader_canvas_item: None,
        }
    }

    fn unhandled_input(&mut self, event: Gd<InputEvent>) {
        if event.get_class() == "InputEventMouseButton".into() {
            let event: Gd<InputEventMouseButton> = event.cast();

            if event.get_button_index() == MouseButton::LEFT {
                if event.is_pressed() {
                    self.mouse_last_pos = self.get_plane_mouse_pos(event.get_position());
                    self.panning = true;
                } else {
                    self.panning = false;
                }
            } else if event.get_button_index() == MouseButton::RIGHT {
                if event.is_pressed() {
                    // Set initial last rotation
                    self.orbit_mouse_last_pos = self.get_rot_mouse_pos(event.get_position());

                    self.orbiting = true;
                } else {
                    self.orbiting = false;
                }
            } else if event.get_button_index() == MouseButton::WHEEL_UP {
                self.set_zoom(self.zoom - self.zoom_step);
            } else if event.get_button_index() == MouseButton::WHEEL_DOWN {
                self.set_zoom(self.zoom + self.zoom_step);
            }
        } else if event.get_class() == "InputEventMouseMotion".into() {
            let event: Gd<InputEventMouseMotion> = event.cast(); // Cast won't fail due to above check

            if self.panning {
                let mouse_current_pos: Vector3 = self.get_plane_mouse_pos(event.get_position());

                // Remove jitter loop by recalculating mouse_last_pos (https://discussions.unity.com/t/click-drag-map-view-so-that-point-under-mouse-remains-under-mouse/763291/5)
                let screen_last_pos: Vector2 = self.screen_last_pos;
                self.mouse_last_pos = self.get_plane_mouse_pos(screen_last_pos);

                let mut offset: Vector3 = self.mouse_last_pos - mouse_current_pos; // Drag by moving opposite dir of mouse movement
                offset.y = 0.0; // Centre cannot leave y = 0 plane
                let mut current_pos: Vector3 = self.centre_pos + offset;

                // Clamp movement to bounds if they are bigger than 0
                if self.bounds.size != Vector2::new(0.0, 0.0) {
                    if current_pos.x < self.bounds.position.x { current_pos.x = self.bounds.position.x; }
                    if current_pos.x > self.bounds.end().x { current_pos.x = self.bounds.end().x; }
                    if current_pos.z < self.bounds.position.y { current_pos.z = self.bounds.position.y; }
                    if current_pos.z > self.bounds.end().y { current_pos.z = self.bounds.end().y; }
                }

                self.centre_pos = current_pos;

                // Update
                self.mouse_last_pos = mouse_current_pos;
            }

            // Change orbit rotation
            if self.orbiting {
                let cur_rot: Quaternion = self.get_rot_mouse_pos(event.get_position());

                // Recalc previous orbit pos
                let screen_last_pos: Vector2 = self.screen_last_pos;
                self.orbit_mouse_last_pos = self.get_rot_mouse_pos(screen_last_pos);

                // Get difference between current and last rotations
                let rot_diff: Quaternion = self.orbit_mouse_last_pos * cur_rot.inverse();
                
                // Adjust and clamp orbit_pos so that it can never be straight up or down
                let orbit_pos: Quaternion = rot_diff * self.orbit_pos;

                // Do not move if orbit position is either straight up or down
                if !(orbit_pos.approx_eq(&Quaternion::new(-0.71, 0.0, 0.0, 0.71)) &&
                    orbit_pos.approx_eq(&Quaternion::new(0.71, 0.0, 0.0, 0.71))) {
                    self.orbit_pos = orbit_pos;
                    self.orbit_mouse_last_pos = cur_rot;
                }
            }
            
            // Keep last screen pos current for outside use
            self.screen_last_pos = event.get_position();
        }
    }

    fn process(&mut self, _: f64) {
        // Since process is ran every frame, this will make the mouse position most up to date
        self.update_world_mouse_intersection();

        // Calculate where camera should be
        let cam_vec: Vector3 = Basis::from_quat(self.orbit_pos) * Vector3::new(0.0, 0.0, -self.zoom);
        let centre_pos: Vector3 = self.centre_pos;
        self.base_mut().set_position(centre_pos + cam_vec);

        // Look at centre
        self.base_mut().set_basis(Basis::new_looking_at(-cam_vec, Vector3::UP, false));

        // Calc diff for whatever needs it
        let cam_pos: Vector3 = self.base().get_position();
        self.cam_pos_diff = cam_pos - self.last_cam_pos;

        // Set uniforms to shader each frame
        if let Some(ref mut canvas_item) = self.get_uniform_shader_canvas_item() {
            let mut shader = canvas_item.get_material().expect("Canvas item should have shader material attached to set uniforms")
                .try_cast::<ShaderMaterial>().expect("Canvas item should have shader material attached to set uniforms");
            
            shader.set_shader_parameter("panning".into(), Variant::from(self.panning));
            shader.set_shader_parameter("cam_pos_diff".into(), Variant::from(self.cam_pos_diff));
        }

        self.last_cam_pos = cam_pos;
    }

    fn physics_process(&mut self, _: f64) {
        // PhysicsDirectSpaceState3D can only safely be accessed from the physics process,
        // so update space state here
        if let Some(world_3d) = self.base().get_world_3d() {
            self.last_space_state = PhysicsServer3D::singleton().space_get_direct_state(world_3d.get_space());
        }
    }
}

#[godot_api]
impl PanningCamera {
    // Get the position of the mouse projected to a plane at y = 0
    #[func]
    fn get_plane_mouse_pos(&self, pos: Vector2) -> Vector3 {
        let origin: Vector3 = self.base().project_ray_origin(pos);
        let normal: Vector3 = self.base().project_ray_normal(pos) * 9999.0;

        if let Some(world_pos) = self.plane.intersect_ray(origin, normal) {
            world_pos
        } else {
            Vector3::ZERO
        }
    }

    #[func]
    fn get_rot_mouse_pos(&self, pos: Vector2) -> Quaternion {
        // Get vector from centre point to mouse pos
        let mouse_rot_vec: Vector3 = (self.base().project_position(pos, self.zoom / 2.0) - self.centre_pos).normalized();

        // Convert that vector to a quaternion by getting shortest arc from Vector3::UP and mouse_rot_vec
        // Constructor is missing so taken from here https://stackoverflow.com/a/1171995
        let mut rot: Quaternion = Quaternion::default();
        let a: Vector3 = Vector3::UP.cross(mouse_rot_vec);
        rot.x = a.x; rot.y = a.y; rot.z = a.z;
        rot.w = 1.0 + Vector3::UP.dot(mouse_rot_vec); // Since both vectors are normalized it reduces down to this
        rot.normalized() // Needs to be normalized for use as rotation
    }

    #[func]
    fn set_zoom(&mut self, zoom: f32) {
        self.zoom = zoom;

        // Clamp
        if self.zoom < self.get_zoom_min() { self.zoom = self.get_zoom_min(); }
        if self.zoom > self.get_zoom_max() { self.zoom = self.get_zoom_max(); }
    }

    // Function that updates first intersection with the world from mouse position
    fn update_world_mouse_intersection(&mut self) {
        let pos: Vector2 = self.screen_last_pos;
        let origin: Vector3 = self.base().project_ray_origin(pos);
        let normal: Vector3 = self.base().project_ray_normal(pos) * 9999.0; // Increase length to calc intersections

        let mut dictionary: Dictionary = Dictionary::new();
        
        if let Some(ref mut space_state ) = &mut self.last_space_state {
            let query: Option<Gd<PhysicsRayQueryParameters3D>> = PhysicsRayQueryParameters3D::create(origin, normal);

            dictionary = space_state.intersect_ray(query);
        }

        self.mouse_world_intersection = dictionary;
    }

    // Get result dictionary of the first intersection between a ray cast from the mouse
    // Clones dictionary of intersection, so use other methods unless needed
    #[func]
    fn get_world_mouse_intersection(&self) -> Dictionary {
        self.mouse_world_intersection.clone()
    }

    // Get the position of the first intersection between a ray cast from the mouse
    // Rust specific as func trait doesn't allow Option<Vector3>
    pub fn get_world_mouse_pos_option(&self) -> Option<Vector3> {
        // Ray did not hit anything
        if !self.mouse_world_intersection.contains_key("position") { return None; }

        let pos: Variant = self.mouse_world_intersection
            .get("position").expect("Cannot fail due to above check");

        Some(Vector3::from_variant(&pos))
    }

    // Function meant for godot that returns a variant, which is compatible with #[func]
    #[func]
    fn get_world_mouse_pos(&self) -> Variant {
        self.mouse_world_intersection.get_or_nil("position")
    }

    #[func]
    fn get_cam_pos_diff(&self) -> Vector3 {
        self.cam_pos_diff
    }
}

// TODO: Change this to integer scale viewport
#[derive(GodotClass)]
#[class(base=SubViewport)]
struct ResolutionDividerViewport {
    base: Base<SubViewport>,

    #[export] #[var(get, set=set_resolution_divisor)] resolution_divisor: i32,
}

#[godot_api]
impl ISubViewport for ResolutionDividerViewport {
    fn init(base: Base<SubViewport>) -> Self {
        Self {
            base,
            resolution_divisor: DITHER_RES_DIVISOR_DEFAULT,
        }
    }

    fn ready(&mut self) {
        // Apply changes to divisor that occured before the node was put in a tree
        self.set_resolution_divisor(self.resolution_divisor);
    }
}

#[godot_api]
impl ResolutionDividerViewport {
    // Adjust resolution scales to reflect divisor
    #[func]
    fn set_resolution_divisor(&mut self, res_div: i32) {
        self.resolution_divisor = res_div;

        // Can't obtain screen size without being in a tree
        if self.base().is_inside_tree() {
            let size: Vector2i = self.base().get_window()
                .expect("Node should have a root window if inside a tree").get_content_scale_size();
            
            let low_size: Vector2i = size / res_div;

            self.base_mut().set_size(low_size);
        }
    }
}

// Passes input to provided viewport
#[derive(GodotClass)]
#[class(base=Node)]
struct InputPassNode {
    base: Base<Node>,

    #[export] child_viewport: Option<Gd<Viewport>>,
    #[export] child_different_size: bool, // Set to true if it is, otherwise mouse inputs will be incorrect

    // If 0, the childviewport is not integer scaled
    // If negative, it is interpreted as 1 / n (this means that the child is smaller by a factor of n)
    #[export] integer_scale: i32,
}

#[godot_api]
impl INode for InputPassNode {
    fn init(base: Base<Node>) -> Self {
        Self {
            base,

            child_viewport: None,
            child_different_size: false,
            integer_scale: 0,
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
                let is_mouse_input = event.get_class() == "InputEventMouseButton".into() || event.get_class() == "InputEventMouseMotion".into();

                if self.child_different_size && is_mouse_input {
                    let mut event: Gd<InputEventMouse> = event.clone().cast(); // Can't fail because of above check
                    let mut x: f32 = event.get_position().x;
                    let mut y: f32 = event.get_position().y;

                    if self.integer_scale == 0 { // Map normally
                        let parent_size: Vector2 = self.base().get_viewport()
                            .expect("InputPassNode requires a parent viewport").get_visible_rect().size;
                        let child_size: Vector2 = viewport.get_visible_rect().size;

                        // f:R^ab -> R^cd
                        x = (x / parent_size.x) * child_size.x;
                        y = (y / parent_size.y) * child_size.y;
                    } else if self.integer_scale > 0 { // Child is n times bigger than parent
                        x = x * self.integer_scale as f32;
                        y = y * self.integer_scale as f32;
                    } else { // Child is n times smaller than parent
                        x = x / (-self.integer_scale) as f32;
                        y = y / (-self.integer_scale) as f32;
                    }

                    event.set_position(Vector2::new(x, y));
                }

                viewport.push_input_ex(event).in_local_coords(true).done();
            }
        }
    }
}

// Simple tree that can have any amount of children
struct VecTree<T> {
    value: T,
    children: Vec<Self>,
}

impl<T> VecTree<T> {
    fn new(value: T, children: Vec<Self>) -> Self {
        Self {
            value,
            children,
        }
    }
}

#[derive(GodotClass)]
#[class(base=GridMap)]
struct FieldGripMap {
    base: Base<GridMap>,
    last_mouse_coords: Option<Vector3i>,
    last_highlight_cell_offset: i32,
    char_refs: HashMap<Vector3i, Gd<FieldCharacter>>,
    focused_char: Option<Gd<FieldCharacter>>,
    focus_highlighted_cells: Vec<Vector3i>, // TODO: Make this a hashmap or tree?

    #[export] cam: Option<Gd<PanningCamera>>,
    #[export] highlight_offset: i32,
    #[export] highlight_move_offset: i32,
    #[export] highlight_attack_offset: i32,
    #[export] highlight_heal_offset: i32,
    #[export] block_type_len: i32,
    #[export] slope_index: i32,
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
                        self.set_char_focused(Some((*char_ref).clone()));
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
    fn get_coords_from_world_pos(&self, world_pos: Vector3) -> Vector3i {
        let local_pos: Vector3 = self.base().to_local(world_pos);
        self.base().local_to_map(local_pos)
    }

    #[func]
    fn set_overlay_block(&mut self, overlay_coords: Vector3i, highlight_offset: i32) {
        let mut cell_type: i32 = self.base().get_cell_item(overlay_coords);
        cell_type -= cell_type % self.block_type_len;

        // Always preserve orientation
        let orientation: i32 = self.base().get_cell_item_orientation(overlay_coords);

        self.base_mut().set_cell_item_ex(overlay_coords, cell_type + highlight_offset)
            .orientation(orientation).done();
    }

    #[func]
    fn map_to_local(&self, coords: Vector3i) -> Vector3 {
        self.base().map_to_local(coords)
    }

    // Change position of character currently on board
    // Does not check whether char_ref at cur_pos is the same
    // Overwrites whatever is at the position
    #[func]
    fn reposition_char(&mut self, char_ref: Gd<FieldCharacter>, cur_pos: Vector3i, new_pos: Vector3i) {
        self.char_refs.remove_entry(&cur_pos);
        self.char_refs.insert(new_pos, char_ref);
    }

    fn show_range_tree(&mut self, node: &VecTree<Vector3i>, highlight_offset: i32) {
        for child_node in node.children.iter() {
            self.show_range_tree(child_node, highlight_offset);
        }

        // Highlight block under current pos
        let pos: Vector3i = node.value + Vector3i::new(0, -1, 0);
        self.set_overlay_block(pos, highlight_offset);
        self.focus_highlighted_cells.push(pos);
    }

    #[func]
    fn show_char_ranges(&mut self, char: Gd<FieldCharacter>) {
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
    fn clear_char_ranges(&mut self) {
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
    fn set_char_focused(&mut self, val: Option<Gd<FieldCharacter>>) {
        self.focused_char = val;
    }
}

#[derive(GodotConvert, Var, Export)]
#[godot(via = GString)]
enum CharType {
    Player,
    Ally,
    Enemy,
}

#[derive(GodotClass)]
#[class(base=CharacterBody3D)]
struct FieldCharacter {
    base: Base<CharacterBody3D>,
    pub movement_tree: VecTree<Vector3i>,
    pub attack_tree: VecTree<Vector3i>,
    pub heal_tree: VecTree<Vector3i>,

    #[export] #[var(get, set=set_field_pos)] field_position: Vector3i,
    #[export] chartype: CharType,
    #[export] movement_range: u32,
    #[export] attack_range: u32,
    #[export] heal_range: u32,
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

    fn ready(&mut self) {
        self.set_field_pos(self.field_position);
    }
}

#[godot_api]
impl FieldCharacter {
    #[func]
    fn set_field_pos(&mut self, pos: Vector3i) {
        if Engine::singleton().is_editor_hint() || self.base().is_inside_tree() {
            // Update position based on FieldGridMap's coordinates
            let mut field: Gd<FieldGripMap> = self.base().get_parent().expect("FieldCharacter should be direct child of a FieldMap")
                .try_cast().expect("FieldCharacter should be direct child of a FieldMap");

            self.base_mut().set_position(field.bind().map_to_local(pos));

            // Update position on field
            field.bind_mut().reposition_char(Gd::from_instance_id(self.base().instance_id()), self.field_position, pos);
        }

        self.field_position = pos;
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
            if cell_item - (cell_item % field.block_type_len) == field.slope_index {
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

                node.children.push(self._get_range_tree(&field, child_node, remaining_range - 1));
            }
        }

        node
    }

    pub fn get_range_tree(&self, field: &FieldGripMap, range: u32) -> VecTree<Vector3i> {
        self._get_range_tree(field, VecTree::new(self.field_position, vec![]), range)
    }
}

// ColorRect specifically for this game's dithering
#[derive(GodotClass)]
#[class(base=ColorRect)]
struct DitherShaderRect {
    base: Base<ColorRect>,
    shader_mat_ref: Option<Gd<ShaderMaterial>>,
    phys_frame: i64,
    pattern_index: i64,
    
    #[export] cam: Option<Gd<PanningCamera>>,
    #[export] pattern_length: i64,
    #[export] divisor_coefficient: i64,
}

#[godot_api]
impl IColorRect for DitherShaderRect {
    fn init(base: Base<ColorRect>) -> Self {
        Self {
            base,
            shader_mat_ref: None,
            phys_frame: 0,
            pattern_index: 0,

            cam: None,
            pattern_length: 0,
            divisor_coefficient: 15,
        }
    }

    // Set Shader reference here for performance
    fn ready(&mut self) {
        let mat: Option<Gd<Material>> = self.base().get_material();
        if mat == None { return; }
        let mat: Gd<Material> = mat.expect("Cannot fail due to above check");

        if mat.get_class() != "ShaderMaterial".into() { return; }

        self.shader_mat_ref = Some(mat.cast());
    }

    fn process(&mut self, _: f64) {
        if let Some(ref mut cam) = self.cam {
            let cam_pos_len: i64 = cam.bind().get_cam_pos_diff().length().ceil() as i64;

            // Don't change index if not moving
            if cam_pos_len != 0 {
                // Get index of which pattern to use this frame
                // Multiply by difference of camera position to add change in dithering
                // Based on how fast the camera is moving
                let mut div: i64 = self.divisor_coefficient / cam_pos_len;
                if div == 0 { div = 1; } // Avoid dividing by 0
                self.pattern_index = self.phys_frame / div;
                self.pattern_index %= self.pattern_length;
            }

            if let Some(ref mut shader_mat) = self.shader_mat_ref {
                shader_mat.set_shader_parameter("pattern_index".into(), Variant::from(self.pattern_index));
            }
        }
    }

    fn physics_process(&mut self, _: f64) {
        let frame: i64 = self.phys_frame;

        if let Some(ref mut shader_mat) = self.shader_mat_ref {
            shader_mat.set_shader_parameter("phys_frame".into(), Variant::from(frame));
        }

        self.phys_frame += 1;
    }
}

#[godot_api]
impl DitherShaderRect {
    // For performance reasons the ref is stored in the struct
    // Needs to be called whenever shader is updated
    #[func]
    fn set_shader_mat_ref(&mut self, shader_mat: Option<Gd<ShaderMaterial>>) {
        self.shader_mat_ref = shader_mat;
    }
}