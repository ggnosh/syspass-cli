pub trait Entity
{
    fn id(&mut self, new_id: Option<u32>) -> Option<u32>;
}
