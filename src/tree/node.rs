use std::sync::Arc;

use super::{children::Children, text::Text, text_info::TextInfo};

#[derive(Debug, Clone)]
pub(crate) enum Node {
    Internal(Arc<Children>),
    Leaf(Arc<Text>),
}

impl Node {
    /// Shallowly computes the text info of this node.
    ///
    /// Assumes that the info of this node's children is up to date.
    pub(crate) fn text_info(&self) -> TextInfo {
        match &self {
            Node::Internal(children) => {
                let mut acc_info = TextInfo::new();
                for info in children.info() {
                    acc_info = acc_info.combine(*info);
                }
                acc_info
            }
            Node::Leaf(text) => text.text_info(),
        }
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
