pub trait Entity {
    fn id(&self) -> Option<&u32>;
    fn set_id(&mut self, id: u32);

    /// An entity without an id renders as `-` instead of panicking.
    fn display_id(&self) -> String {
        self.id().map_or_else(|| "-".to_owned(), ToString::to_string)
    }
}
