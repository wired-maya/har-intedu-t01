// Simple tree that can have any amount of children
pub struct VecTree<T> {
    // Change to get/set funcs
    pub value: T,
    pub children: Vec<Self>,
}

impl<T> VecTree<T> {
    pub fn new(value: T, children: Vec<Self>) -> Self {
        Self {
            value,
            children,
        }
    }
}