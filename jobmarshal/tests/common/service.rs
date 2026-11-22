// Copyright 2026 Julio Merino.
// SPDX-License-Identifier: GPL-3.0-only

//! Shared isolated service fixture.

#[allow(dead_code)]
#[path = "../../../tests/common/mod.rs"]
mod shared;
pub(crate) use shared::{ChildGuard, Service, copy_executable, wait};
