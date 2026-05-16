mod registry;
mod view;

pub use registry::{add, list, list_remote, remove};
pub use view::{cat, cd, info};
