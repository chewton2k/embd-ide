pub mod buffer;
pub mod cursor;
pub mod edit;
pub mod history;
pub mod languages;
pub mod syntax;

pub use buffer::Buffer;
pub use cursor::{Selection, SelectionSet};
pub use languages::build_default_registry;
pub use syntax::{HighlightSpan, LanguageConfig, LanguageRegistry, SyntaxTree};
