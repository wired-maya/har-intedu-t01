use std::collections::HashMap;

use godot::{builtin::{Array, Dictionary, Plane, Rect2, StringName, Variant, Vector2, Vector2i, Vector3, Vector3i}, classes::{Camera3D, CanvasItem, CharacterBody3D, ColorRect, Engine, GridMap, ICamera3D, ICharacterBody3D, IColorRect, IGridMap, INode, ISubViewport, InputEvent, InputEventMouse, InputEventMouseButton, InputEventMouseMotion, Material, Node, PhysicsDirectSpaceState3D, PhysicsRayQueryParameters3D, PhysicsServer3D, ShaderMaterial, SubViewport, Time, Viewport}, global::MouseButton, init::{gdextension, ExtensionLibrary}, meta::FromGodot, obj::{Base, Gd, WithBaseField}, prelude::{godot_api, GodotClass}};

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
    base: Base<Camera3D>,
    screen_last_pos: Vector2,
    mouse_last_pos: Vector3,
    panning: bool,
    mouse_world_intersection: Dictionary,
    last_space_state: Option<Gd<PhysicsDirectSpaceState3D>>,
    last_cam_pos: Vector3,
    cam_pos_diff: Vector3,

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
            } else if event.get_button_index() == MouseButton::WHEEL_UP {
                self.set_zoom(self.zoom - CAM_ZOOM_STEP_DEFAULT);
            } else if event.get_button_index() == MouseButton::WHEEL_DOWN {
                self.set_zoom(self.zoom + CAM_ZOOM_STEP_DEFAULT);
            }
        } else if event.get_class() == "InputEventMouseMotion".into() {
            let event: Gd<InputEventMouseMotion> = event.cast(); // Cast won't fail due to above check

            if self.panning {
                let mouse_current_pos: Vector3 = self.get_plane_mouse_pos(event.get_position());

                // Remove jitter loop by recalculating mouse_last_pos (https://discussions.unity.com/t/click-drag-map-view-so-that-point-under-mouse-remains-under-mouse/763291/5)
                let screen_last_pos = self.screen_last_pos;
                self.mouse_last_pos = self.get_plane_mouse_pos(screen_last_pos);

                let last_pos = self.base().get_position();
                let mut offset: Vector3 = self.mouse_last_pos - mouse_current_pos; // Drag by moving opposite dir of mouse movement
                offset.y = 0.0; // Only zoom can move camera on y axis
                let mut current_pos: Vector3 = last_pos + offset;

                // Clamp movement to bounds if they are bigger than 0
                if self.bounds.size != Vector2::new(0.0, 0.0) {
                    if current_pos.x < self.bounds.position.x { current_pos.x = self.bounds.position.x; }
                    if current_pos.x > self.bounds.end().x { current_pos.x = self.bounds.end().x; }
                    if current_pos.z < self.bounds.position.y { current_pos.z = self.bounds.position.y; }
                    if current_pos.z > self.bounds.end().y { current_pos.z = self.bounds.end().y; }
                }

                self.base_mut().set_position(current_pos);

                // Update
                self.mouse_last_pos = mouse_current_pos;
            }
            
            // Keep last screen pos current for outside use
            self.screen_last_pos = event.get_position();
        }
    }

    fn process(&mut self, _: f64) {
        // Since process is ran every frame, this will make the mouse position most up to date
        self.update_world_mouse_intersection();

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

    // Apply zoom level immediately
    fn ready(&mut self) {
        self.set_zoom(self.zoom);
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
    fn set_zoom(&mut self, zoom: f32) {
        self.zoom = zoom;

        // Clamp
        if self.zoom < self.get_zoom_min() { self.zoom = self.get_zoom_min(); }
        if self.zoom > self.get_zoom_max() { self.zoom = self.get_zoom_max(); }

        let mut last_pos: Vector3 = self.base().get_position();
        last_pos.y = zoom;

        self.base_mut().set_position(last_pos);
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

#[derive(GodotClass)]
#[class(base=GridMap)]
struct FieldGripMap {
    base: Base<GridMap>,
    last_mouse_coords: Option<Vector3i>,
    last_highlight_cell_type: i32,
    char_refs: HashMap<Vector3i, Gd<FieldCharacter>>,
    char_focused: bool,
    focus_highlighted_cells: Vec<Vector3i>,

    #[export] cam: Option<Gd<PanningCamera>>,
    #[export] highlight_index: i32, // TODO: Move these to offsets
    #[export] highlight_move_index: i32,
    #[export] highlight_attack_index: i32,
    #[export] highlight_heal_index: i32,
}

#[godot_api]
impl IGridMap for FieldGripMap {
    fn init(base: Base<GridMap>) -> Self {
        Self {
            base,
            last_mouse_coords: None,
            last_highlight_cell_type: -1,
            char_refs: HashMap::new(),
            char_focused: false,
            focus_highlighted_cells: Vec::new(),

            cam: None,
            highlight_index: 0,
            highlight_move_index: 0,
            highlight_attack_index: 0,
            highlight_heal_index: 0,
        }
    }

    fn unhandled_input(&mut self, event: Gd<InputEvent>) {
        if event.get_class() == "InputEventMouseButton".into() {
            let event: Gd<InputEventMouseButton> = event.cast(); // Cast won't fail due to above check

            if event.get_button_index() == MouseButton::LEFT && event.is_pressed() && !self.char_focused {
                // Get currently moused over character
                if let Some(mut pos) = self.last_mouse_coords {
                    let mut move_range: u32 = 0;
                    let mut attack_range: u32 = 0;
                    let mut heal_range: u32 = 0;

                    pos.y += 1; // Block above currently moused

                    if let Some(char_ref) = self.char_refs.get(&pos) {
                        // Since movement range doesn't include the current position, add 1
                        move_range = char_ref.bind().get_movement_range() + 1;
                        attack_range = char_ref.bind().get_attack_range();
                        heal_range = char_ref.bind().get_heal_range();
                        self.char_focused = true;
                    }

                    if move_range > 0 {
                        self.show_char_ranges(pos, move_range, attack_range, heal_range);
                    }
                }
            } else if event.get_button_index() == MouseButton::RIGHT && event.is_pressed() && self.char_focused {
                // Exit out of being focused on a character
                self.char_focused = false;
                self.clear_char_ranges();
            }
        } /* else if event.get_class() == "InputEventMouseMovement".into() {
            // Show range on hover
            if let Some(mut pos) = self.last_mouse_coords {
                if let Some(char_ref) = self.char_refs.get(&pos) {
                    let move_range: u32 = char_ref.bind().get_movement_range();

                    pos.y += 1; // Block above currently moused
                    

                    if move_range > 0 { self.show_char_ranges(pos, move_range); }

                    self.char_focused = true;
                } else {
                    self.char_focused = false;
                    self.clear_char_ranges();
                }
            }
        } */
    }

    fn process(&mut self, _: f64) {
        // Mouse pos is calculated every frame for smoothness
        if let Some(cam) = self.get_cam() {
            if let Some(world_pos) = cam.bind().get_world_mouse_pos_option() {
                let mouse_coords: Vector3i = self.get_coords_from_world_pos(world_pos);

                if Some(mouse_coords) != self.last_mouse_coords {
                    if !self.char_focused { self.last_highlight_cell_type = 0 } // Block can only be 0 if char not focused

                    if let Some(last_mouse_coords) = self.last_mouse_coords {
                        self.set_overlay_block(last_mouse_coords, self.last_highlight_cell_type);
                    }

                    self.last_highlight_cell_type = self.base().get_cell_item(mouse_coords);

                    self.set_overlay_block(mouse_coords, self.highlight_index);

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
    fn set_overlay_block(&mut self, overlay_coords: Vector3i, mesh_index: i32) {
        // TODO: Change to offset here
        self.base_mut().set_cell_item(overlay_coords, mesh_index);
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

    // Returns a map that includes all possible points the character can move
    #[func]
    fn get_range_map(&self, char_pos: Vector3i, mut range: u32, range_offset: u32) -> Vec<Vector3i> {
        range = range + range_offset;

        // TODO: Handle slopes, cliffs, etc
        // TODO: Make enemies block as well
        let probe_cells: Array<Vector3i> = self.base().get_used_cells();

        // Filters probe cells to remove those that don't have space above them,
        // and add one to y to return cells possible to move to
        let probe_cells = probe_cells.iter_shared().filter_map(|mut pos| {
            pos.y += 1; // Adj to cell above

            if self.base().get_cell_item(pos) != GridMap::INVALID_CELL_ITEM {
                None
            } else {
                Some(pos)
            }
        });

        // Calculate range by just getting block diff between char and probe cell
        // This means the range is how many blocks they can move, which is intended
        let probe_cells = probe_cells.filter(|pos| {
            let pos_diff: Vector3i = (char_pos - *pos).abs();
            let distance: u32 = (pos_diff.x + pos_diff.y + pos_diff.z) as u32;

            distance <= range && distance >= range_offset
        });

        // TODO: iterate over existing chars and remove them from being allowed for movement

        probe_cells.collect()
    }

    #[func]
    fn show_char_ranges(&mut self, char_pos: Vector3i, movement_range: u32, attack_range: u32, heal_range: u32) {
        let mut healable: Vec<Vector3i> = self.get_range_map(char_pos, heal_range, movement_range);

        for pos in healable.iter_mut() {
            pos.y -= 1; // Highlights under the character
            self.focus_highlighted_cells.push(*pos);

            self.set_overlay_block(*pos, self.highlight_heal_index);
        }

        let mut attackable: Vec<Vector3i> = self.get_range_map(char_pos, attack_range, movement_range);

        for pos in attackable.iter_mut() {
            pos.y -= 1; // Highlights under the character
            self.focus_highlighted_cells.push(*pos);

            self.set_overlay_block(*pos, self.highlight_attack_index);
        }

        let mut reachable: Vec<Vector3i> = self.get_range_map(char_pos, movement_range, 0);

        for pos in reachable.iter_mut() {
            pos.y -= 1; // Highlights under the character
            self.focus_highlighted_cells.push(*pos);

            self.set_overlay_block(*pos, self.highlight_move_index);
        }

        

        // Keep mouse highlight on field
        if let Some(mouse_coords) = self.last_mouse_coords {
            self.last_highlight_cell_type = self.highlight_move_index; // Draw movement range under mouse
            self.set_overlay_block(mouse_coords, self.highlight_index);
        }
    }

    #[func]
    fn clear_char_ranges(&mut self) {
        let last_range_cell_types: Vec<Vector3i> = self.focus_highlighted_cells.clone();

        for pos in last_range_cell_types.into_iter() {
            let mut mesh_index: i32 = 0;

            // Don't clear if block is highlighted currently
            if let Some(last_mouse_coords) = self.last_mouse_coords {
                if pos == last_mouse_coords {
                    self.last_highlight_cell_type = mesh_index;
                    mesh_index = self.highlight_index;
                }
            }

            self.set_overlay_block(pos, mesh_index);
        }

        self.focus_highlighted_cells.clear();
    }

    // TODO: Make this show/hide char information
    // TODO: Do that by making char_focused Option<Gd<FieldCharacter>>
    fn _set_char_focused(&mut self, val: bool) {
        self.char_focused = val;
    }
}

#[derive(GodotClass)]
#[class(base=CharacterBody3D)]
struct FieldCharacter {
    base: Base<CharacterBody3D>,

    #[export] #[var(get, set=set_field_pos)] field_position: Vector3i,
    #[export] is_enemy: bool, // TODO: Replace with enum since there can be allies that you can't control
    #[export] movement_range: u32,
    #[export] attack_range: u32,
    #[export] heal_range: u32,
}

#[godot_api]
impl ICharacterBody3D for FieldCharacter {
    fn init(base: Base<CharacterBody3D>) -> Self {
        Self {
            base,

            field_position: Vector3i::ZERO,
            is_enemy: false,
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


}

// ColorRect that passes time to its shader material for more advanced shading
// ColorRect's shader needs a uniform float, but you can set name
#[derive(GodotClass)]
#[class(base=ColorRect)]
struct TimeShaderRect {
    base: Base<ColorRect>,
    shader_mat_ref: Option<Gd<ShaderMaterial>>,

    #[export] uniform_name: StringName,
}

#[godot_api]
impl IColorRect for TimeShaderRect {
    fn init(base: Base<ColorRect>) -> Self {
        Self {
            base,
            shader_mat_ref: None,

            uniform_name: "cur_time".into(),
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
        let uniform_name: StringName = self.get_uniform_name();

        if let Some(ref mut shader_mat) = self.shader_mat_ref {
            let time: f64 = Time::singleton().get_unix_time_from_system();

            shader_mat.set_shader_parameter(uniform_name, Variant::from(time));
        }
    }
}

#[godot_api]
impl TimeShaderRect {
    // For performance reasons the ref is stored in the struct
    // Needs to be called whenever shader is updated
    #[func]
    fn set_shader_mat_ref(&mut self, shader_mat: Option<Gd<ShaderMaterial>>) {
        self.shader_mat_ref = shader_mat;
    }
}

// ColorRect that passes frames elapsed since it was created
// ColorRect's shader needs a uniform int, but you can set name
#[derive(GodotClass)]
#[class(base=ColorRect)]
struct FrameCountShaderRect {
    base: Base<ColorRect>,
    shader_mat_ref: Option<Gd<ShaderMaterial>>,
    frame: i64,

    #[export] uniform_name: StringName,
}

#[godot_api]
impl IColorRect for FrameCountShaderRect {
    fn init(base: Base<ColorRect>) -> Self {
        Self {
            base,
            shader_mat_ref: None,
            frame: 0,

            uniform_name: "frame".into(),
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
        let uniform_name: StringName = self.get_uniform_name();
        let frame: i64 = self.frame;

        if let Some(ref mut shader_mat) = self.shader_mat_ref {
            shader_mat.set_shader_parameter(uniform_name, Variant::from(frame));
        }

        self.frame += 1;
    }
}

#[godot_api]
impl FrameCountShaderRect {
    // For performance reasons the ref is stored in the struct
    // Needs to be called whenever shader is updated
    #[func]
    fn set_shader_mat_ref(&mut self, shader_mat: Option<Gd<ShaderMaterial>>) {
        self.shader_mat_ref = shader_mat;
    }
}

// ColorRect that passes physics frames elapsed since it was created
// ColorRect's shader needs a uniform int, but you can set name
#[derive(GodotClass)]
#[class(base=ColorRect)]
struct PhysCountShaderRect {
    base: Base<ColorRect>,
    shader_mat_ref: Option<Gd<ShaderMaterial>>,
    phys_frame: i64,

    #[export] uniform_name: StringName,
}

#[godot_api]
impl IColorRect for PhysCountShaderRect {
    fn init(base: Base<ColorRect>) -> Self {
        Self {
            base,
            shader_mat_ref: None,
            phys_frame: 0,

            uniform_name: "phys_frame".into(),
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

    fn physics_process(&mut self, _: f64) {
        let uniform_name: StringName = self.get_uniform_name();
        let frame: i64 = self.phys_frame;

        if let Some(ref mut shader_mat) = self.shader_mat_ref {
            shader_mat.set_shader_parameter(uniform_name, Variant::from(frame));
        }

        self.phys_frame += 1;
    }
}

#[godot_api]
impl PhysCountShaderRect {
    // For performance reasons the ref is stored in the struct
    // Needs to be called whenever shader is updated
    #[func]
    fn set_shader_mat_ref(&mut self, shader_mat: Option<Gd<ShaderMaterial>>) {
        self.shader_mat_ref = shader_mat;
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
                let div: i64 = self.divisor_coefficient / cam_pos_len;
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