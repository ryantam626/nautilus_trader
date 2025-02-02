// -------------------------------------------------------------------------------------------------
//  Copyright (C) 2015-2025 Nautech Systems Pty Ltd. All rights reserved.
//  https://nautechsystems.io
//
//  Licensed under the GNU Lesser General Public License Version 3.0 (the "License");
//  You may not use this file except in compliance with the License.
//  You may obtain a copy of the License at https://www.gnu.org/licenses/lgpl-3.0.en.html
//
//  Unless required by applicable law or agreed to in writing, software
//  distributed under the License is distributed on an "AS IS" BASIS,
//  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//  See the License for the specific language governing permissions and
//  limitations under the License.
// -------------------------------------------------------------------------------------------------

use std::{
    sync::{atomic::Ordering, Arc},
    time::Duration,
};

use nautilus_core::python::to_pyruntime_err;
use pyo3::prelude::*;
use tokio::io::AsyncWriteExt;
use tokio_tungstenite::tungstenite::stream::Mode;

use crate::socket::{
    SocketClient, SocketConfig, CONNECTION_ACTIVE, CONNECTION_CLOSED, CONNECTION_DISCONNECT,
    CONNECTION_RECONNECT,
};

#[pymethods]
impl SocketConfig {
    #[new]
    #[pyo3(signature = (url, ssl, suffix, handler, heartbeat=None, reconnect_timeout_ms=10_000, reconnect_delay_initial_ms=2_000, reconnect_delay_max_ms=30_000, reconnect_backoff_factor=1.5, reconnect_jitter_ms=100, certs_dir=None))]
    #[allow(clippy::too_many_arguments)]
    fn py_new(
        url: String,
        ssl: bool,
        suffix: Vec<u8>,
        handler: PyObject,
        heartbeat: Option<(u64, Vec<u8>)>,
        reconnect_timeout_ms: Option<u64>,
        reconnect_delay_initial_ms: Option<u64>,
        reconnect_delay_max_ms: Option<u64>,
        reconnect_backoff_factor: Option<f64>,
        reconnect_jitter_ms: Option<u64>,
        certs_dir: Option<String>,
    ) -> Self {
        let mode = if ssl { Mode::Tls } else { Mode::Plain };
        Self {
            url,
            mode,
            suffix,
            handler: Arc::new(handler),
            heartbeat,
            reconnect_timeout_ms,
            reconnect_delay_initial_ms,
            reconnect_delay_max_ms,
            reconnect_backoff_factor,
            reconnect_jitter_ms,
            certs_dir,
        }
    }
}

#[pymethods]
impl SocketClient {
    /// Create a socket client.
    ///
    /// # Errors
    ///
    /// - Throws an Exception if it is unable to make socket connection.
    #[staticmethod]
    #[pyo3(name = "connect")]
    #[pyo3(signature = (config, post_connection=None, post_reconnection=None, post_disconnection=None))]
    fn py_connect(
        config: SocketConfig,
        post_connection: Option<PyObject>,
        post_reconnection: Option<PyObject>,
        post_disconnection: Option<PyObject>,
        py: Python<'_>,
    ) -> PyResult<Bound<PyAny>> {
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            Self::connect(
                config,
                post_connection,
                post_reconnection,
                post_disconnection,
            )
            .await
            .map_err(to_pyruntime_err)
        })
    }

    /// Check if the client is still alive.
    ///
    /// Even if the connection is disconnected the client will still be alive
    /// and trying to reconnect. Only when reconnect fails the client will
    /// terminate.
    ///
    /// This is particularly useful for check why a `send` failed. It could
    /// be because the connection disconnected and the client is still alive
    /// and reconnecting. In such cases the send can be retried after some
    /// delay
    #[pyo3(name = "is_active")]
    fn py_is_active(slf: PyRef<'_, Self>) -> bool {
        slf.is_active()
    }

    #[pyo3(name = "is_reconnecting")]
    fn py_is_reconnecting(slf: PyRef<'_, Self>) -> bool {
        slf.is_reconnecting()
    }

    #[pyo3(name = "is_disconnecting")]
    fn py_is_disconnecting(slf: PyRef<'_, Self>) -> bool {
        slf.is_disconnecting()
    }

    #[pyo3(name = "is_closed")]
    fn py_is_closed(slf: PyRef<'_, Self>) -> bool {
        slf.is_closed()
    }

    /// Reconnect the client.
    #[pyo3(name = "reconnect")]
    fn py_reconnect<'py>(slf: PyRef<'_, Self>, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let connection_mode = slf.connection_mode.clone();
        let mode = connection_mode.load(Ordering::SeqCst);
        tracing::debug!("Reconnect from mode {mode}");

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            match connection_mode.load(Ordering::SeqCst) {
                CONNECTION_RECONNECT => {
                    tracing::warn!("Cannot reconnect: socket already reconnecting");
                }
                CONNECTION_DISCONNECT => {
                    tracing::warn!("Cannot reconnect: socket disconnecting");
                }
                CONNECTION_CLOSED => {
                    tracing::warn!("Cannot reconnect: socket closed");
                }
                _ => {
                    connection_mode.store(CONNECTION_RECONNECT, Ordering::SeqCst);
                    while connection_mode.load(Ordering::SeqCst) != CONNECTION_ACTIVE {
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                }
            }

            Ok(())
        })
    }

    /// Close the client.
    ///
    /// The connection is not completely closed until all references
    /// to the client are gone and the client is dropped.
    ///
    /// # Safety
    ///
    /// - The client should not be used after closing it
    /// - Any auto-reconnect job should be aborted before closing the client
    #[pyo3(name = "close")]
    fn py_close<'py>(slf: PyRef<'_, Self>, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let connection_mode = slf.connection_mode.clone();
        let mode = connection_mode.load(Ordering::SeqCst);
        tracing::debug!("Close from mode {mode}");

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            match connection_mode.load(Ordering::SeqCst) {
                CONNECTION_CLOSED => {
                    tracing::warn!("Socket already closed");
                }
                CONNECTION_DISCONNECT => {
                    tracing::warn!("Socket already disconnecting");
                }
                _ => {
                    connection_mode.store(CONNECTION_DISCONNECT, Ordering::SeqCst);
                    while connection_mode.load(Ordering::SeqCst) != CONNECTION_CLOSED {
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }
            }

            Ok(())
        })
    }

    /// Send bytes data to the connection.
    ///
    /// # Errors
    ///
    /// - Throws an Exception if it is not able to send data.
    #[pyo3(name = "send")]
    fn py_send<'py>(
        slf: PyRef<'_, Self>,
        mut data: Vec<u8>,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyAny>> {
        data.extend(&slf.suffix);
        let writer = slf.writer.clone();

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let mut writer = writer.lock().await;
            writer.write_all(&data).await?;
            Ok(())
        })
    }
}
