use godot::{builtin::{Plane, Rect2, Vector2, Vector3}, classes::{Camera3D, IMarker3D, INode, InputEvent, InputEventMouseButton, InputEventMouseMotion, Marker3D, Node}, global::MouseButton, init::{gdextension, ExtensionLibrary}, obj::{Base, Gd, WithBaseField}, prelude::{godot_api, GodotClass}};
use godot_macros::{n, nm};

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

#[derive(GodotClass)]
#[class(base=Marker3D)]
struct PanningCamera {
    base: Base<Marker3D>,
    screen_last_pos: Vector2,
    mouse_last_pos: Vector3,
    panning: bool,

    #[export] plane: Plane,
    #[export] bounds: Rect2, // 0,0 rect means no bounds
    #[export] zoom_step: f32,
    #[export] zoom_max: f32,
    #[export] zoom_min: f32,
    #[export] #[var(get, set = set_zoom)] zoom: f32,
}

#[godot_api]
impl IMarker3D for PanningCamera {
    fn init(base: Base<Marker3D>) -> Self {
        Self {
            base,
            screen_last_pos: Vector2::ZERO,
            mouse_last_pos: Vector3::ZERO,
            panning: false,

            // These should be set in editor
            plane: Plane::from_normal_at_origin(Vector3::UP), // y = 0
            bounds: Rect2::from_corners(Vector2::new(0.0, 0.0), Vector2::new(0.0, 0.0)),
            zoom_max: CAM_ZOOM_MAX,
            zoom_min: CAM_ZOOM_MIN,
            zoom_step: CAM_ZOOM_STEP,
            zoom: 1.0,
        }
    }

    // Run when engine has finished loading, so we bind engine objects here
    fn ready(&mut self) {
        n!(self, "Camera3D", Camera3D).set_position(Vector3::new(0.0, self.zoom, 0.0)); // TODO: Ensure camera 3D by adding it in ready
    }

    fn unhandled_input(&mut self, event: Gd<InputEvent>) {
        if event.get_class() == "InputEventMouseButton".into() {
            let event: Gd<InputEventMouseButton> = event.cast();

            if event.get_button_index() == MouseButton::LEFT {
                if event.is_pressed() {
                    self.mouse_last_pos = self.get_world_mouse_pos(event.get_position());
                    self.screen_last_pos = event.get_position();
                    self.panning = true;
                } else {
                    self.panning = false;
                }
            } else if event.get_button_index() == MouseButton::WHEEL_UP {
                self.set_zoom(self.zoom - CAM_ZOOM_STEP);
            } else if event.get_button_index() == MouseButton::WHEEL_DOWN {
                self.set_zoom(self.zoom + CAM_ZOOM_STEP);
            }
        } else if event.get_class() == "InputEventMouseMotion".into() && self.panning {
            let event: Gd<InputEventMouseMotion> = event.cast(); // Cast won't fail due to above check

            let mouse_current_pos: Vector3 = self.get_world_mouse_pos(event.get_position());

            // Remove jitter loop by recalculating mouse_last_pos (https://discussions.unity.com/t/click-drag-map-view-so-that-point-under-mouse-remains-under-mouse/763291/5)
            let screen_last_pos = self.screen_last_pos;
            self.mouse_last_pos = self.get_world_mouse_pos(screen_last_pos);

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
            self.screen_last_pos = event.get_position();
        }
    }
}

#[godot_api]
impl PanningCamera {
    // Get the position of the mouse projected to a plane at y = 0
    fn get_world_mouse_pos(&self, pos: Vector2) -> Vector3 {
        let cam: Gd<Camera3D> = n!(self, "Camera3D", Camera3D);
        let origin: Vector3 = cam.project_ray_origin(pos);
        let normal: Vector3 = cam.project_ray_normal(pos);

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

        nm!(self, "Camera3D", Camera3D).set_position(Vector3::new(0.0, zoom, 0.0));
    }
}