use super::{internal::Internal, leaf::Leaf};

#[derive(Debug, Clone)]
pub(crate) enum Node {
    Internal(Internal),
    Leaf(Leaf),
}
