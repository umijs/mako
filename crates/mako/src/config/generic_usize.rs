pub struct GenericUsizeDefault<const U: usize>;

impl<const U: usize> GenericUsizeDefault<U> {
    pub fn value() -> usize {
        U
    }
}

#[cfg(test)]
mod tests {
    use crate::config::GenericUsizeDefault;

    #[test]
    fn test_generic_usize_default() {
        assert!(GenericUsizeDefault::<100>::value() == 100usize)
    }
}
