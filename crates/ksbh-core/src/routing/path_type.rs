#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PathType {
    Exact,
    Prefix,
    ImplementationSpecific,
}
