mod children;
mod node;
mod text;
mod text_info;

pub use text_info::TextInfo;

#[cfg(not(any(test, feature = "internal_dev_small_chunks")))]
mod constants {
    pub(crate) const MAX_CHILDREN: usize = 16;
    pub(crate) const MIN_CHILDREN: usize = 7;
    pub(crate) const MAX_TEXT_SIZE: usize = 2048;
    pub(crate) const MIN_TEXT_SIZE: usize = (MAX_TEXT_SIZE / 2) - (MAX_TEXT_SIZE / 32);
}
#[cfg(any(test, feature = "internal_dev_small_chunks"))]
mod constants {
    pub(crate) const MAX_CHILDREN: usize = 5;
    pub(crate) const MIN_CHILDREN: usize = 2;
    pub(crate) const MAX_TEXT_SIZE: usize = 15;
    pub(crate) const MIN_TEXT_SIZE: usize = 7;
}
pub(crate) use constants::{MAX_CHILDREN, MAX_TEXT_SIZE, MIN_CHILDREN, MIN_TEXT_SIZE};

const _: () = assert!(
    MAX_CHILDREN <= 31,
    "Due to the way tree balance flags are stored and manipulated, the tree cannot have more than 31 children."
);

pub(crate) use children::Children;
pub(crate) use node::Node;
pub(crate) use text::Text;
