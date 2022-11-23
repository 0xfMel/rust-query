use crate::{hydrate::HydratableQuery, Query};

#[derive(HydratableQuery)]
#[result(<i32, i32>)]
#[param(usize)]
struct Test1;

fn test() {
    let query = Test1::builder().build(Query::new_with_param(Box::new(|_| {
        Box::pin(async { Ok::<i32, ()>(120) })
    })));
}
