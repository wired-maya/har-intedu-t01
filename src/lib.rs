use godot::{builtin::{Plane, Vector2, Vector3}, classes::{Camera3D, IMarker3D, INode, InputEvent, InputEventMouseButton, InputEventMouseMotion, Marker3D, Node}, global::MouseButton, init::{gdextension, ExtensionLibrary}, obj::{Base, Gd, WithBaseField}, prelude::{godot_api, GodotClass}};
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
    zoom: f32,
    panning: bool,
    pub cam_plane: Plane,
    
    #[export] cam_bounds: Vector2,
    #[export] bound_pos: Vector3, // Sets position of camera bound's centre
}

#[godot_api]
impl IMarker3D for PanningCamera {
    fn init(base: Base<Marker3D>) -> Self {
        Self {
            base,
            screen_last_pos: Vector2::ZERO,
            mouse_last_pos: Vector3::ZERO,
            zoom: 0.0,
            panning: false,
            cam_plane: Plane::from_normal_at_origin(Vector3::UP), // y = 0

            cam_bounds: Vector2::ZERO,
            bound_pos: Vector3::ZERO,
        }
    }

    // Run when engine has finished loading, so we bind engine objects here
    fn ready(&mut self) {
        let zoom: f32 = n!(self, "Camera3D", Camera3D).get_position().y; // TODO: Ensure camera 3D by adding it in ready
        self.zoom = zoom;
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

            let mut offset: Vector3 = self.mouse_last_pos - mouse_current_pos; // Drag by moving opposite dir of mouse movement
            offset.y = 0.0; // Only zoom can move camera on y axis
            self.base_mut().translate(offset);

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

        // TODO: change back to not using cam plane
        if let Some(world_pos) = self.cam_plane.intersect_ray(origin, normal) {
            world_pos
        } else {
            Vector3::ZERO
        }
    }

    #[func]
    pub fn get_zoom(&self) -> f32 {
        self.zoom
    }

    #[func]
    pub fn set_zoom(&mut self, zoom: f32) {
        self.zoom = zoom;

        // Clamp
        if self.zoom < CAM_ZOOM_MIN { self.zoom = CAM_ZOOM_MIN; }
        if self.zoom > CAM_ZOOM_MAX { self.zoom = CAM_ZOOM_MAX; }

        nm!(self, "Camera3D", Camera3D).set_position(Vector3::new(0.0, zoom, 0.0));
    }
}