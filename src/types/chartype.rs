use godot::{builtin::GString, prelude::{Export, GodotConvert, Var}};

#[derive(GodotConvert, Var, Export)]
#[godot(via = GString)]
pub enum CharType {
    Player,
    Ally,
    Enemy,
}