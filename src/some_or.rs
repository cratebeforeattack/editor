#[macro_export]
macro_rules! some_or {
    ($res:expr, $action:expr) => {
        match $res {
            Some(val) => val,
            None => {
                $action;
            }
        }
    };
}
pub use some_or;
