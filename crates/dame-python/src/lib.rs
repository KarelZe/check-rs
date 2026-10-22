//! Python bindings for the dame draughts engine.
//!
//! The extension is imported as `dame._core`; the public, documented API
//! is re-exported from the pure-Python `dame` package.

use pyo3::prelude::*;

#[pymodule]
mod _core {
    use pyo3::prelude::*;

    #[pymodule_init]
    fn init(m: &Bound<'_, PyModule>) -> PyResult<()> {
        m.add("__version__", dame::VERSION)
    }
}
