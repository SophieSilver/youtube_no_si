use std::{any::Any, error::Error, fmt::Display};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FullErrorDisplay<E>(pub E);

impl<E: Error> Display for FullErrorDisplay<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}", self.0)?;

        let mut e: &dyn Error = &self.0;
        while let Some(src) = e.source() {
            e = src;
            writeln!(f, "Caused by: {src}")?;
        }

        Ok(())
    }
}

pub fn downcast_panic(panic: &(dyn Any + Send)) -> Option<&str> {
    if let Some(s) = panic.downcast_ref::<&str>() {
        dbg!(s);
        Some(s)
    } else if let Some(s) = panic.downcast_ref::<String>() {
        dbg!(s);
        Some(s)
    } else {
        None
    }
}
