#[cfg(all(feature = "num-backend", feature = "gmp"))]
compile_error!("features `num-backend` and `gmp` are mutually exclusive; pick exactly one");

#[cfg(not(any(feature = "num-backend", feature = "gmp")))]
compile_error!("one of `num-backend` or `gmp` must be enabled");

#[cfg(feature = "num-backend")]
mod num_impl;

#[cfg(feature = "gmp")]
mod rug_impl;

#[cfg(feature = "num-backend")]
pub use num_impl::{QQ, ZZ};

#[cfg(feature = "gmp")]
pub use rug_impl::{QQ, ZZ};
