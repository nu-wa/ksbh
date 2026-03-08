#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PathType {
    Exact,
    Prefix,
    ImplementationSpecific,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_type_exact() {
        let path_type = PathType::Exact;
        assert_eq!(path_type, PathType::Exact);
    }

    #[test]
    fn test_path_type_prefix() {
        let path_type = PathType::Prefix;
        assert_eq!(path_type, PathType::Prefix);
    }

    #[test]
    fn test_path_type_implementation_specific() {
        let path_type = PathType::ImplementationSpecific;
        assert_eq!(path_type, PathType::ImplementationSpecific);
    }

    #[test]
    fn test_path_type_clone() {
        let path_type1 = PathType::Exact;
        let path_type2 = path_type1.clone();
        assert_eq!(path_type1, path_type2);
    }

    #[test]
    fn test_path_type_debug() {
        let path_type = PathType::Exact;
        let debug_str = format!("{:?}", path_type);
        assert!(debug_str.contains("Exact"));
    }

    #[test]
    fn test_path_type_ord() {
        let mut types = vec![
            PathType::Prefix,
            PathType::Exact,
            PathType::ImplementationSpecific,
        ];
        types.sort();
        assert_eq!(types.len(), 3);
    }
}
