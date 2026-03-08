pub type KsbhStr = smol_str::SmolStr;

#[cfg(test)]
mod tests {
    use super::KsbhStr;
    use proptest::prelude::*;

    #[test]
    fn test_ksbh_str_from_str() {
        let s: KsbhStr = KsbhStr::new("hello");
        assert_eq!(s.as_str(), "hello");
    }

    #[test]
    fn test_ksbh_str_from_string() {
        let s: KsbhStr = KsbhStr::from(String::from("hello"));
        assert_eq!(s.as_str(), "hello");
    }

    #[test]
    fn test_ksbh_str_empty() {
        let s: KsbhStr = KsbhStr::new("");
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn test_ksbh_str_from_static_str() {
        let s: KsbhStr = KsbhStr::new_inline("inline");
        assert_eq!(s.as_str(), "inline");
    }

    #[test]
    fn test_ksbh_str_clone() {
        let s1: KsbhStr = KsbhStr::new("hello");
        let s2 = s1.clone();
        assert_eq!(s1.as_str(), s2.as_str());
    }

    #[test]
    fn test_ksbh_str_display() {
        let s: KsbhStr = KsbhStr::new("hello");
        assert_eq!(format!("{}", s), "hello");
    }

    #[test]
    fn test_ksbh_str_debug() {
        let s: KsbhStr = KsbhStr::new("hello");
        assert_eq!(format!("{:?}", s), "\"hello\"");
    }

    #[test]
    fn test_ksbh_str_default() {
        let s: KsbhStr = KsbhStr::default();
        assert!(s.is_empty());
    }

    #[test]
    fn test_ksbh_str_as_str() {
        let s: KsbhStr = KsbhStr::new("test");
        assert_eq!(s.as_str(), "test");
    }

    #[test]
    fn test_ksbh_str_as_bytes() {
        let s: KsbhStr = KsbhStr::new("test");
        assert_eq!(s.as_bytes(), b"test");
    }

    #[test]
    fn test_ksbh_str_equality() {
        let s1: KsbhStr = KsbhStr::new("hello");
        let s2: KsbhStr = KsbhStr::new("hello");
        let s3: KsbhStr = KsbhStr::new("world");
        assert_eq!(s1, s2);
        assert_ne!(s1, s3);
    }

    #[test]
    fn test_ksbh_str_hash() {
        use std::collections::HashSet;
        let mut set: HashSet<KsbhStr> = HashSet::new();
        set.insert(KsbhStr::new("hello"));
        set.insert(KsbhStr::new("world"));
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_ksbh_str_from_long_string() {
        let long = "a".repeat(100);
        let s: KsbhStr = KsbhStr::new(&long);
        assert_eq!(s.len(), 100);
    }

    #[test]
    fn test_ksbh_str_from_unicode() {
        let s: KsbhStr = KsbhStr::new("hello world");
        assert_eq!(s.len(), 11);
    }

    #[test]
    fn test_ksbh_str_concatenation() {
        let s1: KsbhStr = KsbhStr::new("hello");
        let s2: KsbhStr = KsbhStr::new("world");
        let combined = format!("{} {}", s1, s2);
        assert_eq!(combined, "hello world");
    }

    proptest! {
        #[test]
        fn test_ksbh_str_any_string(ref s in ".*") {
            let ks: KsbhStr = KsbhStr::new(s);
            prop_assert_eq!(ks.as_str(), s);
        }

        #[test]
        fn test_ksbh_str_unicode_string(ref s in "\\PCs*") {
            let ks: KsbhStr = KsbhStr::new(s);
            prop_assert_eq!(ks.as_str(), s);
        }
    }
}
