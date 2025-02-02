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

//! [NautilusTrader](http://nautilustrader.io) is an open-source, high-performance, production-grade
//! algorithmic trading platform, providing quantitative traders with the ability to backtest
//! portfolios of automated trading strategies on historical data with an event-driven engine,
//! and also deploy those same strategies live, with no code changes.
//!
//! # Feature flags
//!
//! This crate provides feature flags to control source code inclusion during compilation,
//! depending on the intended use case, i.e. whether to provide Python bindings
//! for the main `nautilus_trader` Python package, or as part of a Rust only build.
//!
//! - `python`: Enables Python bindings from `pyo3`.

#![warn(rustc::all)]
#![deny(nonstandard_style)]
#![deny(rustdoc::broken_intra_doc_links)]
// #![deny(clippy::missing_errors_doc)]

// TODO: We still rely on `IntoPy` for now, so temporarily ignore
// these deprecations until fully migrated to `IntoPyObject`.
#![allow(deprecated)]

pub mod backoff;
pub mod http;
pub mod socket;
pub mod websocket;

#[allow(dead_code)]
mod ratelimiter;
mod tls;

#[cfg(feature = "python")]
pub mod python;
