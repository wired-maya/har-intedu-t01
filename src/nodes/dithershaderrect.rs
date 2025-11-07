use crate::nodes::PanningCamera;

use godot::{builtin::Variant, classes::{ColorRect, IColorRect, Material, ShaderMaterial}, obj::{Base, Gd, WithBaseField}, prelude::{godot_api, GodotClass}};

// ColorRect specifically for this game's dithering
#[derive(GodotClass)]
#[class(base=ColorRect)]
pub struct DitherShaderRect {
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