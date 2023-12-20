use super::{children::Children, text::Text};

#[derive(Debug, Clone)]
pub(crate) enum Node {
    Internal(Children),
    Leaf(Text),
}
