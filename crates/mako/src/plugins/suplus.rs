use crate::plugin::Plugin;

pub(super) struct SUPlus {}

impl Plugin for SUPlus {
    fn name(&self) -> &str {
        "suplus"
    }
}
