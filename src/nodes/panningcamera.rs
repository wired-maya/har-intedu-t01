use crate::constants::*;

use godot::{builtin::{math::ApproxEq, Basis, Dictionary, Plane, Quaternion, Rect2, Variant, Vector2, Vector3}, classes::{Camera3D, CanvasItem, ICamera3D, InputEvent, InputEventMouseButton, InputEventMouseMotion, PhysicsDirectSpaceState3D, PhysicsRayQueryParameters3D, PhysicsServer3D, ShaderMaterial}, global::{deg_to_rad, MouseButton}, meta::FromGodot, obj::{Base, Gd, WithBaseField}, prelude::{godot_api, GodotClass}};

// TODO: Rotation around current pos on plane, scrolling is distance from that point
// TODO: Basically use pivot where pivot is clamped to bounds but not the actual cam
#[derive(GodotClass)]
#[class(base=Camera3D)]
pub struct PanningCamera {
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
    pub fn get_plane_mouse_pos(&self, pos: Vector2) -> Vector3 {
        let origin: Vector3 = self.base().project_ray_origin(pos);
        let normal: Vector3 = self.base().project_ray_normal(pos) * 9999.0;

        if let Some(world_pos) = self.plane.intersect_ray(origin, normal) {
            world_pos
        } else {
            Vector3::ZERO
        }
    }

    #[func]
    pub fn get_rot_mouse_pos(&self, pos: Vector2) -> Quaternion {
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
    pub fn set_zoom(&mut self, zoom: f32) {
        self.zoom = zoom;

        // Clamp
        if self.zoom < self.get_zoom_min() { self.zoom = self.get_zoom_min(); }
        if self.zoom > self.get_zoom_max() { self.zoom = self.get_zoom_max(); }
    }

    // Function that updates first intersection with the world from mouse position
    pub fn update_world_mouse_intersection(&mut self) {
        let pos: Vector2 = self.screen_last_pos;
        let origin: Vector3 = self.base().project_ray_origin(pos);
        let normal: Vector3 = self.base().project_ray_normal(pos) * 9999.0; // Increase length to calc intersections

        let mut dictionary: Dictionary = Dictionary::new();
        
        if let Some(space_state ) = &mut self.last_space_state {
            let query: Option<Gd<PhysicsRayQueryParameters3D>> = PhysicsRayQueryParameters3D::create(origin, normal);

            dictionary = space_state.intersect_ray(query);
        }

        self.mouse_world_intersection = dictionary;
    }

    // Get result dictionary of the first intersection between a ray cast from the mouse
    // Clones dictionary of intersection, so use other methods unless needed
    #[func]
    pub fn get_world_mouse_intersection(&self) -> Dictionary {
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
    pub fn get_world_mouse_pos(&self) -> Variant {
        self.mouse_world_intersection.get_or_nil("position")
    }

    #[func]
    pub fn get_cam_pos_diff(&self) -> Vector3 {
        self.cam_pos_diff
    }
}