use std::sync::Arc;

use super::{Children, Text, TextInfo, MAX_CHILDREN};

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

    pub fn child_count(&self) -> usize {
        self.children().len()
    }

    pub fn children(&self) -> &Children {
        match *self {
            Node::Internal(ref children) => children,
            _ => panic!(),
        }
    }

    pub fn children_mut(&mut self) -> &mut Children {
        match *self {
            Node::Internal(ref mut children) => Arc::make_mut(children),
            _ => panic!(),
        }
    }

    pub fn leaf_text(&self) -> [&str; 2] {
        match *self {
            Node::Leaf(ref text) => text.chunks(),
            _ => panic!(),
        }
    }

    pub fn leaf_text_mut(&mut self) -> &mut Text {
        match *self {
            Node::Leaf(ref mut text) => Arc::make_mut(text),
            _ => panic!(),
        }
    }

    /// Note: `node_info` is the text info *for the node this is being called
    /// on*.  This is because node info for a child is stored in the parent.
    /// This makes it a little inconvenient to call, but is desireable for
    /// efficiency so that the info can be used for a cheaper update rather
    /// than being recomputed from scratch.
    ///
    /// Returns the new text info for the current node, and if a split was
    /// caused returns the right side of the split (the left remaining as the
    /// current node) and its text info.
    pub fn insert_at_byte_idx(
        &mut self,
        byte_idx: usize,
        text: &str,
        _node_info: TextInfo,
    ) -> (TextInfo, Option<(TextInfo, Node)>) {
        // TODO: use `node_info` to do an update of the node info rather
        // than recomputing from scratch.  This will be a bit delicate,
        // because it requires being aware of crlf splits.

        match *self {
            Node::Leaf(ref mut leaf_text) => {
                assert!(
                    leaf_text.is_char_boundary(byte_idx),
                    "Cannot insert text at a non-char boundary."
                );

                let leaf_text = Arc::make_mut(leaf_text);
                if text.len() <= leaf_text.free_capacity() {
                    // Enough room to insert.
                    leaf_text.insert(byte_idx, text);
                    return (leaf_text.text_info(), None);
                } else {
                    // Not enough room to insert.  Need to split into two nodes.
                    let mut right_text = leaf_text.split(byte_idx);
                    let text_split_idx =
                        crate::find_split(leaf_text.free_capacity(), text.as_bytes());
                    leaf_text.append_str(&text[..text_split_idx]);
                    right_text.insert(0, &text[text_split_idx..]);
                    leaf_text.distribute(&mut right_text);
                    return (
                        leaf_text.text_info(),
                        Some((right_text.text_info(), Node::Leaf(Arc::new(right_text)))),
                    );
                }
            }
            Node::Internal(ref mut children) => {
                let children = Arc::make_mut(children);

                // Find the child we care about.
                let (child_i, acc_byte_idx) = children.search_byte_idx_only(byte_idx);
                let info = children.info()[child_i];

                // Recurse into the child.
                let (l_info, residual) = children.nodes_mut()[child_i].insert_at_byte_idx(
                    byte_idx - acc_byte_idx,
                    text,
                    info,
                );
                children.info_mut()[child_i] = l_info;

                // Handle the residual node if there is one and return.
                if let Some((r_info, r_node)) = residual {
                    if children.len() < MAX_CHILDREN {
                        children.insert(child_i + 1, (r_info, r_node));
                        (children.combined_info(), None)
                    } else {
                        let r = children.insert_split(child_i + 1, (r_info, r_node));
                        let r_info = r.combined_info();
                        (
                            children.combined_info(),
                            Some((r_info, Node::Internal(Arc::new(r)))),
                        )
                    }
                } else {
                    (children.combined_info(), None)
                }
            }
        }
    }
}
