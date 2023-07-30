pub trait Entity {
    fn id(&self) -> Option<&u32>;
    fn set_id(&mut self, id: u32);
}
