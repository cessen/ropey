use super::{children::Children, text::Text, text_info::TextInfo};

#[derive(Debug, Clone)]
pub(crate) enum Node {
    Internal(Children),
    Leaf(Text),
}

impl Node {
    /// Shallowly computes the text info of this node.
    ///
    /// Assumes that the info of this node's children is up to date.
    pub(crate) fn text_info(&self) -> TextInfo {
        todo!()
    }

    #[inline(always)]
    pub(crate) fn is_internal(&self) -> bool {
        match self {
            &Self::Internal(_) => true,
            &Self::Leaf(_) => false,
        }
    }

    #[inline(always)]
    pub(crate) fn is_leaf(&self) -> bool {
        match self {
            &Self::Internal(_) => false,
            &Self::Leaf(_) => true,
        }
    }
}
