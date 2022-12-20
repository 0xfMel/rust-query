use std::{fmt::Debug, rc::Rc};

use downcast_rs::{impl_downcast, Downcast};

impl Error for () {
    fn kind(self: Rc<Self>) -> Option<Box<dyn ErrorKind>> {
        None
    }
}

pub trait Error: Debug {
    fn kind(self: Rc<Self>) -> Option<Box<dyn ErrorKind>>;
}

pub trait ErrorKind: Downcast {}
impl_downcast!(ErrorKind);
