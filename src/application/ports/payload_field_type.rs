#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PayloadFieldType {
    Keyword,
    Integer,
    Float,
    Text,
}
