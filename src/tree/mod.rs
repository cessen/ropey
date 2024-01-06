mod children;
mod node;
mod text;
mod text_info;

#[cfg(not(any(test, feature = "internal_dev_small_chunks")))]
mod constants {
    pub(crate) const MAX_CHILDREN: usize = 16;
    pub(crate) const MAX_TEXT_SIZE: usize = 2048;
    pub(crate) const MIN_TEXT_SIZE: usize = (MAX_TEXT_SIZE / 2) - (MAX_TEXT_SIZE / 32);
}
#[cfg(any(test, feature = "internal_dev_small_chunks"))]
mod constants {
    pub(crate) const MAX_CHILDREN: usize = 5;
    pub(crate) const MAX_TEXT_SIZE: usize = 15;
    pub(crate) const MIN_TEXT_SIZE: usize = 7;
}
pub(crate) use constants::{MAX_CHILDREN, MAX_TEXT_SIZE, MIN_TEXT_SIZE};

pub(crate) use children::Children;
pub(crate) use node::Node;
pub(crate) use text::Text;
pub(crate) use text_info::TextInfo;
