use std::{
    fmt::{self, Debug, Formatter},
    rc::Rc,
};

use downcast_rs::{impl_downcast, Downcast};

pub trait ErrorDisplay {
    fn err_fmt(&self, f: &mut Formatter<'_>) -> fmt::Result;
}

impl Error for () {
    fn kind(self: Rc<Self>) -> Option<Box<dyn ErrorKind>> {
        None
    }
}

impl ErrorDisplay for () {
    fn err_fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "no error type")
    }
}

pub trait Error: Debug + ErrorDisplay {
    fn kind(self: Rc<Self>) -> Option<Box<dyn ErrorKind>>;
}

pub trait ErrorKind: Downcast {}
impl_downcast!(ErrorKind);
