use std::sync::Arc;

use super::{node::Node, text_info::TextInfo};

#[derive(Debug, Clone)]
pub(crate) struct Internal {
    children: Vec<Arc<Node>>,
    children_info: Vec<TextInfo>,
}
