mod children;
mod node;
mod text;
mod text_info;

#[cfg(not(any(test, feature = "small_chunks")))]
mod constants {
    pub(crate) const MAX_CHILDREN: usize = 16;
    pub(crate) const MAX_TEXT_SIZE: usize = 2048;
    pub(crate) const MIN_TEXT_SIZE: usize = MAX_TEXT_SIZE / 2 - 64;
}
#[cfg(any(test, feature = "small_chunks"))]
mod constants {
    pub(crate) const MAX_CHILDREN: usize = 4;
    pub(crate) const MAX_TEXT_SIZE: usize = 24;
    pub(crate) const MIN_TEXT_SIZE: usize = 10;
}
pub(crate) use constants::{MAX_CHILDREN, MAX_TEXT_SIZE, MIN_TEXT_SIZE};

pub(crate) use children::Children;
pub(crate) use node::Node;
pub(crate) use text::Text;
pub(crate) use text_info::TextInfo;
