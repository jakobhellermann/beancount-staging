use std::{fmt::Debug, time::Duration};

use pyo3::{exceptions::PyValueError, prelude::*};

/// Python bindings for beancount-staging
#[pymodule]
mod beancount_staging {
    use crate::run_async;
    use beancount_staging_cli::beancount_staging::reconcile::StagingSource;
    use beancount_staging_web::ListenerType;
    use pyo3::prelude::*;
    use std::path::PathBuf;

    /// Run the command line
    #[pyfunction]
    fn cli(py: Python<'_>) -> PyResult<()> {
        let args = py
            .import("sys")?
            .getattr("argv")?
            .extract::<Vec<String>>()?;
        run_async(py, beancount_staging_cli::run(args))
    }

    /// Run the staging UI webserver
    #[pyfunction]
    #[pyo3(signature = (journal_files, staging_files, port = 8472))]
    fn serve(
        py: Python<'_>,
        journal_files: Vec<PathBuf>,
        staging_files: Vec<PathBuf>,
        port: u16,
    ) -> PyResult<()> {
        run_async(
            py,
            beancount_staging_web::run(
                journal_files,
                StagingSource::Files(staging_files),
                ListenerType::Tcp(port),
            ),
        )
    }
}

fn run_async<T, E: Debug>(
    py: Python<'_>,
    future: impl Future<Output = Result<T, E>>,
) -> PyResult<T> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed building the runtime");

    runtime
        .block_on(async {
            tokio::select! {
                result = future => result,
                _ = async {
                    loop {
                        let _ = py.check_signals();
                        tokio::time::sleep(Duration::from_millis(5)).await;
                    }
                } => unreachable!("signal loop never returns"),
            }
        })
        .map_err(|e| PyValueError::new_err(format!("{e:?}")))
}
