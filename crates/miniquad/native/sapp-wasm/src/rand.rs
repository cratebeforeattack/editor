pub const RAND_MAX: u32 = 2147483647;
extern "C" {
    pub fn rand() -> ::std::os::raw::c_int;
    pub fn now() -> f64;
}
